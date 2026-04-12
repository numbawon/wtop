use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
    None,
}

/// Map a crossterm KeyEvent to an AppAction.
pub fn handle_key(
    key: KeyEvent,
    filter_active: bool,
    kill_confirm_active: bool,
    wt_panel_active: bool,
    wt_nerd_font_confirm_active: bool,
    settings_active: bool,
) -> AppAction {
    // When the kill confirm dialog is open, only allow confirm or cancel.
    if kill_confirm_active {
        return match key.code {
            KeyCode::Enter => AppAction::ConfirmKill,
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('q') => AppAction::CancelKill,
            _ => AppAction::None,
        };
    }

    // When the Nerd Font confirmation sub-dialog is active.
    if wt_nerd_font_confirm_active {
        return match key.code {
            KeyCode::Enter => AppAction::WtApplyNerdFont,
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('q') => AppAction::WtCancelNerdFont,
            _ => AppAction::None,
        };
    }

    // When the WT info panel is open, only panel-specific keys work.
    if wt_panel_active {
        return match key.code {
            KeyCode::Esc | KeyCode::Char('q') => AppAction::ToggleWtPanel,
            KeyCode::Char('w') | KeyCode::Char('W') => AppAction::ToggleWtPanel,
            KeyCode::Char('f') | KeyCode::Char('F') => AppAction::WtConfirmNerdFont,
            _ => AppAction::None,
        };
    }

    // When the settings panel is open, only settings keys work.
    if settings_active {
        return match (key.modifiers, key.code) {
            (_, KeyCode::Esc) | (KeyModifiers::SHIFT, KeyCode::Char('C')) => AppAction::ToggleSettings,
            (_, KeyCode::Up)   | (_, KeyCode::Char('k')) => AppAction::SettingsUp,
            (_, KeyCode::Down) | (_, KeyCode::Char('j')) => AppAction::SettingsDown,
            (_, KeyCode::Enter) | (_, KeyCode::Right)    => AppAction::SettingsActivate,
            (_, KeyCode::Left)                           => AppAction::SettingsActivateBack,
            _ => AppAction::None,
        };
    }

    // When the filter bar is open, most keys feed into the search string.
    if filter_active {
        return match key.code {
            KeyCode::Esc => AppAction::CloseFilter,
            KeyCode::Backspace => AppAction::FilterBackspace,
            KeyCode::Char(c) => AppAction::FilterChar(c),
            KeyCode::Enter => AppAction::CloseFilter,
            _ => AppAction::None,
        };
    }

    match (key.modifiers, key.code) {
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => AppAction::Quit,
        (_, KeyCode::Char('q')) => AppAction::Quit,
        (_, KeyCode::Up) => AppAction::MoveUp,
        (_, KeyCode::Char('k')) => AppAction::MoveUp,
        (_, KeyCode::Down) => AppAction::MoveDown,
        (_, KeyCode::Char('j')) => AppAction::MoveDown,
        (_, KeyCode::PageUp) => AppAction::PageUp,
        (_, KeyCode::PageDown) => AppAction::PageDown,
        (_, KeyCode::Home) => AppAction::Home,
        (_, KeyCode::End) => AppAction::End,
        (_, KeyCode::Enter) => AppAction::ExpandCollapse,
        (_, KeyCode::Tab) => AppAction::NextPanel,
        (KeyModifiers::SHIFT, KeyCode::BackTab) => AppAction::PrevPanel,
        (_, KeyCode::Char('s')) => AppAction::SortNext,
        (KeyModifiers::SHIFT, KeyCode::Char('S')) => AppAction::SortPrev,
        (_, KeyCode::Char('r')) => AppAction::ToggleSortOrder,
        (_, KeyCode::Char('f')) => AppAction::OpenFilter,
        (KeyModifiers::SHIFT, KeyCode::Char('K')) => AppAction::KillProcess,
        (_, KeyCode::Char('p')) => AppAction::ToggleSystemProcesses,
        (_, KeyCode::Char('u')) => AppAction::ToggleUserFilter,
        (_, KeyCode::Char('+')) => AppAction::IncreaseRefresh,
        (_, KeyCode::Char('-')) => AppAction::DecreaseRefresh,
        (_, KeyCode::Char('?')) | (_, KeyCode::Char('h')) => AppAction::ToggleHelp,
        (_, KeyCode::Char('g')) => AppAction::ToggleNerdGlyphs,
        (KeyModifiers::SHIFT, KeyCode::Char('T')) => AppAction::CycleTheme,
        (KeyModifiers::SHIFT, KeyCode::Char('L')) => AppAction::CycleLayout,
        (_, KeyCode::Char('d')) => AppAction::ToggleDisk,
        (_, KeyCode::Char('n')) => AppAction::ToggleNetwork,
        (_, KeyCode::Char('c')) => AppAction::ToggleDiskColumns,
        (_, KeyCode::Char('w')) => AppAction::ToggleWtPanel,
        (KeyModifiers::SHIFT, KeyCode::Char('C')) => AppAction::ToggleSettings,
        _ => AppAction::None,
    }
}
