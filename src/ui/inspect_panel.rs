//! Process detail overlay - shown when the user presses `i` on a selected process.
//!
//! Layout (top→bottom):
//!   tab bar   - [Info] [Modules] [Handles] [Network] [Env]
//!   content   - scrollable section for the active tab
//!   footer    - scroll indicator + key hints

use bytesize::ByteSize;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

use crate::app::{AppState, InspectTab};
use crate::models::inspect::ProcessInspectData;
use crate::models::thread::{wait_reason_label, ThreadState};
use crate::ui::theme::Theme;
use crate::ui::truncate;

const PANEL_W: u16 = 100;
/// Characters available for values after the 14-char label + 2-char left padding.
const VALUE_W: usize = (PANEL_W as usize) - 18;
/// Maximum panel height.
const MAX_PANEL_H: usize = 54;

/// Number of copyable rows on the Info tab (must match `clipboard::info_copyable_values`).
/// Exe, Cmdline, Parent, FileVersion, Company, Desc, WindowTitle - 7 max but most optional.
/// The actual count is data-driven; we expose the upper bound for cursor clamping.
pub const INFO_COPYABLE_COUNT: usize = 7;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    data: &ProcessInspectData,
    state: &AppState,
    theme: &Theme,
) {
    let tab       = state.inspect_tab;
    let _scroll   = state.inspect_scroll;
    let h_cursor  = state.inspect_handle_cursor;
    let m_cursor  = state.inspect_module_cursor;
    let e_cursor  = state.inspect_env_cursor;
    let i_cursor  = state.inspect_info_cursor;
    let t_cursor  = state.inspect_thread_cursor;
    let n_cursor  = state.inspect_network_cursor;
    let h_confirm = state.inspect_close_confirm;
    let h_offset  = state.inspect_h_offset;

    // ── Build content for the active tab ──────────────────────────────────────
    let (content_rows, total_scrollable): (Vec<Line>, usize) = match tab {
        InspectTab::Info => {
            let rows = build_info_rows(data, i_cursor, h_offset, theme);
            let n = rows.len();
            (rows, n)
        }
        InspectTab::Threads => {
            let rows = build_thread_rows(data, t_cursor, theme);
            let n = rows.len();
            (rows, n)
        }
        InspectTab::Modules => {
            let rows = build_module_rows(data, m_cursor, h_offset, theme);
            let n = rows.len();
            (rows, n)
        }
        InspectTab::Handles => {
            let rows = build_handle_rows(data, h_cursor, h_offset, theme);
            let n = rows.len();
            (rows, n)
        }
        InspectTab::Network => {
            let rows = build_network_rows(data, n_cursor, theme);
            let n = rows.len();
            (rows, n)
        }
        InspectTab::Env => {
            let rows = build_env_rows(data, e_cursor, h_offset, theme);
            let n = rows.len();
            (rows, n)
        }
    };

    // ── Geometry ──────────────────────────────────────────────────────────────
    let fixed = 2 + 2 + 3; // 2 borders + 2 tab bar lines + 3 footer
    let max_h = (area.height as usize).saturating_sub(4).min(MAX_PANEL_H);
    let visible_rows = max_h.saturating_sub(fixed).min(total_scrollable.max(1));

    // Cursor-driven scroll for cursor-based tabs
    let clamped_scroll = match tab {
        InspectTab::Handles => {
            if data.open_handles.is_empty() { 0 }
            else {
                h_cursor.saturating_sub(visible_rows.saturating_sub(1) / 2)
                    .min(total_scrollable.saturating_sub(visible_rows))
            }
        }
        InspectTab::Modules => {
            if data.modules.is_empty() { 0 }
            else {
                m_cursor.saturating_sub(visible_rows.saturating_sub(1) / 2)
                    .min(total_scrollable.saturating_sub(visible_rows))
            }
        }
        InspectTab::Env => {
            if data.env_vars.is_empty() { 0 }
            else {
                e_cursor.saturating_sub(visible_rows.saturating_sub(1) / 2)
                    .min(total_scrollable.saturating_sub(visible_rows))
            }
        }
        InspectTab::Threads => {
            if data.threads.is_empty() { 0 }
            else {
                t_cursor.saturating_sub(visible_rows.saturating_sub(1) / 2)
                    .min(total_scrollable.saturating_sub(visible_rows))
            }
        }
        InspectTab::Network => {
            if data.open_connections.is_empty() { 0 }
            else {
                // header = 2 rows, then connections; cursor + 2 header offset
                let offset_cursor = n_cursor + 2;
                offset_cursor.saturating_sub(visible_rows.saturating_sub(1) / 2)
                    .min(total_scrollable.saturating_sub(visible_rows))
            }
        }
        InspectTab::Info => {
            i_cursor.saturating_sub(visible_rows.saturating_sub(1) / 2)
                .min(total_scrollable.saturating_sub(visible_rows))
        }
    };

    let panel_h = (fixed + visible_rows.min(total_scrollable.max(1))) as u16;

    let x = area.x + area.width.saturating_sub(PANEL_W) / 2;
    let y = area.y + area.height.saturating_sub(panel_h) / 2;
    let rect = Rect {
        x,
        y,
        width: PANEL_W.min(area.width),
        height: panel_h.min(area.height),
    };

    // ── Assemble lines ────────────────────────────────────────────────────────
    let mut lines: Vec<Line> = Vec::new();

    lines.push(build_tab_bar(tab, theme));
    lines.push(Line::from(Span::styled(
        "─".repeat(PANEL_W as usize - 2),
        theme.text_dim,
    )));

    let end = (clamped_scroll + visible_rows).min(total_scrollable);
    let slice = if total_scrollable > 0 { &content_rows[clamped_scroll..end] } else { &[] };
    lines.extend_from_slice(slice);

    for _ in lines.len()..fixed + visible_rows - 3 {
        lines.push(Line::from(""));
    }

    lines.push(Line::from(""));
    lines.push(build_footer(tab, total_scrollable > visible_rows, h_offset > 0, theme));
    lines.push(Line::from(""));

    // ── Render panel ──────────────────────────────────────────────────────────
    let title = format!(" {} - PID {} ", truncate(&data.name, 28), data.pid);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(theme.border_set)
        .border_style(theme.border_focused)
        .title(Span::styled(title, theme.title))
        .style(theme.panel_bg);

    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines).block(block).alignment(Alignment::Left),
        rect,
    );

    // Scrollbar
    if total_scrollable > visible_rows && visible_rows > 0 {
        let content_top = rect.y + 1 + 2;
        let scroll_rect = Rect {
            x: rect.x + rect.width - 1,
            y: content_top,
            width: 1,
            height: visible_rows as u16,
        };
        let pos = match tab {
            InspectTab::Handles => h_cursor,
            InspectTab::Modules => m_cursor,
            InspectTab::Env     => e_cursor,
            InspectTab::Info    => i_cursor,
            InspectTab::Threads => t_cursor,
            InspectTab::Network => n_cursor.saturating_add(2),
        };
        let mut sb_state = ScrollbarState::new(total_scrollable.saturating_sub(visible_rows))
            .position(pos.min(total_scrollable.saturating_sub(visible_rows)));
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

    if h_confirm {
        if let Some(entry) = data.open_handles.get(h_cursor) {
            render_close_confirm(frame, rect, entry.handle_value, &entry.name, &entry.type_name, theme);
        }
    }
}

