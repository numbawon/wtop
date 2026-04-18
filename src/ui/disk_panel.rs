use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

use crate::glyphs::Glyphs;
use crate::models::disk::DiskSnapshot;
use crate::ui::gauge_bar::build_block_bar;
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

    // Sort: highest utilization first, then by read+write bps.
    let mut sorted: Vec<&DiskSnapshot> = disks.iter().collect();
    sorted.sort_by(|a, b| {
        b.utilization_pct
            .partial_cmp(&a.utilization_pct)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| (b.read_bps + b.write_bps).cmp(&(a.read_bps + a.write_bps)))
    });

    // Show FREE column only if we have free space data for at least one disk
    // AND the panel is wide enough (>= 60 cols).
    let show_free = area.width >= 60 && sorted.iter().any(|d| d.total_bytes > 0);

    let mut header_cells = vec![
        Cell::from("DISK").style(theme.header),
        Cell::from("READ/s").style(theme.header),
        Cell::from("WRITE/s").style(theme.header),
        Cell::from("UTIL%").style(theme.header),
    ];
    if show_free {
        header_cells.push(Cell::from("FREE").style(theme.header));
    }
    let header = Row::new(header_cells).height(1);

    let rows: Vec<Row> = if sorted.is_empty() {
        vec![Row::new(vec![
            Cell::from("-"), Cell::from("-"), Cell::from("-"), Cell::from("-"),
        ])
        .style(theme.text_dim)]
    } else {
        sorted
            .iter()
            .map(|d| {
                let util = d.utilization_pct as f64;
                let bar = build_block_bar(util / 100.0, 8);
                let util_style = theme.gauge_for_pct(util);
                let util_cell = Cell::from(Line::from(vec![
                    Span::styled(bar, util_style),
                    Span::styled(format!(" {:4.1}%", d.utilization_pct), theme.row_normal),
                ]));

                let mut cells = vec![
                    Cell::from(d.drive.clone()),
                    Cell::from(ByteSize(d.read_bps).to_string() + "/s"),
                    Cell::from(ByteSize(d.write_bps).to_string() + "/s"),
                    util_cell,
                ];

                if show_free {
                    let free_str = if d.total_bytes > 0 {
                        ByteSize(d.free_bytes).to_string()
                    } else {
                        "-".to_string()
                    };
                    let free_style = if d.total_bytes > 0 {
                        let used_pct = 100.0 - (d.free_bytes as f64 / d.total_bytes as f64 * 100.0);
                        theme.gauge_for_pct(used_pct)
                    } else {
                        theme.text_dim
                    };
                    cells.push(Cell::from(free_str).style(free_style));
                }

                Row::new(cells).style(theme.row_normal)
            })
            .collect()
    };

    let mut constraints = vec![
        Constraint::Min(4),     // drive letter - expands to fill available space
        Constraint::Length(10), // READ/s
        Constraint::Length(10), // WRITE/s
        Constraint::Length(13), // 8 bar + 1 space + " XX.X%"
    ];
    if show_free {
        constraints.push(Constraint::Length(9)); // FREE
    }

    let table = Table::new(rows, constraints)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_set(theme.border_set)
                .border_style(border_style)
                .title(Span::styled(
                    format!(" {}Disk I/O ", glyphs.disk_icon),
                    theme.title,
                )),
        );

    frame.render_widget(table, area);
}
