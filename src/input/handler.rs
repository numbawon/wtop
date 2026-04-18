use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Active modal/overlay state - passed to `handle_key` to determine which
/// modal block gets priority.
#[derive(Default)]
pub struct ModalState {
    pub filter_active: bool,
    pub kill_confirm_active: bool,
    pub wt_panel_active: bool,
    pub wt_nerd_font_confirm_active: bool,
    pub settings_active: bool,
    pub inspect_active: bool,
    pub inspect_close_confirm_active: bool,
    pub pid_jump_active: bool,
    pub net_filter_active: bool,
    pub help_active: bool,
    pub name_search_active: bool,
}

/// All actions the UI can dispatch in response to key events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppAction {
    Quit,
    MoveUp,
    MoveDown,
    PageUp,
    PageDown,
    Home,
    End,
    ExpandCollapse,
    NextPanel,
    PrevPanel,
    SortNext,
    SortPrev,
    ToggleSortOrder,
    OpenFilter,
    CloseFilter,
    FilterChar(char),
    FilterBackspace,
    FilterEsc,
    KillProcess,
    ConfirmKill,
    CancelKill,
    ToggleSystemProcesses,
    ToggleUserFilter,
    IncreaseRefresh,
    DecreaseRefresh,
    ToggleHelp,
    ToggleNerdGlyphs,
    CycleTheme,
    CycleLayout,
    ToggleDisk,
    ToggleNetwork,
    ToggleDiskColumns,
    ToggleWtPanel,
    WtConfirmNerdFont,
    WtApplyNerdFont,
    WtCancelNerdFont,
    ToggleSettings,
    SettingsUp,
    SettingsDown,
    SettingsActivate,
    SettingsActivateBack,
    ToggleInspect,
    InspectNextTab,
    InspectInitCloseHandle,
    ConfirmCloseHandle,
    CancelCloseHandle,
    OpenPidJump,
    PidJumpChar(char),
    PidJumpBackspace,
    PidJumpConfirm,
    PidJumpCancel,
    NetFilterUp,
    NetFilterDown,
    NetFilterToggle,
    NetFilterClose,
    InspectScrollLeft,
    InspectScrollRight,
    InspectCopyLine,
    ToggleTreeView,
    OpenNameSearch,
    NameSearchChar(char),
    NameSearchBackspace,
    NameSearchConfirm,
    NameSearchCancel,
    None,
}