fn build_tab_bar(active: InspectTab, theme: &Theme) -> Line<'static> {
    let s = |tab| if active == tab { theme.header } else { theme.text_dim };
    Line::from(vec![
        Span::raw("  "),
        Span::styled("[ Info ]",    s(InspectTab::Info)),
        Span::raw("  "),
        Span::styled("[ Threads ]", s(InspectTab::Threads)),
        Span::raw("  "),
        Span::styled("[ Modules ]", s(InspectTab::Modules)),
        Span::raw("  "),
        Span::styled("[ Handles ]", s(InspectTab::Handles)),
        Span::raw("  "),
        Span::styled("[ Network ]", s(InspectTab::Network)),
        Span::raw("  "),
        Span::styled("[ Env ]",     s(InspectTab::Env)),
        Span::raw("  "),
        Span::styled("Tab", theme.text_dim),
    ])
}

fn build_footer<'a>(tab: InspectTab, has_scroll: bool, has_h_scroll: bool, theme: &'a Theme) -> Line<'a> {
    let mut spans: Vec<Span> = vec![Span::raw("  ")];

    if has_scroll {
        spans.push(Span::styled("↑↓ PgUp/Dn", theme.text_bright));
        spans.push(Span::styled("  scroll    ", theme.text_dim));
    }
    if has_h_scroll || matches!(tab, InspectTab::Modules | InspectTab::Env | InspectTab::Info) {
        spans.push(Span::styled("←→", theme.text_bright));
        spans.push(Span::styled("  pan    ", theme.text_dim));
    }
    if tab == InspectTab::Handles {
        spans.push(Span::styled("[x]", theme.text_bright));
        spans.push(Span::styled("  force-close    ", theme.text_dim));
    }

    spans.push(Span::styled("[y]", theme.text_bright));
    spans.push(Span::styled("  copy    ", theme.text_dim));
    spans.push(Span::styled("[Tab]", theme.text_bright));
    spans.push(Span::styled("  switch tab    ", theme.text_dim));
    spans.push(Span::styled("[i/Esc]", theme.text_bright));
    spans.push(Span::styled("  close", theme.text_dim));

    Line::from(spans)
}

