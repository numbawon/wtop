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
pub const SETTINGS_COUNT: usize = 25;

enum RowKind {
    Header,
    Item(usize),
    Hint,
    Spacer,
}

struct SettingRow {
    label: String,
    value: String,
    kind: RowKind,
}

impl SettingRow {
    fn header(label: &'static str) -> Self {
        Self { label: label.into(), value: String::new(), kind: RowKind::Header }
    }
    fn item(label: &'static str, value: String, idx: usize) -> Self {
        Self { label: label.into(), value, kind: RowKind::Item(idx) }
    }
    fn hint(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self { label: label.into(), value: value.into(), kind: RowKind::Hint }
    }
    fn spacer() -> Self {
        Self { label: String::new(), value: String::new(), kind: RowKind::Spacer }
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let width  = 58u16.min(area.width);
    let height = 36u16.min(area.height);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    let disk_io_shown = state.config.process_columns
        .iter()
        .any(|c| c.id == ProcessColumnId::DiskRead && c.visible);

    let mut rows_spec: Vec<SettingRow> = vec![
        SettingRow::header("── Display"),
        SettingRow::item("  Theme", state.theme_display_name.clone(), 0),
    ];

    // Attribution row: show author + homepage when the theme has metadata.
    let author   = state.theme_author.as_deref().unwrap_or("");
    let homepage = state.theme_homepage.as_deref()
        .map(shorten_url)
        .unwrap_or("");
    if !author.is_empty() || !homepage.is_empty() {
        let label = if author.is_empty() {
            String::new()
        } else {
            format!("    by {author}")
        };
        rows_spec.push(SettingRow::hint(label, homepage));
    }

    rows_spec.extend([
        SettingRow::item("  Layout",             state.config.layout_mode.label().into(),                        1),
        SettingRow::item("  Nerd Font Glyphs",   on_off(state.config.nerd_glyphs),                               2),
        SettingRow::item("  ASCII Mode",         on_off(state.config.ascii_mode),                                3),
        SettingRow::header("── Panels"),
        SettingRow::item("  Disk Panel",         shown_hidden(state.config.show_disk),                           4),
        SettingRow::item("  Network Panel",      shown_hidden(state.config.show_network),                        5),
        SettingRow::item("  GPU Panel",          shown_hidden(state.config.show_gpu),                            6),
        SettingRow::item("  NPU Panel",          shown_hidden(state.config.show_npu),                            7),
        SettingRow::item("  Disk I/O Columns",   shown_hidden(disk_io_shown),                                    8),
        SettingRow::header("── Processes"),
        SettingRow::item("  System Processes",   shown_hidden(state.config.show_system_processes),               9),
        SettingRow::item("  Tree View",          on_off(state.config.tree_view),                                10),
        SettingRow::header("── Network"),
        SettingRow::item("  Hide Virtual Adapters", on_off(state.config.hide_virtual_adapters),                 11),
        SettingRow::item("  Adapter Filters →",     format!("{} hidden", state.config.hidden_adapters.len()),   12),
        SettingRow::header("── General"),
        SettingRow::item("  Refresh Interval",   format!("{}ms", state.config.refresh_interval_ms),             13),
        SettingRow::item("  Clock Format",       if state.config.time_24h { "24h".into() } else { "12h AM/PM".into() }, 14),
        SettingRow::header("── Columns"),
        SettingRow::item("  PID",    col_vis(state, ProcessColumnId::Pid),       15),
        SettingRow::item("  NAME",   col_vis(state, ProcessColumnId::Name),      16),
        SettingRow::item("  CPU%",   col_vis(state, ProcessColumnId::CpuPct),    17),
        SettingRow::item("  MEM",    col_vis(state, ProcessColumnId::Mem),       18),
        SettingRow::item("  MEM%",   col_vis(state, ProcessColumnId::MemPct),    19),
        SettingRow::item("  THDS",   col_vis(state, ProcessColumnId::Threads),   20),
        SettingRow::item("  STATUS", col_vis(state, ProcessColumnId::Status),    21),
        SettingRow::item("  USER",   col_vis(state, ProcessColumnId::User),      22),
        SettingRow::item("  DISK-R", col_vis(state, ProcessColumnId::DiskRead),  23),
        SettingRow::item("  DISK-W", col_vis(state, ProcessColumnId::DiskWrite), 24),
        SettingRow::spacer(),
        SettingRow::hint("  ↑↓ nav  ←→/Enter change  Esc close", ""),
    ]);

    let rows: Vec<Row> = rows_spec.iter().map(|r| {
        let (label_style, value_style, row_style) = styles_for(&r.kind, state.settings_cursor, theme);
        Row::new(vec![
            Cell::from(r.label.as_str()).style(label_style),
            Cell::from(r.value.as_str()).style(value_style),
        ]).style(row_style)
    }).collect();

    let table = Table::new(rows, [Constraint::Min(22), Constraint::Length(18)])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_set(theme.border_set)
                .border_style(theme.border_focused)
                .title(Span::styled(" Config ", theme.title)),
        );

    frame.render_widget(Clear, popup);
    frame.render_widget(table, popup);
}

/// Map a table row index (0 = first row inside the border) to a settings cursor value.
/// Returns None for header, hint, and spacer rows.
/// `has_attribution` is true when the theme has author/homepage metadata (adds one extra row
/// between Theme and Layout).
pub fn click_row_to_cursor(table_row: usize, has_attribution: bool) -> Option<usize> {
    if table_row == 0 { return None; } // "── Display" header
    if table_row == 1 { return Some(0); } // Theme
    if has_attribution && table_row == 2 { return None; } // attribution hint
    // All rows below the (optional) attribution row shift by 1 when it is present.
    let r = if has_attribution { table_row - 1 } else { table_row };
    match r {
        2  => Some(1),  // Layout
        3  => Some(2),  // Nerd Font Glyphs
        4  => Some(3),  // ASCII Mode
        5  => None,     // "── Panels" header
        6  => Some(4),  // Disk Panel
        7  => Some(5),  // Network Panel
        8  => Some(6),  // GPU Panel
        9  => Some(7),  // NPU Panel
        10 => Some(8),  // Disk I/O Columns
        11 => None,     // "── Processes" header
        12 => Some(9),  // System Processes
        13 => Some(10), // Tree View
        14 => None,     // "── Network" header
        15 => Some(11), // Hide Virtual Adapters
        16 => Some(12), // Adapter Filters
        17 => None,     // "── General" header
        18 => Some(13), // Refresh Interval
        19 => Some(14), // Clock Format
        20 => None,     // "── Columns" header
        21 => Some(15), // PID
        22 => Some(16), // NAME
        23 => Some(17), // CPU%
        24 => Some(18), // MEM
        25 => Some(19), // MEM%
        26 => Some(20), // THDS
        27 => Some(21), // STATUS
        28 => Some(22), // USER
        29 => Some(23), // DISK-R
        30 => Some(24), // DISK-W
        _  => None,
    }
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

/// Strip common URL prefixes so a homepage fits in the value column.
fn shorten_url(url: &str) -> &str {
    url.trim_start_matches("https://")
       .trim_start_matches("http://")
       .trim_start_matches("www.")
}

fn col_vis(state: &AppState, id: ProcessColumnId) -> String {
    let visible = state.config.process_columns.iter().any(|c| c.id == id && c.visible);
    shown_hidden(visible)
}

fn on_off(v: bool)       -> String { if v { "On".into()     } else { "Off".into()    } }
fn shown_hidden(v: bool) -> String { if v { "Shown".into() } else { "Hidden".into() } }
