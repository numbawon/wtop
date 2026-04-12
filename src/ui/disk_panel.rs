use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Rect},
    text::Span,
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

use crate::glyphs::Glyphs;
use crate::models::disk::DiskSnapshot;
use crate::ui::theme::Theme;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    disks: &[DiskSnapshot],
    theme: &Theme,
    glyphs: &Glyphs,
    focused: bool,
) {
    let border_style = if focused { theme.border_focused } else { theme.border };

    let header = Row::new(vec![
        Cell::from("DISK").style(theme.header),
        Cell::from("READ/s").style(theme.header),
        Cell::from("WRITE/s").style(theme.header),
        Cell::from("UTIL%").style(theme.header),
    ])
    .height(1);

    let rows: Vec<Row> = if disks.is_empty() {
        vec![Row::new(vec![
            Cell::from("—"),
            Cell::from("—"),
            Cell::from("—"),
            Cell::from("—"),
        ])
        .style(theme.text_dim)]
    } else {
        disks
            .iter()
            .map(|d| {
                Row::new(vec![
                    Cell::from(d.device_name.clone()),
                    Cell::from(ByteSize(d.read_bps).to_string() + "/s"),
                    Cell::from(ByteSize(d.write_bps).to_string() + "/s"),
                    Cell::from(format!("{:.1}%", d.utilization_pct)),
                ])
                .style(theme.row_normal)
            })
            .collect()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Min(12),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(7),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_set(theme.border_set.clone())
            .border_style(border_style)
            .title(Span::styled(
            format!(" {}Disk I/O ", glyphs.disk_icon),
            theme.title,
        )),
    );

    frame.render_widget(table, area);
}
