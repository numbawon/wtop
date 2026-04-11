use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub struct LayoutRects {
    pub cpu: Rect,
    pub memory: Rect,
    pub disk: Rect,
    pub network: Rect,
    pub processes: Rect,
    pub statusbar: Rect,
}

/// Compute layout rects based on terminal width.
pub fn compute(area: Rect) -> LayoutRects {
    // Reserve bottom 1 row for the status/keybindings bar.
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let main_area = outer[0];
    let statusbar = outer[1];

    if area.width >= 160 {
        layout_wide(main_area, statusbar)
    } else if area.width >= 100 {
        layout_medium(main_area, statusbar)
    } else {
        layout_narrow(main_area, statusbar)
    }
}

/// Wide layout (≥160 cols): CPU+Mem top-half, Disk|Net side-by-side middle, Processes bottom.
fn layout_wide(area: Rect, statusbar: Rect) -> LayoutRects {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),  // CPU
            Constraint::Length(5),  // Mem
            Constraint::Length(6),  // Disk | Net
            Constraint::Min(10),    // Processes
        ])
        .split(area);

    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[2]);

    LayoutRects {
        cpu: rows[0],
        memory: rows[1],
        disk: middle[0],
        network: middle[1],
        processes: rows[3],
        statusbar,
    }
}

/// Medium layout (100–159 cols): disk and net stacked vertically.
fn layout_medium(area: Rect, statusbar: Rect) -> LayoutRects {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Length(5),
            Constraint::Length(4),
            Constraint::Length(4),
            Constraint::Min(8),
        ])
        .split(area);

    LayoutRects {
        cpu: rows[0],
        memory: rows[1],
        disk: rows[2],
        network: rows[3],
        processes: rows[4],
        statusbar,
    }
}

/// Narrow layout (<100 cols): everything stacked, disk+net compressed.
fn layout_narrow(area: Rect, statusbar: Rect) -> LayoutRects {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(4),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(6),
        ])
        .split(area);

    LayoutRects {
        cpu: rows[0],
        memory: rows[1],
        disk: rows[2],
        network: rows[3],
        processes: rows[4],
        statusbar,
    }
}
