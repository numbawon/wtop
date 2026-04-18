use std::collections::HashMap;
use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind, MouseButton, MouseEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{OpenProcess, OpenProcessToken, TerminateProcess, PROCESS_TERMINATE};

use crate::collectors::CollectorHub;
use crate::config::{Config, ProcessColumnId};
use windows::Win32::Security::{
    AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES,
    SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows::Win32::System::Threading::GetCurrentProcess;
use chrono::Local;
use crate::input::handler::{handle_key, AppAction, ModalState};
use crate::models::process::{sort_processes, ProcessSortField, SortState};
use crate::models::inspect::ProcessInspectData;
use crate::wt::WtInfo;

/// Which tab is active inside the inspect overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectTab {
    Info,
    Threads,
    Modules,
    Handles,
    Network,
    Env,
}

impl InspectTab {
    pub fn next(self) -> Self {
        match self {
            Self::Info    => Self::Threads,
            Self::Threads => Self::Modules,
            Self::Modules => Self::Handles,
            Self::Handles => Self::Network,
            Self::Network => Self::Env,
            Self::Env     => Self::Info,
        }
    }
}

/// Which panel has keyboard focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusedPanel {
    Cpu,
    Memory,
    Disk,
    Network,
    Gpu,
    Processes,
}

impl FocusedPanel {
    pub fn next(self) -> Self {
        match self {
            Self::Cpu => Self::Memory,
            Self::Memory => Self::Disk,
            Self::Disk => Self::Network,
            Self::Network => Self::Gpu,
            Self::Gpu => Self::Processes,
            Self::Processes => Self::Cpu,
        }
    }
    pub fn prev(self) -> Self {
        match self {
            Self::Cpu => Self::Processes,
            Self::Memory => Self::Cpu,
            Self::Disk => Self::Memory,
            Self::Network => Self::Disk,
            Self::Gpu => Self::Network,
            Self::Processes => Self::Gpu,
        }
    }
}

/// All mutable UI state lives here.
pub struct AppState {
    pub config: Config,
    pub hub: CollectorHub,
    pub focused_panel: FocusedPanel,
    pub process_cursor: usize,
    pub sort_state: SortState,
    pub filter_active: bool,
    pub filter_text: String,
    /// Lowercase cache of `filter_text` - updated on every keystroke, not per-process per-frame.
    pub filter_text_lower: String,
    pub show_system_processes: bool,
    pub user_filter_active: bool,
    /// Lowercase username - computed once at startup for filter comparisons.
    pub current_user_lower: String,
    pub show_help: bool,
    pub show_kill_confirm: bool,
    /// The (pid, name) of the process targeted for kill while the confirm dialog is open.
    pub kill_target: Option<(u32, String)>,
    /// Transient status message shown in the status bar (e.g. kill errors).
    /// Cleared on the next non-trivial key action.
    pub status_message: Option<String>,
    /// Windows Terminal environment info, resolved once at startup.
    pub wt_info: WtInfo,
    /// Whether the process detail inspect overlay is visible.
    pub show_inspect: bool,
    /// Data for the currently open inspect overlay.
    pub inspect_data: Option<ProcessInspectData>,
    /// Scroll offset for the module list inside the inspect overlay.
    pub inspect_scroll: usize,
    /// Whether the Windows Terminal info panel overlay is visible.
    pub show_wt_panel: bool,
    /// Whether the "apply Nerd Font" confirmation sub-dialog is active.
    pub wt_nerd_font_confirm: bool,
    /// Whether the settings panel overlay is open.
    pub show_settings: bool,
    /// Cursor position within the settings panel (0-indexed over selectable items).
    pub settings_cursor: usize,
    /// Whether the network adapter filter overlay is open.
    pub show_net_filter: bool,
    /// Cursor position within the net filter adapter list.
    pub net_filter_cursor: usize,
    /// Ordered list of available theme slugs (built-ins + user files).
    /// Rescanned on each theme cycle to pick up newly dropped files.
    pub available_themes: Vec<String>,
    /// Cached rendered theme - rebuilt only when the slug changes or the
    /// user's override file is modified on disk (mtime check each tick).
    pub theme_cache: crate::ui::theme::Theme,
    /// Human-readable display name from the theme file's `name =` field.
    pub theme_display_name: String,
    /// Author credit from the theme file's `author =` field.
    pub theme_author: Option<String>,
    /// Upstream homepage from the theme file's `homepage =` field.
    pub theme_homepage: Option<String>,
    /// Last-known mtime of the active user theme file (None = no file / built-in).
    theme_cache_mtime: Option<std::time::SystemTime>,
    /// Slug that was used to build `theme_cache` - detects slug changes.
    theme_cache_slug: String,
    /// Active tab inside the inspect overlay.
    pub inspect_tab: InspectTab,
    /// Cursor row within the Handles tab of the inspect overlay.
    pub inspect_handle_cursor: usize,
    /// Whether the force-close confirm sub-dialog is open inside the inspect overlay.
    pub inspect_close_confirm: bool,
    /// Whether the jump-to-PID input box is open.
    pub show_pid_jump: bool,
    /// Digits typed so far in the PID jump box.
    pub pid_jump_text: String,
    /// Set for one frame when the searched PID wasn't found.
    pub pid_jump_not_found: bool,
    /// Cursor row within the Info tab (selects copyable field).
    pub inspect_info_cursor: usize,
    /// Cursor row within the Modules tab.
    pub inspect_module_cursor: usize,
    /// Cursor row within the Env tab.
    pub inspect_env_cursor: usize,
    /// Horizontal scroll offset (chars) for long lines in inspect panel.
    pub inspect_h_offset: usize,
    /// Cursor row within the Threads tab of the inspect overlay.
    pub inspect_thread_cursor: usize,
    /// Cursor row within the Network tab of the inspect overlay.
    pub inspect_network_cursor: usize,
    /// Whether the name search input box is open.
    pub show_name_search: bool,
    /// Text typed so far in the name search box.
    pub name_search_text: String,
    /// Set for one frame when no process matched the search.
    pub name_search_not_found: bool,
    /// Whether the services overlay is visible.
    pub show_services: bool,
    /// Cursor row within the services panel.
    pub services_cursor: usize,
    /// Filter text typed while services panel is open.
    pub services_filter: String,
    /// Flat display order for tree view: indices into the sorted+filtered visible list.
    /// Identity mapping in flat mode; DFS order in tree mode.
    pub tree_display_order: Vec<usize>,
    /// Previous per-process CPU% - used to detect spikes.
    prev_cpu_pct: HashMap<u32, f32>,
    /// PIDs that spiked >15 pp since the last sample; counter = frames remaining.
    pub cpu_spike_flash: HashMap<u32, u8>,
    /// Cached HH:MM:SS timestamp string - refreshed once per second.
    pub cached_time: String,
    /// Unix-second at which `cached_time` was last formatted.
    last_time_sec: i64,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let hub = CollectorHub::spawn(&config);
        let show_sys = config.show_system_processes;
        let current_user_lower = std::env::var("USERNAME").unwrap_or_default().to_lowercase();

