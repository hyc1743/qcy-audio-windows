//! Dynamic Windows System Tray icon renderer, tooltip formatter, and `AppCore`-wired quick menu.

use qcy_core::{AncPrimaryMode, AppCore, AppSnapshot, SetAncPayload, SetModesPayload};
use qcy_transport::ConnectionState;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime};

pub const TRAY_ID: &str = "main-tray";
pub const TRAY_ICON_SIZE: u32 = 32;

pub const MENU_SHOW: &str = "tray_show";
pub const MENU_ANC_OFF: &str = "tray_anc_off";
pub const MENU_ANC_ON: &str = "tray_anc_on";
pub const MENU_ANC_TRANSPARENCY: &str = "tray_anc_transparency";
pub const MENU_GAME_MODE: &str = "tray_game_mode";
pub const MENU_PREVIEW_POPUP: &str = "tray_preview_popup";
pub const MENU_OPEN_BT_SETTINGS: &str = "tray_open_bt_settings";
pub const MENU_QUIT: &str = "tray_quit";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAncMode {
    Off,
    NoiseCancellation,
    Transparency,
}

impl TrayAncMode {
    pub fn short_label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::NoiseCancellation => "ANC",
            Self::Transparency => "Transparency",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayBatteryVisual {
    pub device_name: String,
    pub left: Option<u8>,
    pub right: Option<u8>,
    pub case_level: Option<u8>,
    pub charging: bool,
    pub connected: bool,
    pub is_simulator: bool,
    pub anc_mode: TrayAncMode,
    pub game_mode: bool,
}

impl Default for TrayBatteryVisual {
    fn default() -> Self {
        Self {
            device_name: "QCY Audio".to_string(),
            left: Some(88),
            right: Some(92),
            case_level: Some(76),
            charging: false,
            connected: true,
            is_simulator: true,
            anc_mode: TrayAncMode::NoiseCancellation,
            game_mode: false,
        }
    }
}

impl TrayBatteryVisual {
    pub fn from_snapshot(snap: &AppSnapshot) -> Self {
        let device_name = snap
            .active_device
            .as_ref()
            .map(|d| d.name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| snap.active_model.name_en.clone());

        let anc_mode = match snap.anc.mode {
            AncPrimaryMode::Off => TrayAncMode::Off,
            AncPrimaryMode::Anc | AncPrimaryMode::Custom => TrayAncMode::NoiseCancellation,
            AncPrimaryMode::Transparency => TrayAncMode::Transparency,
        };

        Self {
            device_name,
            left: snap.battery.left,
            right: snap.battery.right,
            case_level: snap.battery.case_level,
            charging: snap.battery.left_charging
                || snap.battery.right_charging
                || snap.battery.case_charging,
            connected: snap.connection_state == ConnectionState::Connected,
            is_simulator: snap.is_virtual_session,
            anc_mode,
            game_mode: snap.modes.game_mode || snap.modes.game_mode_pro,
        }
    }
}

/// Formats a concise Windows notification-area tooltip summarizing active device telemetry.
pub fn format_tray_tooltip(state: &TrayBatteryVisual) -> String {
    if !state.connected {
        return "QCY Audio · Disconnected".to_string();
    }

    let mut parts = Vec::new();
    let prefix = if state.is_simulator {
        format!("{} [SIM]", state.device_name)
    } else {
        state.device_name.clone()
    };
    parts.push(prefix);

    let mut bat_segments = Vec::new();
    if let Some(l) = state.left {
        bat_segments.push(format!("L {}%", l.min(100)));
    }
    if let Some(r) = state.right {
        bat_segments.push(format!("R {}%", r.min(100)));
    }
    if let Some(c) = state.case_level {
        bat_segments.push(format!("C {}%", c.min(100)));
    }
    if !bat_segments.is_empty() {
        let mut bat_str = bat_segments.join(" ");
        if state.charging {
            bat_str.push_str(" ⚡");
        }
        parts.push(bat_str);
    }

    parts.push(state.anc_mode.short_label().to_string());
    if state.game_mode {
        parts.push("Game".to_string());
    }

    parts.join(" · ")
}

fn set_px(buf: &mut [u8], x: u32, y: u32, rgba: [u8; 4]) {
    if x >= TRAY_ICON_SIZE || y >= TRAY_ICON_SIZE {
        return;
    }
    let idx = ((y * TRAY_ICON_SIZE + x) * 4) as usize;
    buf[idx..idx + 4].copy_from_slice(&rgba);
}

fn fill_rect(buf: &mut [u8], x0: u32, y0: u32, w: u32, h: u32, rgba: [u8; 4]) {
    for y in y0..(y0 + h).min(TRAY_ICON_SIZE) {
        for x in x0..(x0 + w).min(TRAY_ICON_SIZE) {
            set_px(buf, x, y, rgba);
        }
    }
}

fn is_inside_rounded_box(x: i32, y: i32, min: i32, max: i32, radius: i32) -> bool {
    if x < min || x > max || y < min || y > max {
        return false;
    }
    let dx = if x < min + radius {
        (min + radius) - x
    } else if x > max - radius {
        x - (max - radius)
    } else {
        0
    };
    let dy = if y < min + radius {
        (min + radius) - y
    } else if y > max - radius {
        y - (max - radius)
    } else {
        0
    };
    dx * dx + dy * dy <= radius * radius
}

fn battery_fill_color(pct: u8, charging: bool, connected: bool) -> [u8; 4] {
    if !connected {
        return [0x6E, 0x6E, 0x73, 0xFF];
    }
    if charging {
        return [0x22, 0xC5, 0x5E, 0xFF];
    }
    if pct <= 20 {
        return [0xFF, 0x4C, 0x4C, 0xFF];
    }
    if pct <= 40 {
        return [0xEA, 0xB3, 0x08, 0xFF];
    }
    [0xFF, 0x7A, 0x2F, 0xFF]
}

/// Renders a crisp 32x32 RGBA system tray icon visualizing live earbud battery, charging,
/// disconnected, and simulator status without relying on external bitmap files.
pub fn render_tray_icon_rgba(state: &TrayBatteryVisual) -> Vec<u8> {
    let mut buf = vec![0u8; (TRAY_ICON_SIZE * TRAY_ICON_SIZE * 4) as usize];

    let bg_color = [0x1C, 0x1D, 0x22, 0xFF];
    let border_color = if state.connected {
        [0x3E, 0x41, 0x4B, 0xFF]
    } else {
        [0x32, 0x32, 0x36, 0xFF]
    };

    for y in 0..TRAY_ICON_SIZE as i32 {
        for x in 0..TRAY_ICON_SIZE as i32 {
            if is_inside_rounded_box(x, y, 1, 30, 6) {
                let inner = is_inside_rounded_box(x, y, 2, 29, 5);
                set_px(
                    &mut buf,
                    x as u32,
                    y as u32,
                    if inner { bg_color } else { border_color },
                );
            }
        }
    }

    let accent = if !state.connected {
        [0x73, 0x73, 0x73, 0xFF]
    } else if state.charging {
        [0x22, 0xC5, 0x5E, 0xFF]
    } else {
        [0xFF, 0x7A, 0x2F, 0xFF]
    };
    fill_rect(&mut buf, 7, 4, 18, 2, accent);

    if state.is_simulator && state.connected {
        fill_rect(&mut buf, 25, 4, 3, 3, [0x33, 0x99, 0xFF, 0xFF]);
    }

    let track_bg = [0x2F, 0x32, 0x3B, 0xFF];
    let bar_top = 9u32;
    let bar_height = 18u32;

    match (state.left, state.right) {
        (Some(l_pct), Some(r_pct)) => {
            let l_clamped = l_pct.min(100);
            let r_clamped = r_pct.min(100);

            fill_rect(&mut buf, 6, bar_top, 8, bar_height, track_bg);
            let l_fill = ((bar_height * l_clamped as u32) + 50) / 100;
            if l_fill > 0 {
                let y = bar_top + bar_height - l_fill;
                fill_rect(
                    &mut buf,
                    6,
                    y,
                    8,
                    l_fill,
                    battery_fill_color(l_clamped, state.charging, state.connected),
                );
            }

            fill_rect(&mut buf, 18, bar_top, 8, bar_height, track_bg);
            let r_fill = ((bar_height * r_clamped as u32) + 50) / 100;
            if r_fill > 0 {
                let y = bar_top + bar_height - r_fill;
                fill_rect(
                    &mut buf,
                    18,
                    y,
                    8,
                    r_fill,
                    battery_fill_color(r_clamped, state.charging, state.connected),
                );
            }
        }
        (Some(single), None) | (None, Some(single)) => {
            let pct = single.min(100);
            fill_rect(&mut buf, 9, bar_top, 14, bar_height, track_bg);
            let fill = ((bar_height * pct as u32) + 50) / 100;
            if fill > 0 {
                let y = bar_top + bar_height - fill;
                fill_rect(
                    &mut buf,
                    9,
                    y,
                    14,
                    fill,
                    battery_fill_color(pct, state.charging, state.connected),
                );
            }
        }
        (None, None) => {
            fill_rect(&mut buf, 8, 17, 16, 3, [0x6E, 0x6E, 0x73, 0xFF]);
        }
    }

    if state.connected && state.charging {
        let bolt = [0xFF, 0xFF, 0xFF, 0xFF];
        fill_rect(&mut buf, 15, 11, 2, 4, bolt);
        fill_rect(&mut buf, 14, 15, 4, 2, bolt);
        fill_rect(&mut buf, 15, 17, 2, 4, bolt);
    }

    buf
}

/// Converts the dynamic RGBA buffer into a Tauri `Image`.
pub fn build_tray_image(state: &TrayBatteryVisual) -> Image<'static> {
    Image::new_owned(render_tray_icon_rgba(state), TRAY_ICON_SIZE, TRAY_ICON_SIZE)
}

