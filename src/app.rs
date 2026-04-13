use std::collections::HashMap;
use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
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
use crate::input::handler::{handle_key, AppAction};
use crate::models::process::{sort_processes, SortState};
use crate::models::inspect::ProcessInspectData;
use crate::wt::WtInfo;

/// Which panel has keyboard focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusedPanel {
    Cpu,
    Memory,
    Disk,
    Network,
    Processes,
}

impl FocusedPanel {
    pub fn next(self) -> Self {
        match self {
            Self::Cpu => Self::Memory,
            Self::Memory => Self::Disk,
            Self::Disk => Self::Network,
            Self::Network => Self::Processes,
            Self::Processes => Self::Cpu,
        }
    }
    pub fn prev(self) -> Self {
        match self {
            Self::Cpu => Self::Processes,
            Self::Memory => Self::Cpu,
            Self::Disk => Self::Memory,
            Self::Network => Self::Disk,
            Self::Processes => Self::Network,
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
    /// Lowercase cache of `filter_text` — updated on every keystroke, not per-process per-frame.
    pub filter_text_lower: String,
    pub show_system_processes: bool,
    pub user_filter_active: bool,
    /// Lowercase username — computed once at startup for filter comparisons.
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
    /// Whether the jump-to-PID input box is open.
    pub show_pid_jump: bool,
    /// Digits typed so far in the PID jump box.
    pub pid_jump_text: String,
    /// Set for one frame when the searched PID wasn't found.
    pub pid_jump_not_found: bool,
    /// Previous per-process CPU% — used to detect spikes.
    prev_cpu_pct: HashMap<u32, f32>,
    /// PIDs that spiked >15 pp since the last sample; counter = frames remaining.
    pub cpu_spike_flash: HashMap<u32, u8>,
    /// Cached HH:MM:SS timestamp string — refreshed once per second.
    pub cached_time: String,
    /// Unix-second at which `cached_time` was last formatted.
    last_time_sec: i64,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let hub = CollectorHub::spawn(&config);
        let show_sys = config.show_system_processes;
        let current_user_lower = std::env::var("USERNAME").unwrap_or_default().to_lowercase();
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
            show_wt_panel: false,
            wt_nerd_font_confirm: false,
            show_settings: false,
            settings_cursor: 0,
            show_net_filter: false,
            net_filter_cursor: 0,
            show_pid_jump: false,
            pid_jump_text: String::new(),
            pid_jump_not_found: false,
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
        // Decrement / expire existing flash entries.
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

    /// Refresh `cached_time` if the wall-clock second has advanced.
    pub fn tick_time(&mut self) {
        let now = Local::now();
        let sec = now.timestamp();
        if sec != self.last_time_sec {
            self.cached_time = now.format("%H:%M:%S").to_string();
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
        // Clear any transient status message on the next meaningful key.
        if action != AppAction::None {
            self.status_message = None;
        }
        match action {
            AppAction::MoveUp => {
                if self.show_inspect {
                    self.inspect_scroll = self.inspect_scroll.saturating_sub(1);
                } else if self.process_cursor > 0 {
                    self.process_cursor -= 1;
                }
            }
            AppAction::MoveDown => {
                if self.show_inspect {
                    self.inspect_scroll = self.inspect_scroll.saturating_add(1);
                } else if self.process_cursor + 1 < visible_count {
                    self.process_cursor += 1;
                }
            }
            AppAction::PageUp => {
                if self.show_inspect {
                    self.inspect_scroll = self.inspect_scroll.saturating_sub(10);
                } else {
                    self.process_cursor = self.process_cursor.saturating_sub(20);
                }
            }
            AppAction::PageDown => {
                if self.show_inspect {
                    self.inspect_scroll = self.inspect_scroll.saturating_add(10);
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
                    if let Some(&real_idx) = filtered.get(self.process_cursor) {
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
            AppAction::FilterChar(c) => {
                self.filter_text.push(c);
                self.filter_text_lower = self.filter_text.to_lowercase();
            }
            AppAction::FilterBackspace => {
                self.filter_text.pop();
                self.filter_text_lower = self.filter_text.to_lowercase();
            }
            AppAction::KillProcess => {
                // Capture the selected process before showing the dialog.
                if let Ok(procs) = self.hub.processes.read() {
                    let filtered: Vec<(u32, String)> = procs
                        .iter()
                        .filter(|p| self.process_matches(p))
                        .map(|p| (p.pid, p.name.clone()))
                        .collect();
                    if let Some((pid, name)) = filtered.get(self.process_cursor) {
                        self.kill_target = Some((*pid, name.clone()));
                        self.show_kill_confirm = true;
                    }
                }
            }
            AppAction::ConfirmKill => {
                if let Some((pid, ref name)) = self.kill_target.take() {
                    if !kill_process(pid) {
                        self.status_message = Some(format!(
                            "Kill failed: {} (PID {}) — run as Administrator to kill system processes",
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
            AppAction::ToggleInspect => {
                if self.show_inspect {
                    self.show_inspect = false;
                    self.inspect_data = None;
                    self.inspect_scroll = 0;
                } else {
                    if let Ok(procs) = self.hub.processes.read() {
                        let filtered: Vec<(u32, String)> = procs
                            .iter()
                            .filter(|p| self.process_matches(p))
                            .map(|p| (p.pid, p.name.clone()))
                            .collect();
                        if let Some((pid, name)) = filtered.get(self.process_cursor) {
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
                self.config.theme = self.config.theme.cycle();
                self.status_message = Some(format!("Theme: {}", self.config.theme.label()));
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
                        // Refresh font_face in our snapshot so the panel shows the new font.
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
            0 => self.config.theme = if forward { self.config.theme.cycle() } else { self.config.theme.cycle_back() },
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
                // Open the adapter filter overlay.
                self.show_net_filter = true;
                self.net_filter_cursor = 0;
                // Close settings panel while filter is open to avoid overlap.
                self.show_settings = false;
            }
            10 => {
                if forward {
                    self.config.refresh_interval_ms = (self.config.refresh_interval_ms + 250).min(5000);
                } else {
                    self.config.refresh_interval_ms = self.config.refresh_interval_ms.saturating_sub(250).max(250);
                }
            }
            _ => {}
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
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = event_loop(&mut terminal, config);

    // Always restore terminal even if we error.
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    config: Config,
) -> anyhow::Result<()> {
    let mut state = AppState::new(config);
    let tick = Duration::from_millis(50); // 20fps draw rate

    // Give collectors a moment to populate before first draw.
    std::thread::sleep(Duration::from_millis(300));

    loop {
        state.tick_time();

        // Check for dead collector threads (they only stop if they panicked).
        let dead = state.hub.dead_collectors();
        if !dead.is_empty() && state.status_message.is_none() {
            state.status_message = Some(format!(
                "Collector crashed: {} — data may be stale",
                dead.join(", ")
            ));
        }

        // Drain any thread expansion results.
        state.drain_thread_results();

        // Update CPU spike flash counters.
        state.update_cpu_spikes();

        // Sort processes and compute visible count before drawing.
        let visible_count = {
            if let Ok(mut procs) = state.hub.processes.write() {
                sort_processes(&mut procs, state.sort_state);
                procs.iter().filter(|p| state.process_matches(p)).count()
            } else {
                0
            }
        };

        // Clamp cursor.
        if state.process_cursor >= visible_count && visible_count > 0 {
            state.process_cursor = visible_count - 1;
        }

        // Draw.
        terminal.draw(|frame| {
            crate::ui::draw(frame, &state);
        })?;

        // Poll for input.
        if event::poll(tick)? {
            if let Event::Key(key) = event::read()? {
                // On Windows, crossterm fires both Press and Release events.
                // Only act on Press to prevent every key from registering twice.
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                let action = handle_key(
                    key,
                    state.filter_active,
                    state.show_kill_confirm,
                    state.show_wt_panel,
                    state.wt_nerd_font_confirm,
                    state.show_settings,
                    state.show_inspect,
                    state.show_pid_jump,
                    state.show_net_filter,
                );
                if action == AppAction::Quit {
                    state.config.save();
                    return Ok(());
                }
                state.dispatch(action, visible_count);
            }
        }
    }
}