/// Map a crossterm KeyEvent to an AppAction.
pub fn handle_key(key: KeyEvent, m: &ModalState) -> AppAction {
    // When the inspect overlay is open, handle its own modal states first.
    if m.inspect_active {
        if m.inspect_close_confirm_active {
            return match key.code {
                KeyCode::Enter => AppAction::ConfirmCloseHandle,
                KeyCode::Esc   => AppAction::CancelCloseHandle,
                _ => AppAction::None,
            };
        }
        return match key.code {
            KeyCode::Char('i') | KeyCode::Esc    => AppAction::ToggleInspect,
            KeyCode::Up                          => AppAction::MoveUp,
            KeyCode::Down                        => AppAction::MoveDown,
            KeyCode::PageUp                      => AppAction::PageUp,
            KeyCode::PageDown                    => AppAction::PageDown,
            KeyCode::Tab                         => AppAction::InspectNextTab,
            KeyCode::Char('x')                   => AppAction::InspectInitCloseHandle,
            KeyCode::Left                        => AppAction::InspectScrollLeft,
            KeyCode::Right                       => AppAction::InspectScrollRight,
            KeyCode::Char('y')                   => AppAction::InspectCopyLine,
            _ => AppAction::None,
        };
    }

    // When the PID jump box is open, only digits / confirm / cancel are accepted.
    if m.pid_jump_active {
        return match key.code {
            KeyCode::Enter     => AppAction::PidJumpConfirm,
            KeyCode::Esc       => AppAction::PidJumpCancel,
            KeyCode::Backspace => AppAction::PidJumpBackspace,
            KeyCode::Char(c) if c.is_ascii_digit() => AppAction::PidJumpChar(c),
            _ => AppAction::None,
        };
    }

    // When the net filter overlay is open.
    if m.net_filter_active {
        return match key.code {
            KeyCode::Esc | KeyCode::Char('q')         => AppAction::NetFilterClose,
            KeyCode::Up                               => AppAction::NetFilterUp,
            KeyCode::Down                             => AppAction::NetFilterDown,
            KeyCode::Enter | KeyCode::Char(' ')       => AppAction::NetFilterToggle,
            _ => AppAction::None,
        };
    }

    // When the kill confirm dialog is open, only allow confirm or cancel.
    if m.kill_confirm_active {
        return match key.code {
            KeyCode::Enter => AppAction::ConfirmKill,
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('q') => AppAction::CancelKill,
            _ => AppAction::None,
        };
    }

    // When the Nerd Font confirmation sub-dialog is active.
    if m.wt_nerd_font_confirm_active {
        return match key.code {
            KeyCode::Enter => AppAction::WtApplyNerdFont,
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('q') => AppAction::WtCancelNerdFont,
            _ => AppAction::None,
        };
    }

    // When the WT info panel is open, only panel-specific keys work.
    if m.wt_panel_active {
        return match key.code {
            KeyCode::Esc | KeyCode::Char('q')         => AppAction::ToggleWtPanel,
            KeyCode::Char('w') | KeyCode::Char('W')   => AppAction::ToggleWtPanel,
            KeyCode::Char('f') | KeyCode::Char('F')   => AppAction::WtConfirmNerdFont,
            _ => AppAction::None,
        };
    }

    // When the settings panel is open, only settings keys work.
    if m.settings_active {
        return match (key.modifiers, key.code) {
            (_, KeyCode::Esc)
            | (KeyModifiers::SHIFT, KeyCode::Char('C'))
            | (KeyModifiers::SHIFT, KeyCode::Char('c')) => AppAction::ToggleSettings,
            (_, KeyCode::Up)   => AppAction::SettingsUp,
            (_, KeyCode::Down) => AppAction::SettingsDown,
            (_, KeyCode::Enter) | (_, KeyCode::Right)    => AppAction::SettingsActivate,
            (_, KeyCode::Left)                           => AppAction::SettingsActivateBack,
            _ => AppAction::None,
        };
    }

    // When help overlay is open, Esc (or ? / h) dismisses it.
    if m.help_active {
        return match key.code {
            KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('h') => AppAction::ToggleHelp,
            _ => AppAction::None,
        };
    }

    // When the name search box is open.
    if m.name_search_active {
        return match key.code {
            KeyCode::Enter     => AppAction::NameSearchConfirm,
            KeyCode::Esc       => AppAction::NameSearchCancel,
            KeyCode::Backspace => AppAction::NameSearchBackspace,
            KeyCode::Char(c)   => AppAction::NameSearchChar(c),
            _ => AppAction::None,
        };
    }

    // When the filter bar is open, most keys feed into the search string.
    if m.filter_active {
        return match key.code {
            KeyCode::Esc       => AppAction::FilterEsc,
            KeyCode::Backspace => AppAction::FilterBackspace,
            KeyCode::Char(c)   => AppAction::FilterChar(c),
            KeyCode::Enter     => AppAction::CloseFilter,
            _ => AppAction::None,
        };
    }

    match (key.modifiers, key.code) {
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => AppAction::Quit,
        (_, KeyCode::Char('q')) => AppAction::Quit,
        (_, KeyCode::Up) => AppAction::MoveUp,
        (_, KeyCode::Down) => AppAction::MoveDown,
        (_, KeyCode::PageUp) => AppAction::PageUp,
        (_, KeyCode::PageDown) => AppAction::PageDown,
        (_, KeyCode::Home) => AppAction::Home,
        (_, KeyCode::End) => AppAction::End,
        (_, KeyCode::Enter) => AppAction::ExpandCollapse,
        (_, KeyCode::Tab) => AppAction::NextPanel,
        (KeyModifiers::SHIFT, KeyCode::BackTab) => AppAction::PrevPanel,
        (KeyModifiers::NONE, KeyCode::Char('s')) => AppAction::SortNext,
        (KeyModifiers::SHIFT, KeyCode::Char('S')) | (KeyModifiers::SHIFT, KeyCode::Char('s')) => AppAction::SortPrev,
        (_, KeyCode::Char('r')) => AppAction::ToggleSortOrder,
        (_, KeyCode::Char('f')) => AppAction::OpenFilter,
        (KeyModifiers::SHIFT, KeyCode::Char('K')) | (KeyModifiers::SHIFT, KeyCode::Char('k')) => AppAction::KillProcess,
        (_, KeyCode::Char('p')) => AppAction::ToggleSystemProcesses,
        (_, KeyCode::Char('u')) => AppAction::ToggleUserFilter,
        (_, KeyCode::Char('+')) => AppAction::IncreaseRefresh,
        (_, KeyCode::Char('-')) => AppAction::DecreaseRefresh,
        (_, KeyCode::Char('?')) | (_, KeyCode::Char('h')) => AppAction::ToggleHelp,
        (KeyModifiers::CONTROL, KeyCode::Char('g')) => AppAction::OpenPidJump,
        (_, KeyCode::Char('g')) => AppAction::ToggleNerdGlyphs,
        (KeyModifiers::SHIFT, KeyCode::Char('T')) => AppAction::CycleTheme,
        (KeyModifiers::SHIFT, KeyCode::Char('L')) => AppAction::CycleLayout,
        (_, KeyCode::Char('d')) => AppAction::ToggleDisk,
        (_, KeyCode::Char('n')) => AppAction::ToggleNetwork,
        (KeyModifiers::NONE, KeyCode::Char('c')) => AppAction::ToggleDiskColumns,
        (_, KeyCode::Char('w')) => AppAction::ToggleWtPanel,
        (KeyModifiers::SHIFT, KeyCode::Char('C')) | (KeyModifiers::SHIFT, KeyCode::Char('c')) => AppAction::ToggleSettings,
        (_, KeyCode::Char('i')) => AppAction::ToggleInspect,
        (_, KeyCode::Char('t')) => AppAction::ToggleTreeView,
        (_, KeyCode::Char('/')) => AppAction::OpenNameSearch,
        _ => AppAction::None,
    }
}