// ── Horizontal clip helper ────────────────────────────────────────────────────

/// Clip `s` to `width` chars starting at `offset`, padding with spaces if needed.
fn hclip(s: &str, offset: usize, width: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    let start = offset.min(chars.len());
    let slice: String = chars[start..].iter().take(width).collect();
    slice
}

// ── Info tab ──────────────────────────────────────────────────────────────────

fn build_info_rows<'a>(
    data: &ProcessInspectData,
    cursor: usize,
    h_offset: usize,
    theme: &'a Theme,
) -> Vec<Line<'a>> {
    // We build a list of (label, full_value, is_copyable) tuples first, then render.
    let val = |s: String| -> Span<'a> { Span::styled(s, theme.text_normal) };
    let dim = |s: String| -> Span<'a> { Span::styled(s, theme.text_dim) };

    // Track which row index is "copyable" so we can match cursor position.
    let mut copyable_idx: usize = 0;
    let mut lines: Vec<Line<'a>> = Vec::new();

    let mut push_copyable = |lines: &mut Vec<Line<'a>>, label: &'static str, full_val: &str, bright: bool| {
        let selected = copyable_idx == cursor;
        copyable_idx += 1;
        let row_idx = lines.len();
        let _ = row_idx;
        let val_str = hclip(full_val, h_offset, VALUE_W);
        let marker = if selected { "▶ " } else { "  " };
        let val_style = if selected {
            theme.row_selected
        } else if bright {
            Style::default()
                .fg(theme.text_bright.fg.unwrap_or(ratatui::style::Color::White))
                .add_modifier(Modifier::BOLD)
        } else {
            theme.text_normal
        };
        lines.push(Line::from(vec![
            Span::styled(marker, theme.text_dim),
            Span::styled(format!("{:<12}", label), theme.header),
            Span::styled(val_str, val_style),
        ]));
        // If value is longer than VALUE_W, show continuation hint
        if full_val.chars().count() > h_offset + VALUE_W {
            let cont = hclip(full_val, h_offset + VALUE_W, VALUE_W);
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::raw(format!("{:<12}", "")),
                Span::styled(cont, theme.text_dim),
            ]));
        }
    };

    lines.push(Line::from(""));

    push_copyable(&mut lines, "Exe", &data.exe_path, false);
    push_copyable(&mut lines, "Cmdline", &data.cmdline, false);

    lines.push(Line::from(""));

    // Non-copyable stats row
    lines.push(Line::from(vec![
        Span::raw("    "),
        Span::styled(format!("{:<12}", "Started"), theme.header),
        val(if data.start_time_str.is_empty() { "?".into() } else { data.start_time_str.clone() }),
        Span::styled("    Uptime  ", theme.header),
        val(data.uptime_display()),
    ]));

    let arch_str = if data.arch_x86 { "x86 (WoW64)" } else { "x64 native" };
    lines.push(Line::from(vec![
        Span::raw("    "),
        Span::styled(format!("{:<12}", "Arch"), theme.header),
        val(arch_str.into()),
        Span::styled("    Priority ", theme.header),
        val(data.priority_class.clone()),
    ]));

    // Memory row
    let ws    = ByteSize(data.mem_working_set).to_string();
    let peak  = ByteSize(data.mem_peak_ws).to_string();
    let faults = data.mem_page_faults;
    lines.push(Line::from(vec![
        Span::raw("    "),
        Span::styled(format!("{:<12}", "Memory"), theme.header),
        val(format!("WS {}  Peak {}  Faults {}", ws, peak, faults)),
    ]));

    // CPU time row
    let user_ms   = data.cpu_user_ms;
    let kern_ms   = data.cpu_kernel_ms;
    lines.push(Line::from(vec![
        Span::raw("    "),
        Span::styled(format!("{:<12}", "CPU Time"), theme.header),
        val(format!("User {}  Kernel {}", fmt_ms(user_ms), fmt_ms(kern_ms))),
    ]));

    // Parent
    let parent_str = if data.parent_pid > 0 {
        format!("{} (PID {})", data.parent_name, data.parent_pid)
    } else {
        "?".into()
    };
    push_copyable(&mut lines, "Parent", &parent_str, false);

    // Integrity
    lines.push(Line::from(vec![
        Span::raw("    "),
        Span::styled(format!("{:<12}", "Integrity"), theme.header),
        val(data.integrity.to_string()),
    ]));

    // Window title
    if let Some(ref title) = data.window_title {
        push_copyable(&mut lines, "Window", title, false);
    }

    lines.push(Line::from(""));

    // Mitigations
    let dep  = fmt_flag(data.dep_enabled);
    let aslr = fmt_flag(data.aslr_enabled);
    let cfg  = fmt_flag(data.cfg_enabled);
    lines.push(Line::from(vec![
        Span::raw("    "),
        Span::styled(format!("{:<12}", "Mitigations"), theme.header),
        Span::styled(format!("DEP:{dep}  ASLR:{aslr}  CFG:{cfg}"), theme.text_normal),
    ]));

    lines.push(Line::from(""));

    // Version block
    let has_ver = data.file_version.is_some()
        || data.company_name.is_some()
        || data.file_description.is_some();

    if has_ver {
        if let Some(ref v) = data.file_version {
            push_copyable(&mut lines, "Version", v, true);
        }
        if let Some(ref v) = data.product_version {
            if data.file_version.as_deref() != Some(v.as_str()) {
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(format!("{:<12}", "Product"), theme.header),
                    val(v.clone()),
                ]));
            }
        }
        if let Some(ref v) = data.company_name {
            push_copyable(&mut lines, "Company", v, false);
        }
        if let Some(ref v) = data.file_description {
            push_copyable(&mut lines, "Desc", v, false);
        }
    } else {
        lines.push(Line::from(vec![
            Span::raw("  "),
            dim("No version resource (system process or access denied)".into()),
        ]));
    }

    lines.push(Line::from(""));
    lines
}