/// Builds the Windows system tray right-click context menu.
pub fn build_tray_menu<R: Runtime>(
    app: &AppHandle<R>,
    state: &TrayBatteryVisual,
) -> tauri::Result<Menu<R>> {
    let show = MenuItem::with_id(app, MENU_SHOW, "Show QCY Audio", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;

    let anc_off_label = if state.anc_mode == TrayAncMode::Off {
        "● ANC: Off"
    } else {
        "ANC: Off"
    };
    let anc_on_label = if state.anc_mode == TrayAncMode::NoiseCancellation {
        "● ANC: Noise Cancellation"
    } else {
        "ANC: Noise Cancellation"
    };
    let anc_tr_label = if state.anc_mode == TrayAncMode::Transparency {
        "● ANC: Transparency"
    } else {
        "ANC: Transparency"
    };

    let anc_off = MenuItem::with_id(app, MENU_ANC_OFF, anc_off_label, true, None::<&str>)?;
    let anc_on = MenuItem::with_id(app, MENU_ANC_ON, anc_on_label, true, None::<&str>)?;
    let anc_tr = MenuItem::with_id(app, MENU_ANC_TRANSPARENCY, anc_tr_label, true, None::<&str>)?;

    let game_label = if state.game_mode {
        "✓ Game Mode"
    } else {
        "Game Mode"
    };
    let game_mode = MenuItem::with_id(app, MENU_GAME_MODE, game_label, true, None::<&str>)?;

    let sep2 = PredefinedMenuItem::separator(app)?;
    let preview_popup = MenuItem::with_id(
        app,
        MENU_PREVIEW_POPUP,
        "Preview Pop-Up",
        true,
        None::<&str>,
    )?;
    let open_bt = MenuItem::with_id(
        app,
        MENU_OPEN_BT_SETTINGS,
        "Open Windows Bluetooth Settings",
        true,
        None::<&str>,
    )?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "Quit", true, None::<&str>)?;

    Menu::with_items(
        app,
        &[
            &show,
            &sep1,
            &anc_off,
            &anc_on,
            &anc_tr,
            &game_mode,
            &sep2,
            &preview_popup,
            &open_bt,
            &sep3,
            &quit,
        ],
    )
}

