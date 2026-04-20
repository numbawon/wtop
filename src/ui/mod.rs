pub mod cpu_panel;
pub mod theme_file;
pub mod name_search;
pub mod net_filter_panel;
pub mod pid_jump;
pub mod disk_panel;
pub mod gpu_panel;
pub mod services_panel;
pub mod filter_bar;
pub mod gauge_bar;
pub mod help_overlay;
pub mod inspect_panel;
pub mod kill_confirm;
pub mod layout;
pub mod memory_panel;
pub mod network_panel;
pub mod process_panel;
pub mod theme;
pub mod settings_panel;
pub mod wt_panel;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::{AppState, FocusedPanel};
use crate::glyphs::Glyphs;
use theme::Theme;

/// Main draw function - called every frame by the event loop.
pub fn draw(frame: &mut Frame, state: &AppState) {
    let no_color_theme;
    let theme = if state.config.ascii_mode {
        no_color_theme = Theme::no_color();
        &no_color_theme
    } else {
        &state.theme_cache
    };
    let glyphs = Glyphs::for_config(state.config.nerd_glyphs);
    let area = frame.area();

    let rects = layout::compute(
        area,
        &state.config.layout_mode,
        state.config.show_disk,
        state.config.show_network,
        state.config.show_gpu,
    );

    if let Ok(cpu) = state.hub.cpu.read() {
        cpu_panel::render(
            frame,
            rects.cpu,
            &cpu,
            theme,
            &glyphs,
            state.focused_panel == FocusedPanel::Cpu,
        );
    }

    if let Ok(mem) = state.hub.memory.read() {
        memory_panel::render(
            frame,
            rects.memory,
            &mem,
            theme,
            &glyphs,
            state.focused_panel == FocusedPanel::Memory,
        );
    }

    if let (Ok(disks), Some(disk_rect)) = (state.hub.disks.read(), rects.disk) {
        disk_panel::render(
            frame,
            disk_rect,
            &disks,
            theme,
            &glyphs,
            state.focused_panel == FocusedPanel::Disk,
        );
    }

    if let (Ok(gpus), Ok(npus), Some(gpu_rect)) = (
        state.hub.gpus.read(),
        state.hub.npus.read(),
        rects.gpu,
    ) {
        let visible_npus: &[_] = if state.config.show_npu { &npus } else { &[] };
        gpu_panel::render(
            frame,
            gpu_rect,
            &gpus,
            visible_npus,
            theme,
            &glyphs,
            state.focused_panel == FocusedPanel::Gpu,
        );
    }

    if let (Ok(nets), Some(net_rect)) = (state.hub.networks.read(), rects.network) {
        network_panel::render(
            frame,
            net_rect,
            &nets,
            state,
            theme,
            &glyphs,
            state.focused_panel == FocusedPanel::Network,
        );
    }

    process_panel::render(
        frame,
        rects.processes,
        state,
        theme,
        &glyphs,
        state.focused_panel == FocusedPanel::Processes,
    );

    render_statusbar(frame, rects.statusbar, state, theme);

    if state.show_help {
        help_overlay::render(frame, area, theme);
    }

    if state.show_kill_confirm {
        if let Some((pid, ref name)) = state.kill_target {
            kill_confirm::render(frame, area, pid, name, theme);
        }
    }

    if state.show_wt_panel {
        wt_panel::render(frame, area, state, theme);
    }

    if state.show_settings {
        settings_panel::render(frame, area, state, theme);
    }

    if state.show_inspect {
        if let Some(ref data) = state.inspect_data {
            inspect_panel::render(frame, area, data, state, theme);
        }
    }

    if state.show_pid_jump {
        pid_jump::render(frame, area, &state.pid_jump_text, state.pid_jump_not_found, theme);
    }

    if state.show_name_search {
        name_search::render(frame, area, &state.name_search_text, state.name_search_not_found, theme);
    }

    if state.show_net_filter {
        if let Ok(nets) = state.hub.networks.read() {
            net_filter_panel::render(
                frame,
                area,
                &nets,
                &state.config.hidden_adapters,
                state.net_filter_cursor,
                theme,
            );
        }
    }

    if state.show_services {
        if let Ok(svcs) = state.hub.services.read() {
            services_panel::render(
                frame,
                area,
                &svcs,
                state.services_cursor,
                &state.services_filter,
                theme,
            );
        }
    }
}

fn render_statusbar(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let now = &state.cached_time;
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
        frame.render_widget(Paragraph::new(line).style(theme.panel_bg), area);
        return;
    }

    {
        // 24h "HH:MM:SS" = 8 chars; 12h "HH:MM:SS AM" = 11 chars. Add 1 padding each.
        let clock_w = if state.config.time_24h { 9u16 } else { 12u16 };
        let halves = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(clock_w)])
            .split(area);
        let left_area  = halves[0];
        let right_area = halves[1];

        let user_filter_span = if state.user_filter_active {
            Span::styled(" [u:UserFilter] ", theme.filter_active)
        } else {
            Span::styled("", theme.text_dim)
        };

        let left_line = Line::from(vec![
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
            Span::styled("u", theme.text_bright),
            Span::styled(":UserFilter  ", theme.text_dim),
            Span::styled("K", theme.text_bright),
            Span::styled(":Kill  ", theme.text_dim),
            Span::styled("y", theme.text_bright),
            Span::styled(":Copy  ", theme.text_dim),
            user_filter_span,
            Span::styled(&refresh, theme.text_dim),
        ]);
        frame.render_widget(Paragraph::new(left_line).style(theme.panel_bg), left_area);

        let right_line = Line::from(Span::styled(now.as_str(), theme.text_dim));
        frame.render_widget(
            Paragraph::new(right_line).style(theme.panel_bg).alignment(Alignment::Right),
            right_area,
        );
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
