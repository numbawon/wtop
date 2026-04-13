//! Process detail overlay — shown when the user presses `i` on a selected process.
//!
//! Layout (top→bottom):
//!   header  — exe path, cmdline, uptime, PE version strings
//!   modules — scrollable list of every loaded DLL (name · base · size)
//!   footer  — scroll position indicator + key hints

use bytesize::ByteSize;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

use crate::models::inspect::ProcessInspectData;
use crate::ui::theme::Theme;
use crate::ui::truncate;

const PANEL_W: u16 = 80;
/// Characters available for values after the 11-char label + 2-char left padding.
const VALUE_W: usize = (PANEL_W as usize) - 16;
/// Maximum panel height (rows). Panel never taller than this even on huge terminals.
const MAX_PANEL_H: usize = 52;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    data: &ProcessInspectData,
    scroll: usize,
    theme: &Theme,
) {
    // ── Build sections ────────────────────────────────────────────────────────
    let header = build_header(data, theme);
    let header_len = header.len();
    let mod_rows = build_module_rows(data, theme);
    let total_mods = mod_rows.len();

    // How many rows the terminal offers us (leave 2 rows margin top + bottom).
    let max_h = (area.height as usize).saturating_sub(4).min(MAX_PANEL_H);

    // Fixed overhead: 2 borders + header lines + 1 module-section title + 3 footer lines.
    let fixed = 2 + header.len() + 1 + 3;
    let visible_mods = max_h.saturating_sub(fixed).min(total_mods);
    let clamped_scroll = scroll.min(total_mods.saturating_sub(visible_mods));

    let panel_h = (fixed + visible_mods) as u16;

    // ── Geometry ──────────────────────────────────────────────────────────────
    let x = area.x + area.width.saturating_sub(PANEL_W) / 2;
    let y = area.y + area.height.saturating_sub(panel_h) / 2;
    let rect = Rect {
        x,
        y,
        width: PANEL_W.min(area.width),
        height: panel_h.min(area.height),
    };

    // ── Assemble lines ────────────────────────────────────────────────────────
    let mut lines: Vec<Line> = header;

    // Module section title
    let mod_title = if total_mods == 0 {
        format!("  Modules  (none)")
    } else if total_mods > visible_mods {
        format!(
            "  Modules  ({} total — {}/{})",
            total_mods,
            clamped_scroll + 1,
            total_mods,
        )
    } else {
        format!("  Modules  ({})", total_mods)
    };
    lines.push(Line::from(Span::styled(mod_title, theme.header)));

    // Visible module slice
    let end = (clamped_scroll + visible_mods).min(total_mods);
    lines.extend_from_slice(&mod_rows[clamped_scroll..end]);

    // Footer
    lines.push(Line::from(""));
    if total_mods > visible_mods {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("↑↓ / PgUp PgDn", theme.text_bright),
            Span::styled("  scroll    ", theme.text_dim),
            Span::styled("[i / Esc]", theme.text_bright),
            Span::styled("  close", theme.text_dim),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("[i / Esc]", theme.text_bright),
            Span::styled("  close", theme.text_dim),
        ]));
    }
    lines.push(Line::from(""));

    // ── Render ────────────────────────────────────────────────────────────────
    let title = format!(" {} — PID {} ", truncate(&data.name, 28), data.pid);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(theme.border_set.clone())
        .border_style(theme.border_focused)
        .title(Span::styled(title, theme.title))
        .style(theme.panel_bg);

    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .alignment(Alignment::Left),
        rect,
    );

    // ── Scrollbar (only when module list overflows) ───────────────────────────
    if total_mods > visible_mods && visible_mods > 0 {
        // Position the scrollbar along the right border, covering only the
        // module rows: skip 1 (top border) + header lines + 1 (section title).
        let mod_area_top = rect.y + 1 + header_len as u16 + 1;
        let mod_area_h = visible_mods as u16;
        let scroll_rect = Rect {
            x: rect.x + rect.width - 1,
            y: mod_area_top,
            width: 1,
            height: mod_area_h,
        };

        let mut sb_state = ScrollbarState::new(total_mods.saturating_sub(visible_mods))
            .position(clamped_scroll);

        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .thumb_style(theme.border_focused)
                .track_style(theme.text_dim),
            scroll_rect,
            &mut sb_state,
        );
    }
}

