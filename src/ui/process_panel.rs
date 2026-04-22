use ratatui::{
    layout::{Constraint, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
    Frame,
};

use crate::app::AppState;
use crate::config::ProcessColumnId;
use crate::models::thread::{wait_reason_label, ThreadState};
use crate::glyphs::Glyphs;
use crate::models::process::ProcessEntry;
use crate::ui::gauge_bar::build_block_bar;
use crate::ui::theme::Theme;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    theme: &Theme,
    glyphs: &Glyphs,
    focused: bool,
) {
    let border_style = if focused { theme.border_focused } else { theme.border };

    let processes = match state.hub.processes.read() {
        Ok(p) => p,
        Err(_) => return,
    };

    let visible: Vec<&ProcessEntry> = processes
        .iter()
        .filter(|p| state.process_matches(p))
        .collect();

    let total = processes.len();
    let shown = visible.len();

    // In tree mode build depth map for each visible process
    let depths: Vec<usize> = if state.config.tree_view {
        compute_depths(&visible)
    } else {
        vec![0usize; shown]
    };

    // Collect visible columns in order.
    let cols: Vec<&ProcessColumnId> = state.config.process_columns
        .iter()
        .filter(|c| c.visible)
        .map(|c| &c.id)
        .collect();

    let constraints: Vec<Constraint> = cols.iter().map(|id| col_constraint(id)).collect();

    let header_cells: Vec<Cell> = cols
        .iter()
        .map(|id| Cell::from(col_header(id)).style(theme.header))
        .collect();
    let header = Row::new(header_cells).height(1).bottom_margin(0);

    let mut rows: Vec<Row> = Vec::new();

    // display_indices: for each display row, which visible[] index to show
    let display_indices: Vec<usize> = if state.config.tree_view && !state.tree_display_order.is_empty() {
        state.tree_display_order.clone()
    } else {
        (0..shown).collect()
    };

    // rows_index counts every pushed row (process + thread sub-rows) so that
    // is_selected correctly matches process_cursor regardless of expanded threads.
    let mut rows_index: usize = 0;

    for (display_row, &vi) in display_indices.iter().enumerate() {
        let proc = match visible.get(vi) {
            Some(p) => *p,
            None => continue,
        };
        let is_selected = rows_index == state.process_cursor;
        let row_style = if is_selected {
            theme.row_selected
        } else if state.cpu_spike_flash.contains_key(&proc.pid) {
            theme.row_spike
        } else if display_row % 2 == 1 {
            theme.row_zebra
        } else {
            theme.row_normal
        };

        let depth = depths.get(vi).copied().unwrap_or(0);
        let tree_prefix = if state.config.tree_view && depth > 0 {
            format!("{}{} ", "  ".repeat(depth - 1), "└")
        } else {
            String::new()
        };

        let expand_marker = if proc.expanded {
            glyphs.expand_open
        } else if proc.thread_count > 0 {
            glyphs.expand_closed
        } else {
            glyphs.expand_none
        };

        let cells: Vec<Cell> = cols
            .iter()
            .map(|id| build_cell_tree(id, proc, expand_marker, &tree_prefix, row_style, theme))
            .collect();

        rows.push(Row::new(cells).style(row_style));
        rows_index += 1;

        // Expanded thread sub-rows.
        if proc.expanded {
            for (t_idx, thread) in proc.threads.iter().enumerate() {
                let is_last = t_idx == proc.threads.len() - 1;
                let tree = if is_last { "  └" } else { "  ├" };
                let thread_style = if thread.suspicious {
                    theme.row_suspicious
                } else {
                    theme.row_thread
                };
                let suspicious_marker = if thread.suspicious { glyphs.suspicious } else { "" };

                let thread_cells: Vec<Cell> = cols
                    .iter()
                    .map(|id| match id {
                        ProcessColumnId::Name => {
                            let state_str = match thread.state {
                                ThreadState::Waiting => format!(
                                    "Wait:{}",
                                    wait_reason_label(thread.wait_reason)
                                ),
                                _ => thread.state.to_string(),
                            };
                            // Show thread description name in brackets when present,
                            // plus base priority (useful for spotting elevated threads).
                            // Cap thread name at 32 chars to prevent very long Java/Chromium
                            // names from making the row unwieldy.
                            let name_part = match &thread.name {
                                Some(n) => format!("[{}] ", super::truncate(n, 32)),
                                None => String::new(),
                            };
                            Cell::from(format!(
                                "{} TID:{} p:{} {}{}{}",
                                tree, thread.tid, thread.priority, name_part, state_str, suspicious_marker
                            ))
                        }
                        ProcessColumnId::Threads => {
                            // Show live CPU% rate capped to 4 chars + % to fit Length(5).
                            // "---" on the very first sample before any delta exists.
                            if thread.kernel_ms > 0 || thread.user_ms > 0 {
                                let pct = thread.cpu_pct.min(99.9);
                                Cell::from(format!("{:.1}%", pct))
                            } else {
                                Cell::from("---")
                            }
                        }
                        ProcessColumnId::Status => {
                            // Kernel CPU time - most useful for spotting syscall-heavy threads.
                            Cell::from(format_kernel_time(thread.kernel_ms))
                        }
                        ProcessColumnId::User => {
                            // For suspicious threads show the raw start address so the
                            // analyst can look it up; otherwise show the module name.
                            if thread.suspicious && thread.start_address != 0 {
                                Cell::from(super::truncate(
                                    &format!("0x{:016x}", thread.start_address),
                                    12,
                                ))
                            } else {
                                Cell::from(super::truncate(&thread.start_module, 12))
                            }
                        }
                        _ => Cell::from(""),
                    })
                    .collect();

                rows.push(Row::new(thread_cells).style(thread_style));
                rows_index += 1;
            }
        }
    }

    let sort_label = format!(
        "Sort:{}{} ",
        state.sort_state.field.label(),
        if state.sort_state.ascending { "▲" } else { "▼" }
    );
    let tree_label = if state.config.tree_view { " [Tree] " } else { "" };

    let title = format!(
        " {}Processes  {}{}  Total:{} Shown:{} ",
        glyphs.proc_icon, sort_label, tree_label, total, shown
    );

    let table = Table::new(rows, constraints)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_set(theme.border_set)
                .border_style(border_style)
                .title(Span::styled(title, theme.title)),
        )
        .row_highlight_style(theme.row_selected)
        .highlight_symbol(glyphs.row_cursor);

    let mut table_state = TableState::default();
    table_state.select(Some(state.process_cursor));

    frame.render_stateful_widget(table, area, &mut table_state);
}

