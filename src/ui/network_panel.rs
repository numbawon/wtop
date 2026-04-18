use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Rect},
    text::Span,
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

use crate::app::AppState;
use crate::glyphs::Glyphs;
use crate::models::network::NetSnapshot;
use crate::ui::theme::Theme;

/// Sort priority for a network adapter - lower number = higher in the list.
fn sort_priority(n: &NetSnapshot) -> u8 {
    let has_traffic = n.rx_bps > 0 || n.tx_bps > 0;
    match (has_traffic, n.is_up, n.is_virtual) {
        (true,  true,  _    ) => 0, // active traffic - always first
        (false, true,  false) => 1, // physical, up, idle
        (false, true,  true ) => 2, // virtual, up, idle
        (_,     false, false) => 3, // physical, down
        (_,     false, true ) => 4, // virtual, down
    }
}

pub fn render(
    frame: &mut Frame,
    area: Rect,
    networks: &[NetSnapshot],
    state: &AppState,
    theme: &Theme,
    glyphs: &Glyphs,
    focused: bool,
) {
    let border_style = if focused { theme.border_focused } else { theme.border };

    // Filter: remove explicitly hidden adapters and optionally all virtual ones.
    let mut visible: Vec<&NetSnapshot> = networks
        .iter()
        .filter(|n| {
            if state.config.hide_virtual_adapters && n.is_virtual {
                return false;
            }
            !state.config.hidden_adapters.contains(&n.display_name)
        })
        .collect();

    let hidden_count = networks.len().saturating_sub(visible.len());

    // Sort: active traffic first, then physical-up, virtual-up, physical-down, virtual-down.
    // Within the same priority tier, busiest (rx+tx bps) first.
    visible.sort_by(|a, b| {
        sort_priority(a)
            .cmp(&sort_priority(b))
            .then_with(|| (b.rx_bps + b.tx_bps).cmp(&(a.rx_bps + a.tx_bps)))
    });

    let header = Row::new(vec![
        Cell::from("ADAPTER").style(theme.header),
        Cell::from("RX/s").style(theme.header),
        Cell::from("TX/s").style(theme.header),
        Cell::from("UP").style(theme.header),
    ])
    .height(1);

    let rows: Vec<Row> = if visible.is_empty() && networks.is_empty() {
        vec![Row::new(vec![
            Cell::from("-"), Cell::from("-"), Cell::from("-"), Cell::from("-"),
        ])
        .style(theme.text_dim)]
    } else if visible.is_empty() {
        vec![Row::new(vec![
            Cell::from(format!("({} hidden)", hidden_count)),
            Cell::from(""), Cell::from(""), Cell::from(""),
        ])
        .style(theme.text_dim)]
    } else {
        visible
            .iter()
            .map(|n| {
                let up = if n.is_up { glyphs.net_up } else { glyphs.net_down };
                let name_style = if n.is_virtual { theme.text_dim } else { theme.text_normal };
                Row::new(vec![
                    Cell::from(super::truncate(&n.display_name, 36)).style(name_style),
                    Cell::from(ByteSize(n.rx_bps).to_string() + "/s"),
                    Cell::from(ByteSize(n.tx_bps).to_string() + "/s"),
                    Cell::from(up),
                ])
                .style(theme.row_normal)
            })
            .collect()
    };

    // Show count of hidden adapters in the panel title.
    let title = if hidden_count > 0 {
        format!(" {}Network  ({} hidden) ", glyphs.net_icon, hidden_count)
    } else {
        format!(" {}Network ", glyphs.net_icon)
    };

    let table = Table::new(
        rows,
        [
            Constraint::Min(28),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(4),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_set(theme.border_set)
            .border_style(border_style)
            .title(Span::styled(title, theme.title)),
    );

    frame.render_widget(table, area);
}