fn fmt_flag(v: Option<bool>) -> &'static str {
    match v {
        Some(true)  => "on ",
        Some(false) => "off",
        None        => "?  ",
    }
}

fn fmt_ms(ms: u64) -> String {
    let total_secs = ms / 1000;
    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    if h > 0 { format!("{}h{:02}m{:02}s", h, m, s) }
    else if m > 0 { format!("{}m{:02}s", m, s) }
    else { format!("{}.{:03}s", s, ms % 1000) }
}

// ── Modules tab ───────────────────────────────────────────────────────────────

fn build_module_rows<'a>(
    data: &ProcessInspectData,
    cursor: usize,
    h_offset: usize,
    theme: &'a Theme,
) -> Vec<Line<'a>> {
    if data.modules.is_empty() {
        return vec![Line::from(Span::styled(
            "  (no modules accessible)",
            theme.text_dim,
        ))];
    }
    data.modules
        .iter()
        .enumerate()
        .flat_map(|(i, m)| {
            let selected = i == cursor;
            let row_style = if selected { theme.row_selected } else { theme.text_normal };
            let marker = if selected { "▶ " } else { "  " };
            let size = if m.size > 0 { ByteSize(m.size as u64).to_string() } else { "?".into() };
            let addr = if m.base > 0 { format!("0x{:012x}", m.base) } else { "?".into() };

            // Name row
            let name_row = Line::from(Span::styled(
                format!("{}{:<32}  {:>14}  {:>9}", marker, truncate(&m.name, 32), addr, size),
                row_style,
            ));

            // Path row (indented, horizontally scrollable)
            let path_display = hclip(&m.path, h_offset, (PANEL_W as usize) - 6);
            let path_style = if selected { theme.row_selected } else { theme.text_dim };
            let path_row = Line::from(Span::styled(
                format!("    {}", path_display),
                path_style,
            ));

            vec![name_row, path_row]
        })
        .collect()
}

