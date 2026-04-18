use ratatui::layout::{Constraint, Direction, Layout, Rect};

use crate::config::LayoutMode;

pub struct LayoutRects {
    pub cpu: Rect,
    pub memory: Rect,
    /// `None` when the Disk panel is hidden by the user.
    pub disk: Option<Rect>,
    /// `None` when the Network panel is hidden by the user.
    pub network: Option<Rect>,
    pub processes: Rect,
    pub statusbar: Rect,
}

/// Compute layout rects based on the active `LayoutMode` and panel visibility.
pub fn compute(area: Rect, mode: &LayoutMode, show_disk: bool, show_network: bool) -> LayoutRects {
    // Reserve bottom 1 row for the status/keybindings bar.
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let main_area = outer[0];
    let statusbar = outer[1];

    match mode {
        LayoutMode::Auto => {
            if area.width >= 160 {
                layout_wide(main_area, statusbar, show_disk, show_network)
            } else if area.width >= 100 {
                layout_stacked(main_area, statusbar, 8, 5, 4, 4, show_disk, show_network)
            } else {
                layout_stacked(main_area, statusbar, 6, 4, 3, 3, show_disk, show_network)
            }
        }
        LayoutMode::Compact => {
            layout_stacked(main_area, statusbar, 5, 3, 3, 3, show_disk, show_network)
        }
        LayoutMode::Wide => {
            layout_wide(main_area, statusbar, show_disk, show_network)
        }
        LayoutMode::Stacked => {
            layout_stacked(main_area, statusbar, 6, 3, 3, 3, show_disk, show_network)
        }
    }
}

/// Wide layout: CPU and Memory stacked on top, Disk|Net side-by-side in the middle, Processes at bottom.
fn layout_wide(area: Rect, statusbar: Rect, show_disk: bool, show_network: bool) -> LayoutRects {
    match (show_disk, show_network) {
        (true, true) => {
            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(8),
                    Constraint::Length(5),
                    Constraint::Length(6),
                    Constraint::Min(10),
                ])
                .split(area);

            let middle = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(rows[2]);

            LayoutRects {
                cpu: rows[0],
                memory: rows[1],
                disk: Some(middle[0]),
                network: Some(middle[1]),
                processes: rows[3],
                statusbar,
            }
        }
        (true, false) => {
            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(8),
                    Constraint::Length(5),
                    Constraint::Length(6),
                    Constraint::Min(10),
                ])
                .split(area);
            LayoutRects {
                cpu: rows[0],
                memory: rows[1],
                disk: Some(rows[2]),
                network: None,
                processes: rows[3],
                statusbar,
            }
        }
        (false, true) => {
            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(8),
                    Constraint::Length(5),
                    Constraint::Length(6),
                    Constraint::Min(10),
                ])
                .split(area);
            LayoutRects {
                cpu: rows[0],
                memory: rows[1],
                disk: None,
                network: Some(rows[2]),
                processes: rows[3],
                statusbar,
            }
        }
        (false, false) => {
            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(8),
                    Constraint::Length(5),
                    Constraint::Min(10),
                ])
                .split(area);
            LayoutRects {
                cpu: rows[0],
                memory: rows[1],
                disk: None,
                network: None,
                processes: rows[2],
                statusbar,
            }
        }
    }
}

/// Generic stacked (single-column) layout with configurable panel heights.
/// Omits disk/network rows when hidden, giving that space to processes.
#[allow(clippy::too_many_arguments)]
fn layout_stacked(
    area: Rect,
    statusbar: Rect,
    cpu_h: u16,
    mem_h: u16,
    disk_h: u16,
    net_h: u16,
    show_disk: bool,
    show_network: bool,
) -> LayoutRects {
    let mut constraints: Vec<Constraint> = vec![
        Constraint::Length(cpu_h),
        Constraint::Length(mem_h),
    ];
    if show_disk    { constraints.push(Constraint::Length(disk_h)); }
    if show_network { constraints.push(Constraint::Length(net_h)); }
    constraints.push(Constraint::Min(6));

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let mut idx = 2usize;
    let disk_rect = if show_disk    { let r = rows[idx]; idx += 1; Some(r) } else { None };
    let net_rect  = if show_network { let r = rows[idx]; idx += 1; Some(r) } else { None };
    let proc_rect = rows[idx];

    LayoutRects {
        cpu: rows[0],
        memory: rows[1],
        disk: disk_rect,
        network: net_rect,
        processes: proc_rect,
        statusbar,
    }
}