        crate::ui::theme_file::export_builtin_themes();

        let available_themes = crate::ui::theme_file::available_themes();

        let initial_theme = crate::ui::theme_file::load_theme(&config.theme);
        let theme_cache_mtime = crate::ui::theme_file::check_theme_mtime(&config.theme);
        let theme_cache_slug = config.theme.clone();

        Self {
            config,
            hub,
            focused_panel: FocusedPanel::Processes,
            process_cursor: 0,
            sort_state: SortState::default(),
            filter_active: false,
            filter_text: String::new(),
            filter_text_lower: String::new(),
            show_system_processes: show_sys,
            user_filter_active: false,
            current_user_lower,
            show_help: false,
            show_kill_confirm: false,
            kill_target: None,
            status_message: None,
            wt_info: WtInfo::detect(),
            show_inspect: false,
            inspect_data: None,
            inspect_scroll: 0,
            inspect_tab: InspectTab::Info,
            inspect_handle_cursor: 0,
            inspect_close_confirm: false,
            show_wt_panel: false,
            wt_nerd_font_confirm: false,
            show_settings: false,
            settings_cursor: 0,
            available_themes,
            theme_cache: initial_theme.theme,
            theme_display_name: initial_theme.display_name,
            theme_author: initial_theme.author,
            theme_homepage: initial_theme.homepage,
            theme_cache_mtime,
            theme_cache_slug,
            show_net_filter: false,
            net_filter_cursor: 0,
            show_pid_jump: false,
            pid_jump_text: String::new(),
            pid_jump_not_found: false,
            inspect_info_cursor: 0,
            inspect_module_cursor: 0,
            inspect_env_cursor: 0,
            inspect_h_offset: 0,
            inspect_thread_cursor: 0,
            inspect_network_cursor: 0,
            show_name_search: false,
            name_search_text: String::new(),
            name_search_not_found: false,
            show_services: false,
            services_cursor: 0,
            services_filter: String::new(),
            tree_display_order: Vec::new(),
            prev_cpu_pct: HashMap::new(),
            cpu_spike_flash: HashMap::new(),
            cached_time: Local::now().format("%H:%M:%S").to_string(),
            last_time_sec: Local::now().timestamp(),
        }
    }

    /// Compare current CPU% values against prev snapshot; start flash counter
    /// for any PID that jumped more than 15 percentage points. Decrement all
    /// existing counters, removing entries that reach zero.
    pub fn update_cpu_spikes(&mut self) {
        self.cpu_spike_flash.retain(|_, v| {
            *v = v.saturating_sub(1);
            *v > 0
        });

        if let Ok(procs) = self.hub.processes.read() {
            for p in procs.iter() {
                let prev = self.prev_cpu_pct.get(&p.pid).copied().unwrap_or(0.0);
                if p.cpu_pct - prev > 15.0 {
                    // ~400 ms of flash at 50 ms tick rate (8 frames).
                    self.cpu_spike_flash.insert(p.pid, 8);
                }
                self.prev_cpu_pct.insert(p.pid, p.cpu_pct);
            }
        }
    }

    /// Check whether the active theme file has changed on disk and reload if so.
    /// Also reloads if the slug changed (e.g. after a cycle).
    /// Parse errors are surfaced as a one-time status bar message.
    pub fn refresh_theme(&mut self) {
        let slug = self.config.theme.clone();
        let current_mtime = crate::ui::theme_file::check_theme_mtime(&slug);

        let slug_changed  = slug != self.theme_cache_slug;
        let file_modified = current_mtime != self.theme_cache_mtime;

        if slug_changed || file_modified {
            let result = crate::ui::theme_file::load_theme(&slug);
            if let Some(err) = result.error {
                self.status_message = Some(format!("Theme error: {err} - using built-in fallback"));
            }
            self.theme_display_name = result.display_name;
            self.theme_author       = result.author;
            self.theme_homepage     = result.homepage;
            self.theme_cache        = result.theme;
            self.theme_cache_slug   = slug;
            self.theme_cache_mtime  = current_mtime;
        }
    }

    /// Advance (or retreat) to the next available theme, wrapping around.
    /// Rescans the themes directory first to pick up any newly dropped files.
    fn cycle_theme(&mut self, forward: bool) {
        self.available_themes = crate::ui::theme_file::available_themes();

        let themes = &self.available_themes;
        if themes.is_empty() { return; }
        let current = themes.iter().position(|t| t == &self.config.theme).unwrap_or(0);
        let next = if forward {
            (current + 1) % themes.len()
        } else {
            current.checked_sub(1).unwrap_or(themes.len() - 1)
        };
        self.config.theme = themes[next].clone();
    }

    /// Refresh `cached_time` if the wall-clock second has advanced.
    pub fn tick_time(&mut self) {
        let now = Local::now();
        let sec = now.timestamp();
        if sec != self.last_time_sec {
            self.cached_time = if self.config.time_24h {
                now.format("%H:%M:%S").to_string()
            } else {
                now.format("%I:%M:%S %p").to_string()
            };
            self.last_time_sec = sec;
        }
    }

    /// Apply a pending thread-expansion result from the collector.
    fn drain_thread_results(&mut self) {
        while let Ok((pid, threads)) = self.hub.thread_result_rx.try_recv() {
            if let Ok(mut procs) = self.hub.processes.write() {
                if let Some(entry) = procs.iter_mut().find(|p| p.pid == pid) {
                    entry.threads = threads;
                }
            }
        }
    }

    fn dispatch(&mut self, action: AppAction, visible_count: usize) {
        if action != AppAction::None {
            self.status_message = None;
        }
        match action {
            AppAction::MoveUp => {
                if self.show_inspect {
                    self.inspect_h_offset = 0;
                    match self.inspect_tab {
                        InspectTab::Handles => {
                            self.inspect_handle_cursor = self.inspect_handle_cursor.saturating_sub(1);
                        }
                        InspectTab::Modules => {
                            self.inspect_module_cursor = self.inspect_module_cursor.saturating_sub(1);
                        }
                        InspectTab::Env => {
                            self.inspect_env_cursor = self.inspect_env_cursor.saturating_sub(1);
                        }
                        InspectTab::Info => {
                            self.inspect_info_cursor = self.inspect_info_cursor.saturating_sub(1);
                        }
                        InspectTab::Threads => {
                            self.inspect_thread_cursor = self.inspect_thread_cursor.saturating_sub(1);
                        }
                        InspectTab::Network => {
                            self.inspect_network_cursor = self.inspect_network_cursor.saturating_sub(1);
                        }
                    }
                } else if self.process_cursor > 0 {
                    self.process_cursor -= 1;
                }
            }
            AppAction::MoveDown => {
                if self.show_inspect {
                    self.inspect_h_offset = 0;
                    match self.inspect_tab {
                        InspectTab::Handles => {
                            let count = self.inspect_data.as_ref()
                                .map(|d| d.open_handles.len()).unwrap_or(0);
                            if self.inspect_handle_cursor + 1 < count {
                                self.inspect_handle_cursor += 1;
                            }
                        }
                        InspectTab::Modules => {
                            let count = self.inspect_data.as_ref()
                                .map(|d| d.modules.len()).unwrap_or(0);
                            if self.inspect_module_cursor + 1 < count {
                                self.inspect_module_cursor += 1;
                            }
                        }
                        InspectTab::Env => {
                            let count = self.inspect_data.as_ref()
                                .map(|d| d.env_vars.len()).unwrap_or(0);
                            if self.inspect_env_cursor + 1 < count {
                                self.inspect_env_cursor += 1;
                            }
                        }
                        InspectTab::Info => {
                            let max = crate::ui::inspect_panel::INFO_COPYABLE_COUNT;
                            if self.inspect_info_cursor + 1 < max {
                                self.inspect_info_cursor += 1;
                            }
                        }
                        InspectTab::Threads => {
                            let count = self.inspect_data.as_ref()
                                .map(|d| d.threads.len()).unwrap_or(0);
                            if self.inspect_thread_cursor + 1 < count {
                                self.inspect_thread_cursor += 1;
                            }
                        }
                        InspectTab::Network => {
                            let count = self.inspect_data.as_ref()
                                .map(|d| d.open_connections.len()).unwrap_or(0);
                            if self.inspect_network_cursor + 1 < count {
                                self.inspect_network_cursor += 1;
                            }
                        }
                    }
                } else if self.process_cursor + 1 < visible_count {
                    self.process_cursor += 1;
                }
            }
            AppAction::PageUp => {
                if self.show_inspect {
                    match self.inspect_tab {
                        InspectTab::Handles => {
                            self.inspect_handle_cursor = self.inspect_handle_cursor.saturating_sub(10);
                        }
                        InspectTab::Modules => {
                            self.inspect_module_cursor = self.inspect_module_cursor.saturating_sub(10);
                        }
                        InspectTab::Env => {
                            self.inspect_env_cursor = self.inspect_env_cursor.saturating_sub(10);
                        }
                        InspectTab::Threads => {
                            self.inspect_thread_cursor = self.inspect_thread_cursor.saturating_sub(10);
                        }
                        InspectTab::Network => {
                            self.inspect_network_cursor = self.inspect_network_cursor.saturating_sub(10);
                        }
                        _ => {
                            self.inspect_scroll = self.inspect_scroll.saturating_sub(10);
                        }
                    }
                } else {
                    self.process_cursor = self.process_cursor.saturating_sub(20);
                }
            }
            AppAction::PageDown => {
                if self.show_inspect {
                    match self.inspect_tab {
                        InspectTab::Handles => {
                            let count = self.inspect_data.as_ref()
                                .map(|d| d.open_handles.len()).unwrap_or(0);
                            self.inspect_handle_cursor =
                                (self.inspect_handle_cursor + 10).min(count.saturating_sub(1));
                        }
                        InspectTab::Modules => {
                            let count = self.inspect_data.as_ref()
                                .map(|d| d.modules.len()).unwrap_or(0);
                            self.inspect_module_cursor =
                                (self.inspect_module_cursor + 10).min(count.saturating_sub(1));
                        }
                        InspectTab::Env => {
                            let count = self.inspect_data.as_ref()
                                .map(|d| d.env_vars.len()).unwrap_or(0);
                            self.inspect_env_cursor =
                                (self.inspect_env_cursor + 10).min(count.saturating_sub(1));
                        }
                        InspectTab::Threads => {
                            let count = self.inspect_data.as_ref()
                                .map(|d| d.threads.len()).unwrap_or(0);
                            self.inspect_thread_cursor =
                                (self.inspect_thread_cursor + 10).min(count.saturating_sub(1));
                        }
                        InspectTab::Network => {
                            let count = self.inspect_data.as_ref()
                                .map(|d| d.open_connections.len()).unwrap_or(0);
                            self.inspect_network_cursor =
                                (self.inspect_network_cursor + 10).min(count.saturating_sub(1));
                        }
                        _ => {
                            self.inspect_scroll = self.inspect_scroll.saturating_add(10);
                        }
                    }
                } else {
                    self.process_cursor = (self.process_cursor + 20).min(visible_count.saturating_sub(1));
                }
            }
            AppAction::Home => self.process_cursor = 0,
            AppAction::End => self.process_cursor = visible_count.saturating_sub(1),
            AppAction::ExpandCollapse => {
                if let Ok(mut procs) = self.hub.processes.write() {
                    let filtered: Vec<usize> = procs
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| self.process_matches(p))
                        .map(|(i, _)| i)
                        .collect();
                    let disp = self.tree_display_order.get(self.process_cursor)
                        .copied().unwrap_or(self.process_cursor);
                    if let Some(&real_idx) = filtered.get(disp) {
                        let pid = procs[real_idx].pid;
                        let expanded = procs[real_idx].expanded;
                        procs[real_idx].expanded = !expanded;
                        if !expanded {
                            drop(procs);
                            let _ = self.hub.thread_request_tx.send(pid);
                        }
                    }
                }
            }
            AppAction::NextPanel => self.focused_panel = self.focused_panel.next(),
            AppAction::PrevPanel => self.focused_panel = self.focused_panel.prev(),
            AppAction::SortNext => self.sort_state.field = self.sort_state.field.next(),
            AppAction::SortPrev => self.sort_state.field = self.sort_state.field.prev(),
            AppAction::ToggleSortOrder => self.sort_state.ascending = !self.sort_state.ascending,
            AppAction::OpenFilter => self.filter_active = true,
            AppAction::CloseFilter => self.filter_active = false,
            AppAction::FilterEsc => {
                if self.filter_text.is_empty() {
                    self.filter_active = false;
                } else {
                    self.filter_text.clear();
                    self.filter_text_lower.clear();
                    self.process_cursor = 0;
                }
            }
            AppAction::FilterChar(c) => {
                self.filter_text.push(c);
                self.filter_text_lower = self.filter_text.to_lowercase();
            }
            AppAction::FilterBackspace => {
                self.filter_text.pop();
                self.filter_text_lower = self.filter_text.to_lowercase();
            }
            AppAction::KillProcess => {
                if let Ok(procs) = self.hub.processes.read() {
                    let filtered: Vec<(u32, String)> = procs
                        .iter()
                        .filter(|p| self.process_matches(p))
                        .map(|p| (p.pid, p.name.clone()))
                        .collect();
                    let disp = self.tree_display_order.get(self.process_cursor)
                        .copied().unwrap_or(self.process_cursor);
                    if let Some((pid, name)) = filtered.get(disp) {
                        self.kill_target = Some((*pid, name.clone()));
                        self.show_kill_confirm = true;
                    }
                }
            }
            AppAction::ConfirmKill => {
                if let Some((pid, ref name)) = self.kill_target.take() {
                    if !kill_process(pid) {
                        self.status_message = Some(format!(
                            "Kill failed: {} (PID {}) - run as Administrator to kill system processes",
                            name, pid
                        ));
                    }
                }
                self.show_kill_confirm = false;
            }
            AppAction::CancelKill => {
                self.kill_target = None;
                self.show_kill_confirm = false;
            }
            AppAction::ToggleSystemProcesses => {
                self.show_system_processes = !self.show_system_processes;
            }
            AppAction::ToggleUserFilter => {
                self.user_filter_active = !self.user_filter_active;
                self.process_cursor = 0;
            }
            AppAction::OpenPidJump => {
                self.show_pid_jump = true;
                self.pid_jump_text.clear();
                self.pid_jump_not_found = false;
            }
            AppAction::PidJumpChar(c) => {
                if self.pid_jump_text.len() < 7 {
                    self.pid_jump_text.push(c);
                    self.pid_jump_not_found = false;
                }
            }
            AppAction::PidJumpBackspace => {
                self.pid_jump_text.pop();
                self.pid_jump_not_found = false;
            }
            AppAction::PidJumpCancel => {
                self.show_pid_jump = false;
                self.pid_jump_text.clear();
                self.pid_jump_not_found = false;
            }
            AppAction::PidJumpConfirm => {
                if let Ok(target_pid) = self.pid_jump_text.parse::<u32>() {
                    if let Ok(procs) = self.hub.processes.read() {
                        let pos = procs
                            .iter()
                            .filter(|p| self.process_matches(p))
                            .position(|p| p.pid == target_pid);
                        if let Some(idx) = pos {
                            self.process_cursor = idx;
                            self.show_pid_jump = false;
                            self.pid_jump_text.clear();
                            self.pid_jump_not_found = false;
                        } else {
                            self.pid_jump_not_found = true;
                        }
                    }
                } else {
                    self.pid_jump_not_found = true;
                }
            }
            AppAction::NetFilterClose => {
                self.show_net_filter = false;
                self.config.save();
            }
            AppAction::NetFilterUp => {
                self.net_filter_cursor = self.net_filter_cursor.saturating_sub(1);
            }
            AppAction::NetFilterDown => {
                if let Ok(nets) = self.hub.networks.read() {
                    if self.net_filter_cursor + 1 < nets.len() {
                        self.net_filter_cursor += 1;
                    }
                }
            }
            AppAction::NetFilterToggle => {
                if let Ok(nets) = self.hub.networks.read() {
                    if let Some(n) = nets.get(self.net_filter_cursor) {
                        let name = n.display_name.clone();
                        if let Some(pos) = self.config.hidden_adapters.iter().position(|h| h == &name) {
                            self.config.hidden_adapters.remove(pos);
                        } else {
                            self.config.hidden_adapters.push(name);
                        }
                    }
                }
            }
            AppAction::ToggleHelp => self.show_help = !self.show_help,
            AppAction::InspectNextTab => {
                self.inspect_tab = self.inspect_tab.next();
                self.inspect_h_offset = 0;
            }
            AppAction::InspectScrollLeft => {
                self.inspect_h_offset = self.inspect_h_offset.saturating_sub(1);
            }
            AppAction::InspectScrollRight => {
                self.inspect_h_offset += 1;
            }
            AppAction::InspectCopyLine => {
                if let Some(ref data) = self.inspect_data {
                    let text = crate::clipboard::get_copy_text(
                        self.inspect_tab,
                        data,
                        &crate::clipboard::InspectCursors {
                            info:    self.inspect_info_cursor,
                            module:  self.inspect_module_cursor,
                            handle:  self.inspect_handle_cursor,
                            env:     self.inspect_env_cursor,
                            thread:  self.inspect_thread_cursor,
                            network: self.inspect_network_cursor,
                        },
                    );
                    if let Some(s) = text {
                        crate::clipboard::write_clipboard(&s);
                        self.status_message = Some(format!("Copied: {}", crate::ui::truncate(&s, 60)));
                    }
                }
            }
            AppAction::InspectInitCloseHandle => {
                if self.inspect_tab == InspectTab::Handles {
                    if let Some(ref data) = self.inspect_data {
                        if self.inspect_handle_cursor < data.open_handles.len() {
                            self.inspect_close_confirm = true;
                        }
                    }
                }
            }
            AppAction::ConfirmCloseHandle => {
                self.inspect_close_confirm = false;
                if let Some(ref mut data) = self.inspect_data {
                    if let Some(entry) = data.open_handles.get(self.inspect_handle_cursor) {
                        let pid = data.pid;
                        let hv  = entry.handle_value;
                        match crate::collectors::inspect::force_close_handle(pid, hv) {
                            Ok(()) => {
                                data.open_handles.remove(self.inspect_handle_cursor);
                                if self.inspect_handle_cursor > 0
                                    && self.inspect_handle_cursor >= data.open_handles.len()
                                {
                                    self.inspect_handle_cursor -= 1;
                                }
                                self.status_message = Some("Handle closed.".into());
                            }
                            Err(e) => {
                                self.status_message = Some(format!("Close failed: {e}"));
                            }
                        }
                    }
                }
            }
            AppAction::CancelCloseHandle => {
                self.inspect_close_confirm = false;
            }
            AppAction::ToggleInspect => {
                if self.show_inspect {
                    self.show_inspect = false;
                    self.inspect_data = None;
                    self.inspect_scroll = 0;
                    self.inspect_tab = InspectTab::Info;
                    self.inspect_handle_cursor = 0;
                    self.inspect_close_confirm = false;
                    self.inspect_info_cursor = 0;
                    self.inspect_module_cursor = 0;
                    self.inspect_env_cursor = 0;
                    self.inspect_h_offset = 0;
                    self.inspect_thread_cursor = 0;
                    self.inspect_network_cursor = 0;
                } else {
                    if let Ok(procs) = self.hub.processes.read() {
                        let filtered: Vec<(u32, String)> = procs
                            .iter()
                            .filter(|p| self.process_matches(p))
                            .map(|p| (p.pid, p.name.clone()))
                            .collect();
                        let disp = self.tree_display_order.get(self.process_cursor)
                            .copied().unwrap_or(self.process_cursor);
                        if let Some((pid, name)) = filtered.get(disp) {
                            self.inspect_data = Some(
                                crate::collectors::inspect::collect_inspect(*pid, name),
                            );
                            self.inspect_scroll = 0;
                            self.show_inspect = true;
                        }
                    }
                }
            }
            AppAction::ToggleNerdGlyphs => {
                self.config.nerd_glyphs = !self.config.nerd_glyphs;
                self.config.save();
            }
            AppAction::CycleTheme => {
                self.cycle_theme(true);
                self.refresh_theme();
                self.status_message = Some(format!("Theme: {}", self.theme_display_name));
                self.config.save();
            }
            AppAction::CycleLayout => {
                self.config.layout_mode = self.config.layout_mode.cycle();
                self.status_message = Some(format!("Layout: {}", self.config.layout_mode.label()));
                self.config.save();
            }
            AppAction::ToggleDisk => {
                self.config.show_disk = !self.config.show_disk;
                if !self.config.show_disk && self.focused_panel == FocusedPanel::Disk {
                    self.focused_panel = FocusedPanel::Processes;
                }
                self.status_message = Some(if self.config.show_disk {
                    "Disk panel: shown".to_string()
                } else {
                    "Disk panel: hidden".to_string()
                });
                self.config.save();
            }
            AppAction::ToggleGpu => {
                self.config.show_gpu = !self.config.show_gpu;
                if !self.config.show_gpu && self.focused_panel == FocusedPanel::Gpu {
                    self.focused_panel = FocusedPanel::Processes;
                }
                self.status_message = Some(if self.config.show_gpu {
                    "GPU panel: shown".to_string()
                } else {
                    "GPU panel: hidden".to_string()
                });
                self.config.save();
            }
            AppAction::ToggleNetwork => {
                self.config.show_network = !self.config.show_network;
                if !self.config.show_network && self.focused_panel == FocusedPanel::Network {
                    self.focused_panel = FocusedPanel::Processes;
                }
                self.status_message = Some(if self.config.show_network {
                    "Network panel: shown".to_string()
                } else {
                    "Network panel: hidden".to_string()
                });
                self.config.save();
            }
            AppAction::ToggleDiskColumns => {
                let currently_shown = self.config.process_columns
                    .iter()
                    .any(|c| c.id == ProcessColumnId::DiskRead && c.visible);
                for col in self.config.process_columns.iter_mut() {
                    if col.id == ProcessColumnId::DiskRead || col.id == ProcessColumnId::DiskWrite {
                        col.visible = !currently_shown;
                    }
                }
                self.status_message = Some(if !currently_shown {
                    "Disk I/O columns: shown".to_string()
                } else {
                    "Disk I/O columns: hidden".to_string()
                });
                self.config.save();
            }
            AppAction::ToggleWtPanel => {
                self.show_wt_panel = !self.show_wt_panel;
                self.wt_nerd_font_confirm = false;
            }
            AppAction::WtConfirmNerdFont => {
                if self.show_wt_panel && self.wt_info.detected {
                    self.wt_nerd_font_confirm = true;
                }
            }
            AppAction::WtApplyNerdFont => {
                self.wt_nerd_font_confirm = false;
                match self.wt_info.apply_nerd_font() {
                    Ok(()) => {
                        self.wt_info.font_face =
                            Some(crate::wt::NERD_FONT_FACE.to_string());
                        self.status_message = Some(format!(
                            "Applied \"{}\". Restart Windows Terminal to see the change.",
                            crate::wt::NERD_FONT_FACE
                        ));
                        self.config.save();
                        self.show_wt_panel = false;
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Font apply failed: {e}"));
                        self.show_wt_panel = false;
                    }
                }
            }
            AppAction::WtCancelNerdFont => {
                self.wt_nerd_font_confirm = false;
            }
            AppAction::ToggleTreeView => {
                self.config.tree_view = !self.config.tree_view;
                self.process_cursor = 0;
                self.status_message = Some(if self.config.tree_view {
                    "Tree view: on".to_string()
                } else {
                    "Tree view: off".to_string()
                });
                self.config.save();
            }
            AppAction::OpenNameSearch => {
                self.show_name_search = true;
                self.name_search_text.clear();
                self.name_search_not_found = false;
            }
            AppAction::NameSearchChar(c) => {
                self.name_search_text.push(c);
                self.name_search_not_found = false;
            }
            AppAction::NameSearchBackspace => {
                self.name_search_text.pop();
                self.name_search_not_found = false;
            }
            AppAction::NameSearchCancel => {
                self.show_name_search = false;
                self.name_search_text.clear();
                self.name_search_not_found = false;
            }
            AppAction::NameSearchConfirm => {
                let needle = self.name_search_text.to_lowercase();
                if let Ok(procs) = self.hub.processes.read() {
                    let filtered: Vec<_> = procs
                        .iter()
                        .filter(|p| self.process_matches(p))
                        .collect();
                    let pos = self.tree_display_order.iter().position(|&di| {
                        filtered.get(di)
                            .map(|p| p.name.to_lowercase().contains(&needle))
                            .unwrap_or(false)
                    }).or_else(|| {
                        filtered.iter().position(|p| p.name.to_lowercase().contains(&needle))
                    });
                    if let Some(idx) = pos {
                        self.process_cursor = idx;
                        self.show_name_search = false;
                        self.name_search_text.clear();
                        self.name_search_not_found = false;
                    } else {
                        self.name_search_not_found = true;
                    }
                }
            }
            AppAction::ToggleServices => {
                self.show_services = !self.show_services;
                if !self.show_services {
                    self.services_cursor = 0;
                    self.services_filter.clear();
                }
            }
            AppAction::ServicesUp => {
                self.services_cursor = self.services_cursor.saturating_sub(1);
            }
            AppAction::ServicesDown => {
                let count = self.filtered_services_count();
                if self.services_cursor + 1 < count {
                    self.services_cursor += 1;
                }
            }
            AppAction::ServicesPageUp => {
                self.services_cursor = self.services_cursor.saturating_sub(10);
            }
            AppAction::ServicesPageDown => {
                let count = self.filtered_services_count();
                self.services_cursor = (self.services_cursor + 10).min(count.saturating_sub(1));
            }
            AppAction::ServicesFilterChar(c) => {
                self.services_filter.push(c);
                self.services_cursor = 0;
            }
            AppAction::ServicesFilterBackspace => {
                self.services_filter.pop();
                self.services_cursor = 0;
            }
            AppAction::ServicesFilterClear => {
                self.services_filter.clear();
                self.services_cursor = 0;
            }
            AppAction::ToggleSettings => {
                self.show_settings = !self.show_settings;
                if !self.show_settings {
                    self.config.save();
                }
            }
            AppAction::SettingsUp => {
                if self.settings_cursor > 0 {
                    self.settings_cursor -= 1;
                }
            }
            AppAction::SettingsDown => {
                if self.settings_cursor + 1 < crate::ui::settings_panel::SETTINGS_COUNT {
                    self.settings_cursor += 1;
                }
            }
            AppAction::SettingsActivate => self.apply_setting(true),
            AppAction::SettingsActivateBack => self.apply_setting(false),
            AppAction::IncreaseRefresh => {
                self.config.refresh_interval_ms =
                    (self.config.refresh_interval_ms + 250).min(5000);
                self.config.save();
            }
            AppAction::DecreaseRefresh => {
                self.config.refresh_interval_ms =
                    self.config.refresh_interval_ms.saturating_sub(250).max(250);
                self.config.save();
            }
            _ => {}
        }
    }

    fn apply_setting(&mut self, forward: bool) {
        use crate::config::ProcessColumnId;
        match self.settings_cursor {
            0 => self.cycle_theme(forward),
            1 => self.config.layout_mode = if forward { self.config.layout_mode.cycle() } else { self.config.layout_mode.cycle_back() },
            2 => self.config.nerd_glyphs = !self.config.nerd_glyphs,
            3 => self.config.ascii_mode = !self.config.ascii_mode,
            4 => {
                self.config.show_disk = !self.config.show_disk;
                if !self.config.show_disk && self.focused_panel == FocusedPanel::Disk {
                    self.focused_panel = FocusedPanel::Processes;
                }
            }
            5 => {
                self.config.show_network = !self.config.show_network;
                if !self.config.show_network && self.focused_panel == FocusedPanel::Network {
                    self.focused_panel = FocusedPanel::Processes;
                }
            }
            6 => {
                let shown = self.config.process_columns
                    .iter()
                    .any(|c| c.id == ProcessColumnId::DiskRead && c.visible);
                for col in self.config.process_columns.iter_mut() {
                    if col.id == ProcessColumnId::DiskRead || col.id == ProcessColumnId::DiskWrite {
                        col.visible = !shown;
                    }
                }
            }
            7 => self.config.show_system_processes = !self.config.show_system_processes,
            8 => self.config.hide_virtual_adapters = !self.config.hide_virtual_adapters,
            9 => {
                self.show_net_filter = true;
                self.net_filter_cursor = 0;
                self.show_settings = false;
            }
            10 => {
                if forward {
                    self.config.refresh_interval_ms = (self.config.refresh_interval_ms + 250).min(5000);
                } else {
                    self.config.refresh_interval_ms = self.config.refresh_interval_ms.saturating_sub(250).max(250);
                }
            }
            11 => self.config.time_24h = !self.config.time_24h,
            12 => {
                self.config.tree_view = !self.config.tree_view;
                self.process_cursor = 0;
            }
            23 => {
                self.config.show_gpu = !self.config.show_gpu;
                if !self.config.show_gpu && self.focused_panel == FocusedPanel::Gpu {
                    self.focused_panel = FocusedPanel::Processes;
                }
            }
            13 => self.toggle_column(ProcessColumnId::Pid),
            14 => self.toggle_column(ProcessColumnId::Name),
            15 => self.toggle_column(ProcessColumnId::CpuPct),
            16 => self.toggle_column(ProcessColumnId::Mem),
            17 => self.toggle_column(ProcessColumnId::MemPct),
            18 => self.toggle_column(ProcessColumnId::Threads),
            19 => self.toggle_column(ProcessColumnId::Status),
            20 => self.toggle_column(ProcessColumnId::User),
            21 => self.toggle_column(ProcessColumnId::DiskRead),
            22 => self.toggle_column(ProcessColumnId::DiskWrite),
            _ => {}
        }
    }

    fn toggle_column(&mut self, id: ProcessColumnId) {
        if let Some(col) = self.config.process_columns.iter_mut().find(|c| c.id == id) {
            col.visible = !col.visible;
            self.config.save();
        }
    }

    fn filtered_services_count(&self) -> usize {
        if let Ok(svcs) = self.hub.services.read() {
            if self.services_filter.is_empty() {
                return svcs.len();
            }
            let lower = self.services_filter.to_lowercase();
            svcs.iter()
                .filter(|s| s.name.to_lowercase().contains(&lower)
                    || s.display_name.to_lowercase().contains(&lower))
                .count()
        } else {
            0
        }
    }

    pub fn process_matches(&self, p: &crate::models::process::ProcessEntry) -> bool {
        if !self.show_system_processes && is_system_account(&p.user) {
            return false;
        }
        if self.user_filter_active
            && !self.current_user_lower.is_empty()
            && !p.user.to_lowercase().contains(&self.current_user_lower as &str)
        {
            return false;
        }
        if !self.filter_text_lower.is_empty() {
            return p.name.to_lowercase().contains(&self.filter_text_lower as &str);
        }
        true
    }
}

