use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Rect},
    style::Style,
    text::Span,
    widgets::{Block, Borders, Cell, Row, Table, TableState},
    Frame,
};

use crate::app::AppState;
use crate::models::process::ProcessEntry;
use crate::ui::theme::Theme;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme, focused: bool) {
    let border_style = if focused { theme.border_focused } else { theme.border };

    let processes = match state.hub.processes.read() {
        Ok(p) => p,
        Err(_) => return,
    };

    // Filter and collect visible entries.
    let visible: Vec<&ProcessEntry> = processes
        .iter()
        .filter(|p| {
            if !state.show_system_processes && p.user == "SYSTEM" {
                return false;
            }
            if !state.filter_text.is_empty()
                && !p.name.to_lowercase().contains(&state.filter_text.to_lowercase())
            {
                return false;
            }
            true
        })
        .collect();

    let total = processes.len();
    let shown = visible.len();

    // Column widths.
    let header_cells = [
        "PID", "NAME", "CPU%", "MEM", "MEM%", "THDS", "STATUS", "USER",
    ]
    .iter()
    .map(|h| {
        Cell::from(*h).style(theme.header)
    });

    let header = Row::new(header_cells).height(1).bottom_margin(0);

    // Build rows — each process may have expanded thread sub-rows.
    let mut rows: Vec<Row> = Vec::new();
    for (idx, proc) in visible.iter().enumerate() {
        let is_selected = idx == state.process_cursor;
        let row_style = if is_selected {
            theme.row_selected
        } else {
            theme.row_normal
        };

        let expand_marker = if proc.expanded { "▼" } else if proc.thread_count > 0 { "▶" } else { " " };
        let name_col = format!("{} {}", expand_marker, proc.name);

        let cells = vec![
            Cell::from(proc.pid.to_string()),
            Cell::from(name_col),
            Cell::from(format!("{:>5.1}", proc.cpu_pct)),
            Cell::from(ByteSize(proc.mem_bytes).to_string()),
            Cell::from(format!("{:>4.1}%", proc.mem_pct)),
            Cell::from(proc.thread_count.to_string()),
            Cell::from(proc.status.to_string()).style(status_style(proc, theme)),
            Cell::from(truncate(&proc.user, 12)),
        ];

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

                let suspicious_marker = if thread.suspicious { " ⚠" } else { "" };
                let name_cell = format!(
                    "{} TID:{} {}{}",
                    tree, thread.tid, thread.state, suspicious_marker
                );

                let thread_cells = vec![
                    Cell::from(""),
                    Cell::from(name_cell),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(format!("cpu:{}ms", thread.cpu_time_ms)),
                    Cell::from(format!("pri:{}", thread.priority)),
                    Cell::from(truncate(&thread.start_module, 20)),
                ];

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
        " Processes  {}  Total:{} Shown:{} ",
        sort_label, total, shown
    );

    let table = Table::new(
        rows,
        [
            Constraint::Length(7),   // PID
            Constraint::Min(18),     // NAME
            Constraint::Length(6),   // CPU%
            Constraint::Length(9),   // MEM
            Constraint::Length(6),   // MEM%
            Constraint::Length(5),   // THDS
            Constraint::Length(8),   // STATUS
            Constraint::Length(12),  // USER
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(Span::styled(title, theme.title)),
    )
    .row_highlight_style(theme.row_selected)
    .highlight_symbol("» ");

    let mut table_state = TableState::default();
    table_state.select(Some(state.process_cursor));

    frame.render_stateful_widget(table, area, &mut table_state);
}

fn status_style(proc: &ProcessEntry, theme: &Theme) -> Style {
    match proc.status {
        crate::models::process::ProcessStatus::Running => theme.status_running,
        crate::models::process::ProcessStatus::Suspended => theme.status_suspended,
        _ => theme.status_other,
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max - 1])
    }
}

