#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalPoint {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TaskbarEdge {
    Left,
    Right,
    Top,
    Bottom,
}

pub fn calculate_popup_position(
    monitor: PhysicalRect,
    work_area: PhysicalRect,
    tray_rect: PhysicalRect,
    popup_size_dip: (u32, u32),
    scale_factor: f64,
) -> PhysicalPoint {
    let (popup_width, popup_height) = popup_size_physical(popup_size_dip, scale_factor);
    let popup_width = popup_width as i32;
    let popup_height = popup_height as i32;
    let tray_width = tray_rect.width as i32;
    let tray_height = tray_rect.height as i32;

    let edge = taskbar_edge(monitor, work_area, tray_rect);
    let (x, y) = match edge {
        TaskbarEdge::Left => (
            tray_rect.x + tray_width,
            tray_rect.y + tray_height / 2 - popup_height / 2,
        ),
        TaskbarEdge::Right => (
            tray_rect.x - popup_width,
            tray_rect.y + tray_height / 2 - popup_height / 2,
        ),
        TaskbarEdge::Top => (
            tray_rect.x + tray_width / 2 - popup_width / 2,
            tray_rect.y + tray_height,
        ),
        TaskbarEdge::Bottom => (
            tray_rect.x + tray_width / 2 - popup_width / 2,
            tray_rect.y - popup_height,
        ),
    };

    PhysicalPoint {
        x: clamp_axis(x, work_area.x, work_area.width as i32, popup_width),
        y: clamp_axis(y, work_area.y, work_area.height as i32, popup_height),
    }
}

pub fn popup_size_physical(popup_size_dip: (u32, u32), scale_factor: f64) -> (u32, u32) {
    (
        (f64::from(popup_size_dip.0) * scale_factor).round() as u32,
        (f64::from(popup_size_dip.1) * scale_factor).round() as u32,
    )
}

#[cfg(windows)]
pub fn adjust_with_windows(
    ideal: PhysicalPoint,
    tray_rect: PhysicalRect,
    popup_size: (u32, u32),
) -> PhysicalPoint {
    use windows::Win32::{
        Foundation::{POINT, RECT, SIZE},
        UI::WindowsAndMessaging::{CalculatePopupWindowPosition, TPM_WORKAREA},
    };

    let anchor = POINT {
        x: ideal.x,
        y: ideal.y,
    };
    let size = SIZE {
        cx: popup_size.0 as i32,
        cy: popup_size.1 as i32,
    };
    let exclude = RECT {
        left: tray_rect.x,
        top: tray_rect.y,
        right: tray_rect.x + tray_rect.width as i32,
        bottom: tray_rect.y + tray_rect.height as i32,
    };
    let mut result = RECT::default();
    let adjusted = unsafe {
        CalculatePopupWindowPosition(&anchor, &size, TPM_WORKAREA.0, Some(&exclude), &mut result)
    };
    if adjusted.is_ok() {
        PhysicalPoint {
            x: result.left,
            y: result.top,
        }
    } else {
        ideal
    }
}

#[cfg(not(windows))]
pub fn adjust_with_windows(
    ideal: PhysicalPoint,
    _tray_rect: PhysicalRect,
    _popup_size: (u32, u32),
) -> PhysicalPoint {
    ideal
}

fn clamp_axis(value: i32, start: i32, available: i32, popup: i32) -> i32 {
    value.clamp(start, start + (available - popup).max(0))
}

fn taskbar_edge(
    monitor: PhysicalRect,
    work_area: PhysicalRect,
    tray_rect: PhysicalRect,
) -> TaskbarEdge {
    let monitor_right = monitor.x + monitor.width as i32;
    let monitor_bottom = monitor.y + monitor.height as i32;
    let work_right = work_area.x + work_area.width as i32;
    let work_bottom = work_area.y + work_area.height as i32;
    let gaps = [
        (work_area.x - monitor.x, TaskbarEdge::Left),
        (monitor_right - work_right, TaskbarEdge::Right),
        (work_area.y - monitor.y, TaskbarEdge::Top),
        (monitor_bottom - work_bottom, TaskbarEdge::Bottom),
    ];
    if let Some((gap, edge)) = gaps.into_iter().max_by_key(|(gap, _)| *gap) {
        if gap > 0 {
            return edge;
        }
    }

    let center_x = tray_rect.x + tray_rect.width as i32 / 2;
    let center_y = tray_rect.y + tray_rect.height as i32 / 2;
    [
        ((center_x - monitor.x).abs(), TaskbarEdge::Left),
        ((monitor_right - center_x).abs(), TaskbarEdge::Right),
        ((center_y - monitor.y).abs(), TaskbarEdge::Top),
        ((monitor_bottom - center_y).abs(), TaskbarEdge::Bottom),
    ]
    .into_iter()
    .min_by_key(|(distance, _)| *distance)
    .map(|(_, edge)| edge)
    .unwrap_or(TaskbarEdge::Bottom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchors_above_bottom_taskbar_and_clamps_to_work_area() {
        assert_eq!(
            calculate_popup_position(
                PhysicalRect {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080
                },
                PhysicalRect {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1040
                },
                PhysicalRect {
                    x: 1800,
                    y: 1040,
                    width: 24,
                    height: 40
                },
                (960, 680),
                1.0,
            ),
            PhysicalPoint { x: 960, y: 360 }
        );
    }

    #[test]
    fn anchors_below_top_taskbar() {
        assert_eq!(
            calculate_popup_position(
                PhysicalRect {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080
                },
                PhysicalRect {
                    x: 0,
                    y: 40,
                    width: 1920,
                    height: 1040
                },
                PhysicalRect {
                    x: 1800,
                    y: 0,
                    width: 24,
                    height: 40
                },
                (960, 680),
                1.0,
            ),
            PhysicalPoint { x: 960, y: 40 }
        );
    }

    #[test]
    fn handles_negative_monitor_coordinates_and_mixed_dpi() {
        assert_eq!(
            calculate_popup_position(
                PhysicalRect {
                    x: -1280,
                    y: 0,
                    width: 1280,
                    height: 1024
                },
                PhysicalRect {
                    x: -1240,
                    y: 0,
                    width: 1240,
                    height: 1024
                },
                PhysicalRect {
                    x: -1280,
                    y: 700,
                    width: 40,
                    height: 24
                },
                (360, 420),
                1.25,
            ),
            PhysicalPoint { x: -1240, y: 450 }
        );
    }

    #[test]
    fn anchors_left_of_right_taskbar_at_target_dpi() {
        assert_eq!(
            calculate_popup_position(
                PhysicalRect {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080
                },
                PhysicalRect {
                    x: 0,
                    y: 0,
                    width: 1880,
                    height: 1080
                },
                PhysicalRect {
                    x: 1880,
                    y: 500,
                    width: 40,
                    height: 24
                },
                (360, 420),
                1.5,
            ),
            PhysicalPoint { x: 1340, y: 197 }
        );
    }
}