/// Build DFS tree display order.
/// `procs` is the full sorted process Vec; `filtered` is sorted indices into it.
/// Returns a Vec of indices into `filtered` in DFS parent-first order.
pub fn build_tree_display_order(
    procs: &[crate::models::process::ProcessEntry],
    filtered: &[usize],
) -> Vec<usize> {
    use std::collections::HashMap;

    let pid_to_fi: HashMap<u32, usize> = filtered
        .iter()
        .enumerate()
        .map(|(fi, &ri)| (procs[ri].pid, fi))
        .collect();

    let mut children: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut roots: Vec<usize> = Vec::new();

    for (fi, &ri) in filtered.iter().enumerate() {
        let parent_pid = procs[ri].parent_pid;
        if parent_pid > 0 {
            if let Some(&parent_fi) = pid_to_fi.get(&parent_pid) {
                if parent_fi != fi {
                    children.entry(parent_fi).or_default().push(fi);
                    continue;
                }
            }
        }
        roots.push(fi);
    }

    let mut order: Vec<usize> = Vec::with_capacity(filtered.len());
    let mut stack: Vec<usize> = roots.into_iter().rev().collect();
    while let Some(fi) = stack.pop() {
        order.push(fi);
        if let Some(mut kids) = children.remove(&fi) {
            kids.reverse();
            stack.extend(kids);
        }
    }

    let visited: std::collections::HashSet<usize> = order.iter().copied().collect();
    for fi in 0..filtered.len() {
        if !visited.contains(&fi) {
            order.push(fi);
        }
    }

    order
}

