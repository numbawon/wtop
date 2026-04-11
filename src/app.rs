use std::io;
use std::time::Duration;

use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

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
    pub process_offset: usize,
    pub sort_state: SortState,
    pub filter_active: bool,
    pub filter_text: String,
    pub show_system_processes: bool,
    pub show_help: bool,
    pub show_kill_confirm: bool,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let hub = CollectorHub::spawn(&config);
        let show_sys = config.show_system_processes;
        Self {
            config,
            hub,
            focused_panel: FocusedPanel::Processes,
            process_cursor: 0,
            process_offset: 0,
            sort_state: SortState::default(),
            filter_active: false,
            filter_text: String::new(),
            show_system_processes: show_sys,
            show_help: false,
            show_kill_confirm: false,
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
            AppAction::KillProcess => self.show_kill_confirm = true,
            AppAction::ToggleSystemProcesses => {
                self.show_system_processes = !self.show_system_processes;
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
        if !self.show_system_processes && p.user == "SYSTEM" {
            return false;
        }
        if !self.filter_text.is_empty() {
            return p.name.to_lowercase().contains(&self.filter_text.to_lowercase());
        }
        true
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
                let action = handle_key(key, state.filter_active);
                if action == AppAction::Quit {
                    return Ok(());
                }
                state.dispatch(action, visible_count);
            }
        }
    }
}
