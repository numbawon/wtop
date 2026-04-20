use ratatui::layout::{Constraint, Direction, Layout, Rect};

use crate::config::LayoutMode;

pub struct LayoutRects {
    pub cpu: Rect,
    pub memory: Rect,
    /// `None` when the Disk panel is hidden by the user.
    pub disk: Option<Rect>,
    /// `None` when the Network panel is hidden by the user.
    pub network: Option<Rect>,
    /// `None` when the GPU panel is hidden or no GPU detected.
    pub gpu: Option<Rect>,
    pub processes: Rect,
    pub statusbar: Rect,
}

/// Compute layout rects based on the active `LayoutMode` and panel visibility.
pub fn compute(area: Rect, mode: &LayoutMode, show_disk: bool, show_network: bool, show_gpu: bool) -> LayoutRects {
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
                layout_wide(main_area, statusbar, show_disk, show_network, show_gpu)
            } else if area.width >= 100 {
                layout_stacked(main_area, statusbar, 8, 5, 4, 4, 4, show_disk, show_network, show_gpu)
            } else {
                layout_stacked(main_area, statusbar, 6, 4, 3, 3, 3, show_disk, show_network, show_gpu)
            }
        }
        LayoutMode::Compact => {
            layout_stacked(main_area, statusbar, 5, 3, 3, 3, 3, show_disk, show_network, show_gpu)
        }
        LayoutMode::Wide => {
            layout_wide(main_area, statusbar, show_disk, show_network, show_gpu)
        }
        LayoutMode::Stacked => {
            layout_stacked(main_area, statusbar, 6, 3, 3, 3, 3, show_disk, show_network, show_gpu)
        }
    }
}

/// Wide layout: CPU+Memory stacked on top, Disk|Net side-by-side in the middle,
/// GPU below that, Processes at bottom.
#[allow(clippy::too_many_arguments)]
fn layout_wide(
    area: Rect,
    statusbar: Rect,
    show_disk: bool,
    show_network: bool,
    show_gpu: bool,
) -> LayoutRects {
    let show_mid = show_disk || show_network;

    let mut v_constraints: Vec<Constraint> = vec![
        Constraint::Length(8),
        Constraint::Length(5),
    ];
    if show_mid  { v_constraints.push(Constraint::Length(6)); }
    if show_gpu  { v_constraints.push(Constraint::Length(4)); }
    v_constraints.push(Constraint::Min(10));

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(v_constraints)
        .split(area);

    let mut idx = 2usize;

    let (disk_rect, net_rect) = if show_mid {
        let mid_row = rows[idx];
        idx += 1;
        match (show_disk, show_network) {
            (true, true) => {
                let halves = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(mid_row);
                (Some(halves[0]), Some(halves[1]))
            }
            (true, false) => (Some(mid_row), None),
            (false, true) => (None, Some(mid_row)),
            (false, false) => (None, None),
        }
    } else {
        (None, None)
    };

    let gpu_rect = if show_gpu { let r = rows[idx]; idx += 1; Some(r) } else { None };
    let proc_rect = rows[idx];

    LayoutRects {
        cpu: rows[0],
        memory: rows[1],
        disk: disk_rect,
        network: net_rect,
        gpu: gpu_rect,
        processes: proc_rect,
        statusbar,
    }
}

/// Generic stacked layout. When both disk and network are visible they share one row
/// split 50/50, matching the wide layout behaviour.
#[allow(clippy::too_many_arguments)]
fn layout_stacked(
    area: Rect,
    statusbar: Rect,
    cpu_h: u16,
    mem_h: u16,
    disk_h: u16,
    net_h: u16,
    gpu_h: u16,
    show_disk: bool,
    show_network: bool,
    show_gpu: bool,
) -> LayoutRects {
    let show_mid = show_disk || show_network;
    // When both panels are present, one shared row at the taller height.
    let mid_h = match (show_disk, show_network) {
        (true, true) => disk_h.max(net_h),
        (true, false) => disk_h,
        (false, true) => net_h,
        (false, false) => 0,
    };

    let mut constraints: Vec<Constraint> = vec![
        Constraint::Length(cpu_h),
        Constraint::Length(mem_h),
    ];
    if show_mid { constraints.push(Constraint::Length(mid_h)); }
    if show_gpu { constraints.push(Constraint::Length(gpu_h)); }
    constraints.push(Constraint::Min(6));

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let mut idx = 2usize;

    let (disk_rect, net_rect) = if show_mid {
        let mid_row = rows[idx];
        idx += 1;
        match (show_disk, show_network) {
            (true, true) => {
                let halves = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(mid_row);
                (Some(halves[0]), Some(halves[1]))
            }
            (true, false) => (Some(mid_row), None),
            (false, true) => (None, Some(mid_row)),
            (false, false) => (None, None),
        }
    } else {
        (None, None)
    };

    let gpu_rect = if show_gpu { let r = rows[idx]; idx += 1; Some(r) } else { None };
    let proc_rect = rows[idx];

    LayoutRects {
        cpu: rows[0],
        memory: rows[1],
        disk: disk_rect,
        network: net_rect,
        gpu: gpu_rect,
        processes: proc_rect,
        statusbar,
    }
}