/// Returns true if the user account is a Windows built-in service account.
pub fn is_system_account(user: &str) -> bool {
    matches!(
        user,
        "SYSTEM" | "LOCAL SERVICE" | "NETWORK SERVICE" | "?"
    ) || user.starts_with("NT AUTHORITY\\")
        || user.starts_with("NT SERVICE\\")
        || user.is_empty()
}

/// Kill a process by PID using TerminateProcess. Returns true on success.
fn kill_process(pid: u32) -> bool {
    // Safety: pid comes from the live process list; we check the handle before use.
    unsafe {
        match OpenProcess(PROCESS_TERMINATE, false, pid) {
            Ok(handle) => {
                let ok = TerminateProcess(handle, 1).is_ok();
                let _ = CloseHandle(handle);
                ok
            }
            Err(_) => false,
        }
    }
}

/// Attempt to enable SeDebugPrivilege for the current process.
/// This allows opening handles to more processes (same as Task Manager / Process Explorer).
/// Silently does nothing if already enabled or if the process isn't running as admin.
fn enable_debug_privilege() {
    use windows::Win32::Foundation::LUID;

    unsafe {
        let mut token = windows::Win32::Foundation::HANDLE::default();
        if OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token,
        )
        .is_err()
        {
            return;
        }

        let mut luid = LUID::default();
        if LookupPrivilegeValueW(None, windows::core::w!("SeDebugPrivilege"), &mut luid).is_err() {
            let _ = CloseHandle(token);
            return;
        }

        let tp = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };
        let _ = AdjustTokenPrivileges(token, false, Some(&tp), 0, None, None);
        let _ = CloseHandle(token);
    }
}

