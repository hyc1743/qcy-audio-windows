//! Taskbar-aware bottom-right Connection Pop-Up overlay window coordinator.

use qcy_core::PopupTriggerPayload;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Runtime, WebviewWindow};

pub const POPUP_WINDOW_LABEL: &str = "popup";
pub const POPUP_EVENT_TRIGGERED: &str = "qcy://popup-triggered";
pub const DEFAULT_POPUP_WIDTH_LOGICAL: f64 = 360.0;
pub const DEFAULT_POPUP_HEIGHT_LOGICAL: f64 = 260.0;
pub const DEFAULT_POPUP_MARGIN_LOGICAL: f64 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonitorWorkArea {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PopupCoords {
    pub x: i32,
    pub y: i32,
}

/// Computes taskbar-safe bottom-right physical coordinates for the popup window inside a monitor's
/// work area, clamping cleanly to the work area origin on small or ultra-high-DPI screens.
pub fn compute_bottom_right_position(
    work_area: MonitorWorkArea,
    popup_logical_width: f64,
    popup_logical_height: f64,
    margin_logical: f64,
) -> PopupCoords {
    let scale = if work_area.scale_factor.is_finite() && work_area.scale_factor > 0.0 {
        work_area.scale_factor
    } else {
        1.0
    };

    let popup_w = (popup_logical_width.max(1.0) * scale).round() as i32;
    let popup_h = (popup_logical_height.max(1.0) * scale).round() as i32;
    let margin = (margin_logical.max(0.0) * scale).round() as i32;

    let right_edge = work_area.x.saturating_add(work_area.width as i32);
    let bottom_edge = work_area.y.saturating_add(work_area.height as i32);

    let x = right_edge
        .saturating_sub(popup_w)
        .saturating_sub(margin)
        .max(work_area.x);
    let y = bottom_edge
        .saturating_sub(popup_h)
        .saturating_sub(margin)
        .max(work_area.y);

    PopupCoords { x, y }
}

/// Positions the secondary `popup` window in the primary monitor's bottom-right work area
/// (automatically accounting for Windows taskbar height and DPI scaling).
pub fn position_popup_bottom_right<R: Runtime>(
    window: &WebviewWindow<R>,
) -> Result<PopupCoords, String> {
    let monitor = window
        .primary_monitor()
        .map_err(|e| format!("failed to query primary monitor: {e}"))?
        .ok_or_else(|| "no primary monitor available".to_string())?;

    let scale_factor = monitor.scale_factor();
    let wa = monitor.work_area();

    let (logical_w, logical_h) = match window.outer_size() {
        Ok(sz) if sz.width > 0 && sz.height > 0 => (
            sz.width as f64 / scale_factor.max(0.1),
            sz.height as f64 / scale_factor.max(0.1),
        ),
        _ => (DEFAULT_POPUP_WIDTH_LOGICAL, DEFAULT_POPUP_HEIGHT_LOGICAL),
    };

    let coords = compute_bottom_right_position(
        MonitorWorkArea {
            x: wa.position.x,
            y: wa.position.y,
            width: wa.size.width,
            height: wa.size.height,
            scale_factor,
        },
        logical_w,
        logical_h,
        DEFAULT_POPUP_MARGIN_LOGICAL,
    );

    window
        .set_position(PhysicalPosition::new(coords.x, coords.y))
        .map_err(|e| format!("failed to position popup window: {e}"))?;

    Ok(coords)
}

/// Emits `PopupTriggerPayload` on `qcy://popup-triggered`, positions the `popup` window cleanly
/// in the bottom-right work area, and displays it without stealing focus from active apps.
pub fn show_popup_window<R: Runtime>(
    app: &AppHandle<R>,
    payload: Option<&PopupTriggerPayload>,
) -> Result<(), String> {
    if let Some(data) = payload {
        app.emit(POPUP_EVENT_TRIGGERED, data)
            .map_err(|e| format!("failed to emit popup payload: {e}"))?;
    }

    if let Some(win) = app.get_webview_window(POPUP_WINDOW_LABEL) {
        let _ = position_popup_bottom_right(&win);
        let _ = win.set_always_on_top(true);
        win.show()
            .map_err(|e| format!("failed to show popup window: {e}"))?;
    }
    Ok(())
}

/// Hides the `popup` window cleanly.
pub fn hide_popup_window<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(POPUP_WINDOW_LABEL) {
        win.hide()
            .map_err(|e| format!("failed to hide popup window: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bottom_right_position_standard_1080p_100pct() {
        let coords = compute_bottom_right_position(
            MonitorWorkArea {
                x: 0,
                y: 0,
                width: 1920,
                height: 1040,
                scale_factor: 1.0,
            },
            360.0,
            230.0,
            16.0,
        );
        assert_eq!(coords, PopupCoords { x: 1544, y: 794 });
    }

    #[test]
    fn bottom_right_position_hidpi_4k_200pct() {
        let coords = compute_bottom_right_position(
            MonitorWorkArea {
                x: 0,
                y: 0,
                width: 3840,
                height: 2080,
                scale_factor: 2.0,
            },
            360.0,
            230.0,
            16.0,
        );
        assert_eq!(coords, PopupCoords { x: 3088, y: 1588 });
    }

    #[test]
    fn bottom_right_position_respects_nonzero_origin_and_clamps_small_screens() {
        let coords = compute_bottom_right_position(
            MonitorWorkArea {
                x: 64,
                y: 32,
                width: 300,
                height: 200,
                scale_factor: 1.5,
            },
            360.0,
            230.0,
            16.0,
        );
        assert_eq!(coords, PopupCoords { x: 64, y: 32 });
    }
}