// ── Handles tab ───────────────────────────────────────────────────────────────

fn build_handle_rows<'a>(
    data: &ProcessInspectData,
    cursor: usize,
    h_offset: usize,
    theme: &'a Theme,
) -> Vec<Line<'a>> {
    if data.open_handles.is_empty() {
        return vec![Line::from(Span::styled(
            "  (no handles accessible - run as Administrator for full list)",
            theme.text_dim,
        ))];
    }

    data.open_handles
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let selected = i == cursor;
            let style = if selected { theme.row_selected } else { theme.text_normal };
            let marker = if selected { "▶ " } else { "  " };
            let hv     = format!("0x{:04x}", h.handle_value);
            let tname  = truncate(&h.type_name, 12);
            let name_w = (PANEL_W as usize).saturating_sub(2 + 8 + 2 + 12 + 2 + 2);
            let raw_name = if h.name.is_empty() { "-".to_string() } else { h.name.clone() };
            let name = hclip(&raw_name, h_offset, name_w);
            Line::from(Span::styled(
                format!("{}{:<8}  {:<12}  {}", marker, hv, tname, name),
                style,
            ))
        })
        .collect()
}

// ── Threads tab ───────────────────────────────────────────────────────────────

fn build_thread_rows<'a>(
    data: &ProcessInspectData,
    cursor: usize,
    theme: &'a Theme,
) -> Vec<Line<'a>> {
    if data.threads.is_empty() {
        return vec![Line::from(Span::styled(
            "  (no threads accessible)",
            theme.text_dim,
        ))];
    }

    let mut rows = Vec::new();
    rows.push(Line::from(Span::styled(
        format!("  {:<8}  {:<4}  {:<16}  {:<18}  {}", "TID", "PRI", "CPU%", "STATE", "MODULE"),
        theme.header,
    )));
    rows.push(Line::from(Span::styled(
        format!("  {}", "─".repeat(PANEL_W as usize - 4)),
        theme.text_dim,
    )));

    for (i, t) in data.threads.iter().enumerate() {
        let selected = i == cursor;
        let style = if t.suspicious {
            theme.row_suspicious
        } else if selected {
            theme.row_selected
        } else {
            theme.text_normal
        };
        let marker = if selected { "▶ " } else { "  " };
        let cpu = if t.kernel_ms > 0 || t.user_ms > 0 {
            format!("{:.1}%", t.cpu_pct.min(99.9))
        } else {
            "---".into()
        };
        let state_str = match t.state {
            ThreadState::Waiting => format!("Wait:{}", wait_reason_label(t.wait_reason)),
            _ => t.state.to_string(),
        };
        let name_part = t.name.as_deref().map(|n| format!("[{}] ", truncate(n, 16))).unwrap_or_default();
        let suspicious_mark = if t.suspicious { " !" } else { "" };
        rows.push(Line::from(Span::styled(
            format!(
                "{}{:<8}  {:>4}  {:>6}  {:<18}  {}{}{}",
                marker, t.tid, t.priority, cpu,
                truncate(&state_str, 18),
                name_part,
                truncate(&t.start_module, 20),
                suspicious_mark,
            ),
            style,
        )));
    }

    rows
}