/// Run the application: set up the terminal, enter the event loop, restore on exit.
pub fn run(config: Config) -> anyhow::Result<()> {
    enable_debug_privilege();
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = event_loop(&mut terminal, config);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    result
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    config: Config,
) -> anyhow::Result<()> {
    let mut state = AppState::new(config);
    let tick = Duration::from_millis(50); // 20fps draw rate

    std::thread::sleep(Duration::from_millis(300));

    loop {
        state.tick_time();
        state.refresh_theme();

        let dead = state.hub.dead_collectors();
        if !dead.is_empty() && state.status_message.is_none() {
            state.status_message = Some(format!(
                "Collector crashed: {} - data may be stale",
                dead.join(", ")
            ));
        }

        state.drain_thread_results();
        state.update_cpu_spikes();

        let visible_count = {
            if let Ok(mut procs) = state.hub.processes.write() {
                sort_processes(&mut procs, state.sort_state);
                let filtered: Vec<usize> = procs
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| state.process_matches(p))
                    .map(|(i, _)| i)
                    .collect();
                let count = filtered.len();
                if state.config.tree_view {
                    state.tree_display_order =
                        build_tree_display_order(&procs, &filtered);
                } else {
                    state.tree_display_order = (0..count).collect();
                }
                count
            } else {
                0
            }
        };

        if state.process_cursor >= visible_count && visible_count > 0 {
            state.process_cursor = visible_count - 1;
        }

        terminal.draw(|frame| {
            crate::ui::draw(frame, &state);
        })?;

        if event::poll(tick)? {
            match event::read()? {
                Event::Key(key) => {
                    // On Windows, crossterm fires both Press and Release events.
                    // Only act on Press to prevent every key from registering twice.
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    let action = handle_key(key, &ModalState {
                        filter_active:                state.filter_active,
                        kill_confirm_active:          state.show_kill_confirm,
                        wt_panel_active:              state.show_wt_panel,
                        wt_nerd_font_confirm_active:  state.wt_nerd_font_confirm,
                        settings_active:              state.show_settings,
                        inspect_active:               state.show_inspect,
                        inspect_close_confirm_active: state.inspect_close_confirm,
                        pid_jump_active:              state.show_pid_jump,
                        net_filter_active:            state.show_net_filter,
                        help_active:                  state.show_help,
                        name_search_active:           state.show_name_search,
                        services_active:              state.show_services,
                    });
                    if action == AppAction::Quit {
                        state.config.save();
                        return Ok(());
                    }
                    state.dispatch(action, visible_count);
                }
                Event::Mouse(mouse) => {
                    let sz = terminal.size()?;
                    let term_rect = ratatui::layout::Rect::new(0, 0, sz.width, sz.height);
                    handle_mouse_event(mouse, &mut state, visible_count, term_rect);
                }
                _ => {}
            }
        }
    }
}

