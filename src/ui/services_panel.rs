use ratatui::{
    layout::{Constraint, Rect},
    style::Style,
    text::Span,
    widgets::{Block, Borders, Cell, Clear, Row, Table},
    Frame,
};

use crate::models::services::{ServiceEntry, ServiceStartType, ServiceStatus};
use crate::ui::theme::{heat_color, Theme};

pub fn render(
    frame: &mut Frame,
    area: Rect,
    services: &[ServiceEntry],
    cursor: usize,
    filter: &str,
    theme: &Theme,
) {
    let width  = area.width.min(100);
    let height = area.height.clamp(10, 40);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    let visible: Vec<&ServiceEntry> = if filter.is_empty() {
        services.iter().collect()
    } else {
        let lower = filter.to_lowercase();
        services.iter()
            .filter(|s| s.name.to_lowercase().contains(&lower)
                || s.display_name.to_lowercase().contains(&lower))
            .collect()
    };

    let header = Row::new(vec![
        Cell::from("STATUS").style(theme.header),
        Cell::from("NAME").style(theme.header),
        Cell::from("DISPLAY NAME").style(theme.header),
        Cell::from("START").style(theme.header),
        Cell::from("PID").style(theme.header),
    ]).height(1);

    let rows: Vec<Row> = visible.iter().enumerate().map(|(i, svc)| {
        let is_selected = i == cursor;

        let status_style = status_color(&svc.status, theme);
        let row_style = if is_selected {
            theme.row_selected
        } else {
            theme.row_normal
        };

        let pid_str = if svc.pid > 0 { svc.pid.to_string() } else { String::new() };

        Row::new(vec![
            Cell::from(svc.status.label()).style(if is_selected { theme.row_selected } else { status_style }),
            Cell::from(svc.name.as_str()).style(if is_selected { theme.row_selected } else { theme.text_normal }),
            Cell::from(svc.display_name.as_str()).style(if is_selected { theme.row_selected } else { theme.text_dim }),
            Cell::from(start_type_label(&svc.start_type)).style(if is_selected { theme.row_selected } else { theme.text_dim }),
            Cell::from(pid_str).style(if is_selected { theme.row_selected } else { theme.text_dim }),
        ]).style(row_style)
    }).collect();

    let title = if filter.is_empty() {
        format!(" Services ({}) ", visible.len())
    } else {
        format!(" Services [/{}] ({}) ", filter, visible.len())
    };

    let hint = format!(
        "  ↑↓ nav  / filter  Esc close  ({} total)",
        services.len()
    );

    let inner_height = popup.height.saturating_sub(3) as usize; // border + header + hint
    let scroll_offset = cursor.saturating_sub(inner_height.saturating_sub(1));

    let scrolled_rows: Vec<Row> = rows.into_iter().skip(scroll_offset).collect();

    let table = Table::new(scrolled_rows, [
        Constraint::Length(9),   // STATUS
        Constraint::Length(22),  // NAME
        Constraint::Min(20),     // DISPLAY NAME
        Constraint::Length(9),   // START
        Constraint::Length(7),   // PID
    ])
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_set(theme.border_set)
            .border_style(theme.border_focused)
            .title(Span::styled(title, theme.title))
            .title_bottom(Span::styled(hint, theme.text_dim)),
    );

    frame.render_widget(Clear, popup);
    frame.render_widget(table, popup);
}

fn status_color(status: &ServiceStatus, theme: &Theme) -> Style {
    match status {
        ServiceStatus::Running => Style::default().fg(heat_color(20.0)),
        ServiceStatus::Stopped => Style::default().fg(theme.text_dim.fg.unwrap_or(ratatui::style::Color::DarkGray)),
        ServiceStatus::StartPending | ServiceStatus::ContinuePending => {
            Style::default().fg(heat_color(55.0))
        }
        ServiceStatus::StopPending | ServiceStatus::PausePending => {
            Style::default().fg(heat_color(75.0))
        }
        ServiceStatus::Paused => Style::default().fg(heat_color(45.0)),
        ServiceStatus::Unknown => theme.text_dim,
    }
}

fn start_type_label(st: &ServiceStartType) -> &'static str {
    st.label()
}