// ── Header ────────────────────────────────────────────────────────────────────

fn build_header<'a>(data: &ProcessInspectData, theme: &'a Theme) -> Vec<Line<'a>> {
    let lbl = |s: &'static str| -> Span<'a> {
        Span::styled(format!("  {:<10}", s), theme.header)
    };

    let mut lines: Vec<Line<'a>> = Vec::new();
    lines.push(Line::from(""));

    // Exe path — wrap to a second line if truncated
    lines.push(Line::from(vec![
        lbl("Exe"),
        Span::styled(truncate(&data.exe_path, VALUE_W), theme.text_normal),
    ]));
    if data.exe_path.chars().count() > VALUE_W {
        let cont = data.exe_path.chars().skip(VALUE_W - 1).collect::<String>();
        lines.push(Line::from(vec![
            Span::raw(format!("  {:<10}", "")),
            Span::styled(truncate(&cont, VALUE_W), theme.text_dim),
        ]));
    }

    lines.push(Line::from(vec![
        lbl("Cmdline"),
        Span::styled(truncate(&data.cmdline, VALUE_W), theme.text_normal),
    ]));
    if data.cmdline.chars().count() > VALUE_W {
        let cont = data.cmdline.chars().skip(VALUE_W - 1).collect::<String>();
        lines.push(Line::from(vec![
            Span::raw(format!("  {:<10}", "")),
            Span::styled(truncate(&cont, VALUE_W), theme.text_dim),
        ]));
    }

    lines.push(Line::from(vec![
        lbl("Uptime"),
        Span::styled(data.uptime_display(), theme.text_normal),
    ]));

    lines.push(Line::from(""));

    // Version block
    let has_ver = data.file_version.is_some()
        || data.company_name.is_some()
        || data.file_description.is_some();

    if has_ver {
        if let Some(ref v) = data.file_version {
            lines.push(Line::from(vec![
                lbl("Version"),
                Span::styled(
                    v.clone(),
                    Style::default()
                        .fg(theme.text_bright.fg.unwrap_or(ratatui::style::Color::White))
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        }
        if let Some(ref v) = data.product_version {
            if data.file_version.as_deref() != Some(v.as_str()) {
                lines.push(Line::from(vec![
                    lbl("Product"),
                    Span::styled(v.clone(), theme.text_normal),
                ]));
            }
        }
        if let Some(ref v) = data.company_name {
            lines.push(Line::from(vec![
                lbl("Company"),
                Span::styled(v.clone(), theme.text_normal),
            ]));
        }
        if let Some(ref v) = data.file_description {
            lines.push(Line::from(vec![
                lbl("Desc"),
                Span::styled(v.clone(), theme.text_normal),
            ]));
        }
    } else {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "No version resource (system process or access denied)",
                theme.text_dim,
            ),
        ]));
    }

    lines.push(Line::from(""));
    lines
}

// ── Module rows ───────────────────────────────────────────────────────────────

fn build_module_rows(data: &ProcessInspectData, theme: &Theme) -> Vec<Line<'static>> {
    data.modules
        .iter()
        .map(|m| {
            let name = truncate(&m.name, 30);
            let size = if m.size > 0 {
                ByteSize(m.size as u64).to_string()
            } else {
                "?".into()
            };
            let addr = if m.base > 0 {
                format!("0x{:012x}", m.base)
            } else {
                "?".into()
            };
            Line::from(Span::styled(
                format!("  {:<30}  {:>14}  {:>9}", name, addr, size),
                theme.text_normal,
            ))
        })
        .collect()
}