fn handle_mouse_event(
    mouse: crossterm::event::MouseEvent,
    state: &mut AppState,
    visible_count: usize,
    term_size: ratatui::layout::Rect,
) {
    // Skip mouse events when any modal overlay is open - keyboard takes priority.
    if state.show_inspect || state.show_kill_confirm || state.show_settings
        || state.show_wt_panel || state.show_pid_jump || state.show_net_filter
        || state.show_help || state.show_name_search || state.filter_active
        || state.show_services
    {
        return;
    }

    let rects = crate::ui::layout::compute(
        term_size,
        &state.config.layout_mode,
        state.config.show_disk,
        state.config.show_network,
        state.config.show_gpu,
    );

    let mc = mouse.column;
    let mr = mouse.row;
    let in_rect = |r: ratatui::layout::Rect| {
        mc >= r.x && mc < r.x + r.width && mr >= r.y && mr < r.y + r.height
    };

    match mouse.kind {
        MouseEventKind::ScrollUp => {
            state.dispatch(AppAction::MoveUp, visible_count);
        }
        MouseEventKind::ScrollDown => {
            state.dispatch(AppAction::MoveDown, visible_count);
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if in_rect(rects.processes) {
                state.focused_panel = FocusedPanel::Processes;
                let header_y = rects.processes.y + 1; // y+1 = inside border
                let data_start_y = rects.processes.y + 2; // +1 for header row
                if mr == header_y && mc > rects.processes.x {
                    // Column header click - sort by that column
                    let inner_x = (mc - (rects.processes.x + 1)) as usize;
                    let inner_width = rects.processes.width.saturating_sub(2) as usize;
                    let vis_cols: Vec<&ProcessColumnId> = state.config.process_columns
                        .iter()
                        .filter(|c| c.visible)
                        .map(|c| &c.id)
                        .collect();
                    if let Some(sf) = col_sort_field_at_x(&vis_cols, inner_width, inner_x) {
                        if state.sort_state.field == sf {
                            state.sort_state.ascending = !state.sort_state.ascending;
                        } else {
                            state.sort_state.field = sf;
                            state.sort_state.ascending = false;
                        }
                    }
                } else if mr >= data_start_y && visible_count > 0 {
                    let display_row = (mr - data_start_y) as usize;
                    if display_row < visible_count {
                        state.process_cursor = display_row;
                    }
                }
            } else if in_rect(rects.cpu) {
                state.focused_panel = FocusedPanel::Cpu;
            } else if in_rect(rects.memory) {
                state.focused_panel = FocusedPanel::Memory;
            } else if rects.disk.is_some_and(|r| in_rect(r)) {
                state.focused_panel = FocusedPanel::Disk;
            } else if rects.network.is_some_and(|r| in_rect(r)) {
                state.focused_panel = FocusedPanel::Network;
            } else if rects.gpu.is_some_and(|r| in_rect(r)) {
                state.focused_panel = FocusedPanel::Gpu;
            }
        }
        _ => {}
    }
}

