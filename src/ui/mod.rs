pub mod cpu_panel;
pub mod disk_panel;
pub mod filter_bar;
pub mod help_overlay;
pub mod layout;
pub mod memory_panel;
pub mod network_panel;
pub mod process_panel;
pub mod theme;

use chrono::Local;
use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::{AppState, FocusedPanel};
use theme::Theme;

/// Main draw function — called every frame by the event loop.
pub fn draw(frame: &mut Frame, state: &AppState) {
    let theme = Theme::default_dark();
    let area = frame.area();

    let rects = layout::compute(area);

    // --- CPU panel ---
    if let Ok(cpu) = state.hub.cpu.read() {
        cpu_panel::render(
            frame,
            rects.cpu,
            &cpu,
            &theme,
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
            state.focused_panel == FocusedPanel::Memory,
        );
    }

    // --- Disk panel ---
    if let Ok(disks) = state.hub.disks.read() {
        disk_panel::render(
            frame,
            rects.disk,
            &disks,
            &theme,
            state.focused_panel == FocusedPanel::Disk,
        );
    }

    // --- Network panel ---
    if let Ok(nets) = state.hub.networks.read() {
        network_panel::render(
            frame,
            rects.network,
            &nets,
            &theme,
            state.focused_panel == FocusedPanel::Network,
        );
    }

    // --- Process panel ---
    process_panel::render(
        frame,
        rects.processes,
        state,
        &theme,
        state.focused_panel == FocusedPanel::Processes,
    );

    // --- Status bar (keybindings + filter) ---
    render_statusbar(frame, rects.statusbar, state, &theme);

    // --- Overlays ---
    if state.show_help {
        help_overlay::render(frame, area, &theme);
    }
}

fn render_statusbar(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let now = Local::now().format("%H:%M:%S").to_string();
    let refresh = format!("{}ms", state.config.refresh_interval_ms);

    if state.filter_active || !state.filter_text.is_empty() {
        filter_bar::render(frame, area, &state.filter_text, state.filter_active, theme);
    } else {
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
            Span::styled("k", theme.text_bright),
            Span::styled(":Kill  ", theme.text_dim),
            Span::styled(&refresh, theme.text_dim),
            Span::styled("  ", theme.text_dim),
            Span::styled(&now, theme.text_dim),
        ]);
        let para = Paragraph::new(line);
        frame.render_widget(para, area);
    }
}