/// Initializes the Windows system tray icon, tooltip, left-click restore, and `AppCore`-wired menu.
pub fn setup_tray<R: Runtime>(
    app: &AppHandle<R>,
    core: AppCore,
    initial: &TrayBatteryVisual,
) -> tauri::Result<()> {
    let menu = build_tray_menu(app, initial)?;
    let icon = build_tray_image(initial);
    let tooltip = format_tray_tooltip(initial);

    let _tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip(tooltip)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.unminimize();
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
        })
        .on_menu_event(move |app, event| match event.id.as_ref() {
            MENU_SHOW => {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.unminimize();
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
            MENU_ANC_OFF => {
                let c = core.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = c
                        .set_anc_state(SetAncPayload {
                            mode: AncPrimaryMode::Off,
                            ..Default::default()
                        })
                        .await;
                });
            }
            MENU_ANC_ON => {
                let c = core.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = c
                        .set_anc_state(SetAncPayload {
                            mode: AncPrimaryMode::Anc,
                            ..Default::default()
                        })
                        .await;
                });
            }
            MENU_ANC_TRANSPARENCY => {
                let c = core.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = c
                        .set_anc_state(SetAncPayload {
                            mode: AncPrimaryMode::Transparency,
                            ..Default::default()
                        })
                        .await;
                });
            }
            MENU_GAME_MODE => {
                let c = core.clone();
                tauri::async_runtime::spawn(async move {
                    let snap = c.get_app_snapshot();
                    let next = !(snap.modes.game_mode || snap.modes.game_mode_pro);
                    let _ = c
                        .set_modes(SetModesPayload {
                            game_mode: Some(next),
                            force: true,
                            ..Default::default()
                        })
                        .await;
                });
            }
            MENU_PREVIEW_POPUP => {
                let payload = core.build_popup_payload();
                let _ = crate::popup::show_popup_window(app, Some(&payload));
            }
            MENU_OPEN_BT_SETTINGS => {
                let _ = AppCore::open_windows_bt_settings();
            }
            MENU_QUIT => {
                teardown_tray(app);
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}

