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

    for (proc_idx, proc) in visible.iter().enumerate() {
        let is_selected = proc_idx == state.process_cursor;
        let row_style = if is_selected {
            theme.row_selected
        } else if proc_idx % 2 == 1 {
            theme.row_zebra
        } else {
            theme.row_normal
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
            .map(|id| build_cell(id, proc, expand_marker, row_style, theme))
            .collect();

        rows.push(Row::new(cells).style(row_style));

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
                            Cell::from(format!(
                                "{} TID:{} {}{}",
                                tree, thread.tid, state_str, suspicious_marker
                            ))
                        }
                        ProcessColumnId::Threads => {
                            Cell::from(format!("cpu:{}ms", thread.cpu_time_ms))
                        }
                        ProcessColumnId::Status => {
                            Cell::from(format!("pri:{}", thread.priority))
                        }
                        ProcessColumnId::User => {
                            Cell::from(super::truncate(&thread.start_module, 20))
                        }
                        _ => Cell::from(""),
                    })
                    .collect();

                rows.push(Row::new(thread_cells).style(thread_style));
            }
        }
    }

    let sort_label = format!(
        "Sort:{}{} ",
        state.sort_state.field.label(),
        if state.sort_state.ascending { "▲" } else { "▼" }
    );

    let title = format!(
        " {}Processes  {}  Total:{} Shown:{} ",
        glyphs.proc_icon, sort_label, total, shown
    );

    let table = Table::new(rows, constraints)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_set(theme.border_set.clone())
                .border_style(border_style)
                .title(Span::styled(title, theme.title)),
        )
        .row_highlight_style(theme.row_selected)
        .highlight_symbol(glyphs.row_cursor);

    let mut table_state = TableState::default();
    table_state.select(Some(state.process_cursor));

    frame.render_stateful_widget(table, area, &mut table_state);
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
