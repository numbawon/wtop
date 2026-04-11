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
    ToggleSystemProcesses,
    ToggleUserFilter,
    IncreaseRefresh,
    DecreaseRefresh,
    ToggleHelp,
    None,
}

/// Map a crossterm KeyEvent to an AppAction.
pub fn handle_key(key: KeyEvent, filter_active: bool) -> AppAction {
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
        _ => AppAction::None,
    }
}