// ── Network tab ───────────────────────────────────────────────────────────────

fn build_network_rows<'a>(data: &ProcessInspectData, cursor: usize, theme: &'a Theme) -> Vec<Line<'a>> {
    if data.open_connections.is_empty() {
        return vec![Line::from(Span::styled(
            "  (no network connections)",
            theme.text_dim,
        ))];
    }

    let mut rows = Vec::new();
    rows.push(Line::from(Span::styled(
        format!("  {:<6}  {:<26}  {:<26}  {}", "PROTO", "LOCAL", "REMOTE", "STATE"),
        theme.header,
    )));
    rows.push(Line::from(Span::styled(
        format!("  {}", "─".repeat(PANEL_W as usize - 4)),
        theme.text_dim,
    )));

    for (i, conn) in data.open_connections.iter().enumerate() {
        let selected = i == cursor;
        let local  = truncate(&conn.local_addr, 26);
        let remote = if conn.remote_addr.is_empty() { "-".to_string() } else { truncate(&conn.remote_addr, 26) };
        let state  = conn.state;
        let style = if selected {
            theme.row_selected
        } else {
            match state {
                "ESTABLISHED" => theme.text_bright,
                "LISTEN"      => theme.text_normal,
                _             => theme.text_dim,
            }
        };
        let marker = if selected { "▶ " } else { "  " };
        rows.push(Line::from(Span::styled(
            format!("{}{:<6}  {:<26}  {:<26}  {}", marker, conn.proto, local, remote, state),
            style,
        )));
    }

    rows
}

// ── Env tab ───────────────────────────────────────────────────────────────────

fn build_env_rows<'a>(
    data: &ProcessInspectData,
    cursor: usize,
    h_offset: usize,
    theme: &'a Theme,
) -> Vec<Line<'a>> {
    if data.env_vars.is_empty() {
        return vec![Line::from(Span::styled(
            "  (no environment variables accessible)",
            theme.text_dim,
        ))];
    }

    let key_w   = 28usize;
    let val_w   = (PANEL_W as usize).saturating_sub(key_w + 8);

    data.env_vars
        .iter()
        .enumerate()
        .map(|(i, (k, v))| {
            let selected = i == cursor;
            let marker   = if selected { "▶ " } else { "  " };
            let key_str  = truncate(k, key_w);
            let val_clip = hclip(v, h_offset, val_w);
            let key_style = if selected { theme.row_selected } else { theme.header };
            let val_style = if selected { theme.row_selected } else { theme.text_normal };
            Line::from(vec![
                Span::styled(marker, theme.text_dim),
                Span::styled(format!("{:<width$}", key_str, width = key_w), key_style),
                Span::styled("  ", theme.text_dim),
                Span::styled(val_clip, val_style),
            ])
        })
        .collect()
}

// ── Force-close confirm dialog ────────────────────────────────────────────────

fn render_close_confirm(
    frame: &mut Frame,
    parent: Rect,
    handle_value: u64,
    name: &str,
    type_name: &str,
    theme: &Theme,
) {
    let w: u16 = (PANEL_W - 8).min(parent.width);
    let h: u16 = 7;
    let x = parent.x + parent.width.saturating_sub(w) / 2;
    let y = parent.y + parent.height.saturating_sub(h) / 2;
    let rect = Rect { x, y, width: w, height: h };

    let inner_w = (w as usize).saturating_sub(4);
    let path_display = if name.is_empty() {
        format!("Type: {type_name}")
    } else {
        truncate(name, inner_w)
    };

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("Handle: 0x{handle_value:04x}  {path_display}"), theme.text_normal),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "Closing this handle may corrupt the process.",
                theme.gauge_high,
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("[Enter]", theme.text_bright),
            Span::styled(" confirm    ", theme.text_dim),
            Span::styled("[Esc]", theme.text_bright),
            Span::styled(" cancel", theme.text_dim),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(theme.border_set)
        .border_style(theme.gauge_high)
        .title(Span::styled(" Close Handle? ", theme.gauge_high))
        .style(theme.panel_bg);

    frame.render_widget(Clear, rect);
    frame.render_widget(Paragraph::new(lines).block(block), rect);
}
