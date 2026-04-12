use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Rect},
    text::Span,
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

use crate::glyphs::Glyphs;
use crate::models::network::NetSnapshot;
use crate::ui::theme::Theme;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    networks: &[NetSnapshot],
    theme: &Theme,
    glyphs: &Glyphs,
    focused: bool,
) {
    let border_style = if focused { theme.border_focused } else { theme.border };

    let header = Row::new(vec![
        Cell::from("ADAPTER").style(theme.header),
        Cell::from("RX/s").style(theme.header),
        Cell::from("TX/s").style(theme.header),
        Cell::from("UP").style(theme.header),
    ])
    .height(1);

    let rows: Vec<Row> = if networks.is_empty() {
        vec![Row::new(vec![
            Cell::from("—"),
            Cell::from("—"),
            Cell::from("—"),
            Cell::from("—"),
        ])
        .style(theme.text_dim)]
    } else {
        networks
            .iter()
            .map(|n| {
                let up = if n.is_up { glyphs.net_up } else { glyphs.net_down };
                Row::new(vec![
                    Cell::from(super::truncate(&n.display_name, 18)),
                    Cell::from(ByteSize(n.rx_bps).to_string() + "/s"),
                    Cell::from(ByteSize(n.tx_bps).to_string() + "/s"),
                    Cell::from(up),
                ])
                .style(theme.row_normal)
            })
            .collect()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Min(18),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(4),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_set(theme.border_set.clone())
            .border_style(border_style)
            .title(Span::styled(
            format!(" {}Network ", glyphs.net_icon),
            theme.title,
        )),
    );

    frame.render_widget(table, area);
}