/// Refreshes the live tray icon RGBA buffer, tooltip text, and context menu checkmarks.
pub fn update_tray<R: Runtime>(app: &AppHandle<R>, state: &TrayBatteryVisual) -> tauri::Result<()> {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_icon(Some(build_tray_image(state)))?;
        tray.set_tooltip(Some(format_tray_tooltip(state)))?;
        let menu = build_tray_menu(app, state)?;
        tray.set_menu(Some(menu))?;
    }
    Ok(())
}

/// Cleanly removes the tray icon before app shutdown to prevent ghost icons or teardown warnings.
pub fn teardown_tray<R: Runtime>(app: &AppHandle<R>) {
    let _ = app.remove_tray_by_id(TRAY_ID);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contains_rgba(buf: &[u8], target: [u8; 4]) -> bool {
        buf.as_chunks::<4>().0.contains(&target)
    }

    #[test]
    fn tray_icon_rgba_dimensions_and_state_colors() {
        let normal = TrayBatteryVisual {
            device_name: "QCY MeloBuds Pro".to_string(),
            left: Some(85),
            right: Some(90),
            case_level: Some(60),
            charging: false,
            connected: true,
            is_simulator: false,
            anc_mode: TrayAncMode::NoiseCancellation,
            game_mode: false,
        };
        let buf_normal = render_tray_icon_rgba(&normal);
        assert_eq!(
            buf_normal.len(),
            (TRAY_ICON_SIZE * TRAY_ICON_SIZE * 4) as usize
        );
        assert!(contains_rgba(&buf_normal, [0xFF, 0x7A, 0x2F, 0xFF]));
        assert!(!contains_rgba(&buf_normal, [0x33, 0x99, 0xFF, 0xFF]));

        let low = TrayBatteryVisual {
            left: Some(15),
            right: Some(18),
            ..normal.clone()
        };
        let buf_low = render_tray_icon_rgba(&low);
        assert!(contains_rgba(&buf_low, [0xFF, 0x4C, 0x4C, 0xFF]));

        let charging_sim = TrayBatteryVisual {
            charging: true,
            is_simulator: true,
            ..normal.clone()
        };
        let buf_chg = render_tray_icon_rgba(&charging_sim);
        assert!(contains_rgba(&buf_chg, [0x22, 0xC5, 0x5E, 0xFF]));
        assert!(contains_rgba(&buf_chg, [0x33, 0x99, 0xFF, 0xFF]));
        assert!(contains_rgba(&buf_chg, [0xFF, 0xFF, 0xFF, 0xFF]));

        let disconnected = TrayBatteryVisual {
            connected: false,
            left: None,
            right: None,
            case_level: None,
            ..normal
        };
        let buf_disc = render_tray_icon_rgba(&disconnected);
        assert_eq!(buf_disc.len(), 4096);
        assert!(contains_rgba(&buf_disc, [0x6E, 0x6E, 0x73, 0xFF]));
        assert!(!contains_rgba(&buf_disc, [0xFF, 0x7A, 0x2F, 0xFF]));
    }

    #[test]
    fn tray_tooltip_formatting_covers_connected_charging_sim_and_disconnected() {
        let tws = TrayBatteryVisual {
            device_name: "QCY MeloBuds Pro".to_string(),
            left: Some(85),
            right: Some(90),
            case_level: Some(60),
            charging: false,
            connected: true,
            is_simulator: false,
            anc_mode: TrayAncMode::NoiseCancellation,
            game_mode: false,
        };
        assert_eq!(
            format_tray_tooltip(&tws),
            "QCY MeloBuds Pro · L 85% R 90% C 60% · ANC"
        );

        let sim_charging = TrayBatteryVisual {
            charging: true,
            is_simulator: true,
            game_mode: true,
            anc_mode: TrayAncMode::Transparency,
            ..tws.clone()
        };
        assert_eq!(
            format_tray_tooltip(&sim_charging),
            "QCY MeloBuds Pro [SIM] · L 85% R 90% C 60% ⚡ · Transparency · Game"
        );

        let disconnected = TrayBatteryVisual {
            connected: false,
            ..tws
        };
        assert_eq!(
            format_tray_tooltip(&disconnected),
            "QCY Audio · Disconnected"
        );
    }
}