fn build_cell_tree<'a>(
    id: &ProcessColumnId,
    proc: &'a ProcessEntry,
    expand_marker: &str,
    tree_prefix: &str,
    row_style: Style,
    theme: &'a Theme,
) -> Cell<'a> {
    match id {
        ProcessColumnId::Name => Cell::from(
            format!("{}{} {}", tree_prefix, expand_marker, proc.name)
        ),
        _ => build_cell(id, proc, expand_marker, row_style, theme),
    }
}

fn build_cell<'a>(
    id: &ProcessColumnId,
    proc: &'a ProcessEntry,
    expand_marker: &str,
    row_style: Style,
    theme: &'a Theme,
) -> Cell<'a> {
    match id {
        ProcessColumnId::Pid => Cell::from(proc.pid_str.as_str()),
        ProcessColumnId::Name => Cell::from(format!("{} {}", expand_marker, proc.name)),
        ProcessColumnId::CpuPct => {
            // 3-char inline mini bar + padded percentage.
            // Total width = 3 + 1 space + 6 (" XX.X%") = 10 → col_constraint is Length(10).
            let bar = build_block_bar(proc.cpu_pct as f64 / 100.0, 3);
            Cell::from(Line::from(vec![
                Span::styled(bar, theme.gauge_for_pct(proc.cpu_pct as f64)),
                Span::styled(proc.cpu_pct_str.as_str(), row_style),
            ]))
        }
        ProcessColumnId::Mem => Cell::from(proc.mem_str.as_str()),
        ProcessColumnId::MemPct => Cell::from(proc.mem_pct_str.as_str()),
        ProcessColumnId::Threads => Cell::from(proc.thread_count_str.as_str()),
        ProcessColumnId::Status => {
            Cell::from(proc.status.as_str()).style(status_style(proc, theme))
        }
        ProcessColumnId::User => Cell::from(super::truncate(&proc.user, 12)),
        ProcessColumnId::DiskRead => Cell::from(proc.disk_read_str.as_str()),
        ProcessColumnId::DiskWrite => Cell::from(proc.disk_write_str.as_str()),
    }
}

