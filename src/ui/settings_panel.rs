use ratatui::{
    layout::{Constraint, Rect},
    style::Style,
    text::Span,
    widgets::{Block, Borders, Cell, Clear, Row, Table},
    Frame,
};

use crate::app::AppState;
use crate::config::ProcessColumnId;
use crate::ui::theme::Theme;

/// Total number of selectable settings items.
pub const SETTINGS_COUNT: usize = 11;

enum RowKind {
    Header,
    Item(usize),
    Hint,
    Spacer,
}

struct SettingRow {
    label: &'static str,
    value: String,
    kind: RowKind,
}

pub fn render(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let width  = 58u16.min(area.width);
    let height = 20u16.min(area.height);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    let disk_io_shown = state.config.process_columns
        .iter()
        .any(|c| c.id == ProcessColumnId::DiskRead && c.visible);

    let rows_spec: Vec<SettingRow> = vec![
        SettingRow { label: "── Display",          value: String::new(),                                                     kind: RowKind::Header    },
        SettingRow { label: "  Theme",              value: state.config.theme.label().into(),                                 kind: RowKind::Item(0)   },
        SettingRow { label: "  Layout",             value: state.config.layout_mode.label().into(),                          kind: RowKind::Item(1)   },
        SettingRow { label: "  Nerd Font Glyphs",   value: on_off(state.config.nerd_glyphs),                                 kind: RowKind::Item(2)   },
        SettingRow { label: "  ASCII Mode",         value: on_off(state.config.ascii_mode),                                  kind: RowKind::Item(3)   },
        SettingRow { label: "── Panels",            value: String::new(),                                                     kind: RowKind::Header    },
        SettingRow { label: "  Disk Panel",         value: shown_hidden(state.config.show_disk),                             kind: RowKind::Item(4)   },
        SettingRow { label: "  Network Panel",      value: shown_hidden(state.config.show_network),                          kind: RowKind::Item(5)   },
        SettingRow { label: "  Disk I/O Columns",   value: shown_hidden(disk_io_shown),                                      kind: RowKind::Item(6)   },
        SettingRow { label: "── Processes",         value: String::new(),                                                     kind: RowKind::Header    },
        SettingRow { label: "  System Processes",   value: shown_hidden(state.config.show_system_processes),                 kind: RowKind::Item(7)   },
        SettingRow { label: "── Network",               value: String::new(),                                                kind: RowKind::Header    },
        SettingRow { label: "  Hide Virtual Adapters", value: on_off(state.config.hide_virtual_adapters),                   kind: RowKind::Item(8)   },
        SettingRow { label: "  Adapter Filters →",     value: format!("{} hidden", state.config.hidden_adapters.len()),     kind: RowKind::Item(9)   },
        SettingRow { label: "── General",              value: String::new(),                                                kind: RowKind::Header    },
        SettingRow { label: "  Refresh Interval",      value: format!("{}ms", state.config.refresh_interval_ms),           kind: RowKind::Item(10)  },
        SettingRow { label: "",                     value: String::new(),                                                     kind: RowKind::Spacer    },
        SettingRow { label: "  ↑↓ nav  ←→/Enter change  Esc close", value: String::new(),                                   kind: RowKind::Hint      },
    ];

    let rows: Vec<Row> = rows_spec.iter().map(|r| {
        let (label_style, value_style, row_style) = styles_for(&r.kind, state.settings_cursor, theme);
        Row::new(vec![
            Cell::from(r.label).style(label_style),
            Cell::from(r.value.as_str()).style(value_style),
        ]).style(row_style)
    }).collect();

    let table = Table::new(rows, [Constraint::Min(22), Constraint::Length(18)])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_set(theme.border_set.clone())
                .border_style(theme.border_focused)
                .title(Span::styled(" Config ", theme.title)),
        );

    frame.render_widget(Clear, popup);
    frame.render_widget(table, popup);
}

fn styles_for(kind: &RowKind, cursor: usize, theme: &Theme) -> (Style, Style, Style) {
    match kind {
        RowKind::Header => (theme.header, theme.header, theme.panel_bg),
        RowKind::Hint | RowKind::Spacer => (theme.text_dim, theme.text_dim, theme.panel_bg),
        RowKind::Item(idx) => {
            if *idx == cursor {
                (theme.row_selected, theme.row_selected, theme.row_selected)
            } else {
                (theme.text_normal, theme.text_bright, theme.panel_bg)
            }
        }
    }
}

fn on_off(v: bool)      -> String { if v { "On".into()     } else { "Off".into()    } }
fn shown_hidden(v: bool) -> String { if v { "Shown".into() } else { "Hidden".into() } }
