//! Windows Terminal info panel — shows WT detection status, current profile,
//! font face, and offers to apply a Nerd Font. Opened with `w`, closed with
//! Esc / q / w. Font apply triggered with `f`, confirmed with Enter.

use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::AppState;
use crate::ui::theme::Theme;
use crate::wt::NERD_FONT_FACE;

const PANEL_WIDTH: u16 = 62;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let info = &state.wt_info;

    // Build content lines first so we can size the panel.
    let mut lines: Vec<Line> = Vec::new();

    // ── Detection status ─────────────────────────────────────────────────────
    lines.push(Line::from(""));
    if info.detected {
        lines.push(Line::from(vec![
            Span::raw("  Status:   "),
            Span::styled(
                "Running inside Windows Terminal",
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::raw("  Status:   "),
            Span::styled("Not running inside Windows Terminal", theme.text_dim),
        ]));
    }

    // ── Profile & font (only when detected) ──────────────────────────────────
    if info.detected {
        let profile_display = info
            .profile_name
            .as_deref()
            .or(info.profile_id.as_deref())
            .unwrap_or("(default)");

        lines.push(Line::from(vec![
            Span::raw("  Profile:  "),
            Span::styled(profile_display, theme.text_normal),
        ]));

        let font_display = info.font_face.as_deref().unwrap_or("(not set)");
        lines.push(Line::from(vec![
            Span::raw("  Font:     "),
            Span::styled(font_display, theme.text_normal),
        ]));

        if let Some(ref path) = info.settings_path {
            let path_str = path.to_string_lossy();
            // Truncate from the left if the path is very long.
            let max_path = (PANEL_WIDTH as usize).saturating_sub(14);
            let display_path = if path_str.len() > max_path {
                format!("…{}", &path_str[path_str.len() - max_path..])
            } else {
                path_str.into_owned()
            };
            lines.push(Line::from(vec![
                Span::raw("  Settings: "),
                Span::styled(display_path, theme.text_dim),
            ]));
        }

        // ── Read error warning ──────────────────────────────────────────────
        if let Some(ref err) = info.read_error {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("  ✗ ", theme.gauge_high),
                Span::styled(err.as_str(), theme.gauge_high),
            ]));
        }

        // ── Nerd Font status / action ───────────────────────────────────────
        lines.push(Line::from(""));
        if info.has_nerd_font() {
            lines.push(Line::from(vec![
                Span::styled(
                    "  ✓ ",
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "Nerd Font glyphs available in current font",
                    Style::default().fg(Color::Green),
                ),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled("  ○ ", theme.text_dim),
                Span::styled(
                    "Font does not appear to include Nerd Font glyphs",
                    theme.text_normal,
                ),
            ]));
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("  [f] ", theme.text_bright),
                Span::styled("Apply font: ", theme.text_normal),
                Span::styled(
                    NERD_FONT_FACE,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  [w / Esc] ", theme.text_dim),
        Span::styled("Close", theme.text_dim),
    ]));
    lines.push(Line::from(""));

    // ── Size and position panel ───────────────────────────────────────────────
    let content_height = lines.len() as u16 + 2; // +2 for block borders
    let width = PANEL_WIDTH.min(area.width);
    let height = content_height.min(area.height);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(theme.border_set.clone())
        .border_style(theme.border_focused)
        .title(Span::styled(" Windows Terminal ", theme.title));

    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines).block(block).style(theme.panel_bg),
        popup,
    );

    // ── Nerd Font confirmation sub-dialog ─────────────────────────────────────
    if state.wt_nerd_font_confirm {
        render_nerd_font_confirm(frame, popup, theme);
    }
}

fn render_nerd_font_confirm(frame: &mut Frame, parent: Rect, theme: &Theme) {
    let confirm_lines: Vec<Line> = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Apply this font to your Windows Terminal settings?",
            theme.text_normal,
        )]),
        Line::from(""),
        Line::from(vec![
            Span::raw("  Font: "),
            Span::styled(
                NERD_FONT_FACE,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  settings.json will be rewritten.",
            theme.text_dim,
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [Enter] ", theme.text_bright),
            Span::styled("Apply  ", theme.text_normal),
            Span::styled("  [Esc / n] ", theme.text_dim),
            Span::styled("Cancel", theme.text_dim),
        ]),
        Line::from(""),
    ];

    let width = 56u16.min(parent.width.saturating_sub(2));
    let height = (confirm_lines.len() as u16 + 2).min(parent.height.saturating_sub(2));
    let x = parent.x + (parent.width.saturating_sub(width)) / 2;
    let y = parent.y + (parent.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(theme.border_set.clone())
        .border_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .title(Span::styled(
            " Confirm ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center);

    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(confirm_lines).block(block).style(theme.panel_bg),
        popup,
    );
}