/// Map a click x-position (relative to process panel inner area) to a sort field.
/// Returns None for columns that have no corresponding sort field.
fn col_sort_field_at_x(
    vis_cols: &[&ProcessColumnId],
    inner_width: usize,
    click_x: usize,
) -> Option<ProcessSortField> {
    // Fixed column widths matching col_constraint() in process_panel.rs.
    let fixed_total: usize = vis_cols.iter().filter_map(|id| match id {
        ProcessColumnId::Name => None,
        ProcessColumnId::Pid       => Some(7usize),
        ProcessColumnId::CpuPct    => Some(10),
        ProcessColumnId::Mem       => Some(9),
        ProcessColumnId::MemPct    => Some(6),
        ProcessColumnId::Threads   => Some(5),
        ProcessColumnId::Status    => Some(8),
        ProcessColumnId::User      => Some(12),
        ProcessColumnId::DiskRead  => Some(11),
        ProcessColumnId::DiskWrite => Some(11),
    }).sum();

    let has_name = vis_cols.iter().any(|id| matches!(id, ProcessColumnId::Name));
    let name_width = if has_name { inner_width.saturating_sub(fixed_total).max(18) } else { 0 };

    let mut x = 0usize;
    for id in vis_cols {
        let w = match id {
            ProcessColumnId::Name      => name_width,
            ProcessColumnId::Pid       => 7,
            ProcessColumnId::CpuPct    => 10,
            ProcessColumnId::Mem       => 9,
            ProcessColumnId::MemPct    => 6,
            ProcessColumnId::Threads   => 5,
            ProcessColumnId::Status    => 8,
            ProcessColumnId::User      => 12,
            ProcessColumnId::DiskRead  => 11,
            ProcessColumnId::DiskWrite => 11,
        };
        if click_x >= x && click_x < x + w {
            return match id {
                ProcessColumnId::Pid       => Some(ProcessSortField::Pid),
                ProcessColumnId::Name      => Some(ProcessSortField::Name),
                ProcessColumnId::CpuPct    => Some(ProcessSortField::CpuPct),
                ProcessColumnId::Mem       => Some(ProcessSortField::MemBytes),
                ProcessColumnId::Threads   => Some(ProcessSortField::ThreadCount),
                ProcessColumnId::DiskRead  => Some(ProcessSortField::DiskRead),
                ProcessColumnId::DiskWrite => Some(ProcessSortField::DiskWrite),
                _ => None,
            };
        }
        x += w;
    }
    None
}
