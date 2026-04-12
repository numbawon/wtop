pub mod cpu_panel;
pub mod disk_panel;
pub mod filter_bar;
pub mod gauge_bar;
pub mod help_overlay;
pub mod kill_confirm;
pub mod layout;
pub mod memory_panel;
pub mod network_panel;
pub mod process_panel;
pub mod theme;
pub mod wt_panel;

use chrono::Local;
use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::{AppState, FocusedPanel};
use crate::glyphs::Glyphs;
use theme::Theme;

/// Main draw function — called every frame by the event loop.
pub fn draw(frame: &mut Frame, state: &AppState) {
    let theme = if state.config.ascii_mode {
        Theme::no_color()
    } else {
        Theme::for_name(&state.config.theme)
    };
    let glyphs = Glyphs::for_config(state.config.nerd_glyphs);
    let area = frame.area();

    let rects = layout::compute(
        area,
        &state.config.layout_mode,
        state.config.show_disk,
        state.config.show_network,
    );

    // --- CPU panel ---
    if let Ok(cpu) = state.hub.cpu.read() {
        cpu_panel::render(
            frame,
            rects.cpu,
            &cpu,
            &theme,
            &glyphs,
            state.focused_panel == FocusedPanel::Cpu,
        );
    }

    // --- Memory panel ---
    if let Ok(mem) = state.hub.memory.read() {
        memory_panel::render(
            frame,
            rects.memory,
            &mem,
            &theme,
            &glyphs,
            state.focused_panel == FocusedPanel::Memory,
        );
    }

    // --- Disk panel ---
    if let (Ok(disks), Some(disk_rect)) = (state.hub.disks.read(), rects.disk) {
        disk_panel::render(
            frame,
            disk_rect,
            &disks,
            &theme,
            &glyphs,
            state.focused_panel == FocusedPanel::Disk,
        );
    }

    // --- Network panel ---
    if let (Ok(nets), Some(net_rect)) = (state.hub.networks.read(), rects.network) {
        network_panel::render(
            frame,
            net_rect,
            &nets,
            &theme,
            &glyphs,
            state.focused_panel == FocusedPanel::Network,
        );
    }

    // --- Process panel ---
    process_panel::render(
        frame,
        rects.processes,
        state,
        &theme,
        &glyphs,
        state.focused_panel == FocusedPanel::Processes,
    );

    // --- Status bar (keybindings + filter) ---
    render_statusbar(frame, rects.statusbar, state, &theme);

    // --- Overlays ---
    if state.show_help {
        help_overlay::render(frame, area, &theme);
    }

    if state.show_kill_confirm {
        if let Some((pid, ref name)) = state.kill_target {
            kill_confirm::render(frame, area, pid, name, &theme);
        }
    }

    if state.show_wt_panel {
        wt_panel::render(frame, area, state, &theme);
    }
}

fn render_statusbar(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let now = Local::now().format("%H:%M:%S").to_string();
    let refresh = format!("{}ms", state.config.refresh_interval_ms);

    if state.filter_active || !state.filter_text.is_empty() {
        filter_bar::render(frame, area, &state.filter_text, state.filter_active, theme);
        return;
    }

    if let Some(ref msg) = state.status_message {
        let line = Line::from(vec![
            Span::styled("  ✗ ", theme.gauge_high),
            Span::styled(msg.as_str(), theme.text_normal),
        ]);
        frame.render_widget(Paragraph::new(line), area);
        return;
    }

    {
        let user_filter_span = if state.user_filter_active {
            Span::styled(" [u:UserFilter] ", theme.filter_active)
        } else {
            Span::styled("", theme.text_dim)
        };

        let line = Line::from(vec![
            Span::styled("q", theme.text_bright),
            Span::styled(":Quit  ", theme.text_dim),
            Span::styled("?", theme.text_bright),
            Span::styled(":Help  ", theme.text_dim),
            Span::styled("↑↓", theme.text_bright),
            Span::styled(":Nav  ", theme.text_dim),
            Span::styled("Enter", theme.text_bright),
            Span::styled(":Expand  ", theme.text_dim),
            Span::styled("f", theme.text_bright),
            Span::styled(":Filter  ", theme.text_dim),
            Span::styled("K", theme.text_bright),
            Span::styled(":Kill  ", theme.text_dim),
            Span::styled("u", theme.text_bright),
            Span::styled(":UserFilter  ", theme.text_dim),
            user_filter_span,
            Span::styled(&refresh, theme.text_dim),
            Span::styled("  ", theme.text_dim),
            Span::styled(&now, theme.text_dim),
        ]);
        let para = Paragraph::new(line);
        frame.render_widget(para, area);
    }
}

/// Truncate a string to at most `max_chars` visible characters.
/// Appends `…` when truncation occurs. Safe for multi-byte Unicode.
pub fn truncate(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let out: String = chars.by_ref().take(max_chars.saturating_sub(1)).collect();
    if chars.next().is_some() {
        format!("{}…", out)
    } else {
        s.to_string()
    }
}