fn col_constraint(id: &ProcessColumnId) -> Constraint {
    match id {
        ProcessColumnId::Pid       => Constraint::Length(7),
        ProcessColumnId::Name      => Constraint::Min(18),
        ProcessColumnId::CpuPct    => Constraint::Length(10),
        ProcessColumnId::Mem       => Constraint::Length(9),
        ProcessColumnId::MemPct    => Constraint::Length(6),
        ProcessColumnId::Threads   => Constraint::Length(5),
        ProcessColumnId::Status    => Constraint::Length(8),
        ProcessColumnId::User      => Constraint::Length(12),
        ProcessColumnId::DiskRead  => Constraint::Length(11),
        ProcessColumnId::DiskWrite => Constraint::Length(11),
    }
}

fn col_header(id: &ProcessColumnId) -> &'static str {
    match id {
        ProcessColumnId::Pid       => "PID",
        ProcessColumnId::Name      => "NAME",
        ProcessColumnId::CpuPct    => "CPU%",
        ProcessColumnId::Mem       => "MEM",
        ProcessColumnId::MemPct    => "MEM%",
        ProcessColumnId::Threads   => "THDS",
        ProcessColumnId::Status    => "STATUS",
        ProcessColumnId::User      => "USER",
        ProcessColumnId::DiskRead  => "DISK-R",
        ProcessColumnId::DiskWrite => "DISK-W",
    }
}

fn status_style(proc: &ProcessEntry, theme: &Theme) -> Style {
    match proc.status {
        crate::models::process::ProcessStatus::Running   => theme.status_running,
        crate::models::process::ProcessStatus::Suspended => theme.status_suspended,
        _ => theme.status_other,
    }
}

/// Format kernel CPU time compactly to fit the 8-char Status column.
/// Uses ms for values under 10 seconds, seconds otherwise.
fn format_kernel_time(ms: u64) -> String {
    if ms < 10_000 {
        format!("K:{}ms", ms)
    } else {
        format!("K:{}s", ms / 1_000)
    }
}

/// Compute tree depth for each visible process (index = position in visible list).
/// Depth 0 = root, depth 1 = child, etc.
fn compute_depths(visible: &[&ProcessEntry]) -> Vec<usize> {
    use std::collections::HashMap;
    let pid_to_vi: HashMap<u32, usize> = visible.iter().enumerate()
        .map(|(i, p)| (p.pid, i))
        .collect();

    let mut depths = vec![0usize; visible.len()];
    for (i, p) in visible.iter().enumerate() {
        let mut depth = 0usize;
        let mut cur_pid = p.parent_pid;
        let mut visited = std::collections::HashSet::new();
        while cur_pid > 0 {
            if !visited.insert(cur_pid) { break; }
            if let Some(&pi) = pid_to_vi.get(&cur_pid) {
                depth += 1;
                cur_pid = visible[pi].parent_pid;
            } else {
                break;
            }
        }
        depths[i] = depth;
    }
    depths
}
