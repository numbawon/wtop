use std::io;
use std::time::Duration;

use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};

use crate::collectors::CollectorHub;
use crate::config::Config;
use crate::input::handler::{handle_key, AppAction};
use crate::models::process::{sort_processes, SortState};

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
    pub show_system_processes: bool,
    pub user_filter_active: bool,
    pub current_user: String,
    pub show_help: bool,
    pub show_kill_confirm: bool,
    /// The (pid, name) of the process targeted for kill while the confirm dialog is open.
    pub kill_target: Option<(u32, String)>,
    /// Transient status message shown in the status bar (e.g. kill errors).
    /// Cleared on the next non-trivial key action.
    pub status_message: Option<String>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let hub = CollectorHub::spawn(&config);
        let show_sys = config.show_system_processes;
        let current_user = std::env::var("USERNAME").unwrap_or_default();
        Self {
            config,
            hub,
            focused_panel: FocusedPanel::Processes,
            process_cursor: 0,
            sort_state: SortState::default(),
            filter_active: false,
            filter_text: String::new(),
            show_system_processes: show_sys,
            user_filter_active: false,
            current_user,
            show_help: false,
            show_kill_confirm: false,
            kill_target: None,
            status_message: None,
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
                if self.process_cursor > 0 {
                    self.process_cursor -= 1;
                }
            }
            AppAction::MoveDown => {
                if self.process_cursor + 1 < visible_count {
                    self.process_cursor += 1;
                }
            }
            AppAction::PageUp => {
                self.process_cursor = self.process_cursor.saturating_sub(20);
            }
            AppAction::PageDown => {
                self.process_cursor = (self.process_cursor + 20).min(visible_count.saturating_sub(1));
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
            AppAction::FilterChar(c) => self.filter_text.push(c),
            AppAction::FilterBackspace => { self.filter_text.pop(); }
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
            AppAction::ToggleHelp => self.show_help = !self.show_help,
            AppAction::IncreaseRefresh => {
                self.config.refresh_interval_ms =
                    (self.config.refresh_interval_ms + 250).min(5000);
            }
            AppAction::DecreaseRefresh => {
                self.config.refresh_interval_ms =
                    self.config.refresh_interval_ms.saturating_sub(250).max(250);
            }
            _ => {}
        }
    }

    fn process_matches(&self, p: &crate::models::process::ProcessEntry) -> bool {
        if !self.show_system_processes && is_system_account(&p.user) {
            return false;
        }
        if self.user_filter_active
            && !self.current_user.is_empty()
            && !p.user.to_lowercase().contains(&self.current_user.to_lowercase())
        {
            return false;
        }
        if !self.filter_text.is_empty() {
            return p.name.to_lowercase().contains(&self.filter_text.to_lowercase());
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

/// Run the application: set up the terminal, enter the event loop, restore on exit.
pub fn run(config: Config) -> anyhow::Result<()> {
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
        // Drain any thread expansion results.
        state.drain_thread_results();

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
                let action = handle_key(key, state.filter_active, state.show_kill_confirm);
                if action == AppAction::Quit {
                    return Ok(());
                }
                state.dispatch(action, visible_count);
            }
        }
    }
}
