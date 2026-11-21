pub mod autostart;
pub mod commands;
pub mod popup;
pub mod tray;

use qcy_core::{AppCore, AppSettingsPatch, CoreEvent};
use qcy_transport::ConnectionState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{Emitter, Manager, WindowEvent};

pub const EVENT_STATE_UPDATED: &str = "qcy://state-updated";
pub const EVENT_SCAN_DISCOVERED: &str = "qcy://scan-discovered";
pub const EVENT_PROTOCOL_LOG: &str = "qcy://protocol-log";

/// Lock-free desktop shell flags read synchronously from Tauri window/event callbacks.
#[derive(Clone)]
pub struct ShellRuntime {
    pub minimize_to_tray: Arc<AtomicBool>,
    pub show_connection_popup: Arc<AtomicBool>,
}

impl ShellRuntime {
    pub fn new(minimize_to_tray: bool, show_connection_popup: bool) -> Self {
        Self {
            minimize_to_tray: Arc::new(AtomicBool::new(minimize_to_tray)),
            show_connection_popup: Arc::new(AtomicBool::new(show_connection_popup)),
        }
    }
}

/// Returns `true` when a WebView URL points to Tauri's embedded custom-protocol origin.
pub fn is_embedded_app_url(url: &str) -> bool {
    url.starts_with("http://tauri.localhost")
        || url.starts_with("https://tauri.localhost")
        || url.starts_with("tauri://localhost")
}

/// Input bundle validated by `--smoke-test` during CI and local portable packaging.
#[derive(Debug, Clone)]
pub struct SmokeCheckInput<'a> {
    pub require_embedded: bool,
    pub dev_build: bool,
    pub main_ok: bool,
    pub popup_ok: bool,
    pub main_url: &'a str,
    pub popup_url: &'a str,
    pub index_html_bytes: usize,
    pub index_has_script: bool,
    pub simulator_enabled: bool,
    pub connection_state: ConnectionState,
    pub has_active_device: bool,
    pub battery_empty: bool,
}

/// Validates smoke-test startup defaults, window URLs, and embedded frontend asset integrity.
pub fn validate_smoke_state(input: &SmokeCheckInput<'_>) -> Result<(), Vec<String>> {
    let mut failures = Vec::new();
    if !input.main_ok {
        failures.push("main window missing".to_string());
    }
    if !input.popup_ok {
        failures.push("popup window missing".to_string());
    }
    if input.simulator_enabled {
        failures.push("default startup must have simulator_enabled=false".to_string());
    }
    if input.connection_state != ConnectionState::Disconnected {
        failures.push(format!(
            "default startup must be disconnected (got {:?})",
            input.connection_state
        ));
    }
    if input.has_active_device {
        failures.push("default startup must have active_device=none".to_string());
    }
    if !input.battery_empty {
        failures.push("default startup must not fabricate battery levels".to_string());
    }
    if input.require_embedded {
        if input.dev_build {
            failures.push(
                "release binary compiled with cfg(dev) (missing custom-protocol feature)"
                    .to_string(),
            );
        }
        if input.main_url.contains("localhost:1420") || input.popup_url.contains("localhost:1420") {
            failures.push(format!(
                "window navigated to devUrl localhost:1420 (main={}, popup={})",
                input.main_url, input.popup_url
            ));
        }
        if !is_embedded_app_url(input.main_url) {
            failures.push(format!(
                "main_url is not embedded custom protocol: {}",
                input.main_url
            ));
        }
        if !is_embedded_app_url(input.popup_url) {
            failures.push(format!(
                "popup_url is not embedded custom protocol: {}",
                input.popup_url
            ));
        }
        if input.index_html_bytes == 0 || !input.index_has_script {
            failures.push(format!(
                "embedded index.html invalid (bytes={}, has_script={})",
                input.index_html_bytes, input.index_has_script
            ));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures)
    }
}

fn connection_state_label(state: ConnectionState) -> &'static str {
    match state {
        ConnectionState::Disconnected => "disconnected",
        ConnectionState::Connecting => "connecting",
        ConnectionState::Connected => "connected",
        ConnectionState::Reconnecting => "reconnecting",
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    let smoke_test = args.iter().any(|a| a == "--smoke-test");
    let start_minimized = args.iter().any(|a| a == "--minimized");

    let smoke_storage_path = if smoke_test {
        let tmp = std::env::temp_dir().join(format!(
            "qcy_desktop_smoke_test_state_{}.json",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&tmp);
        Some(tmp)
    } else {
        None
    };

    let core = if let Some(ref tmp) = smoke_storage_path {
        AppCore::with_storage_path(tmp.clone())
    } else {
        AppCore::new()
    };

    let initial_snap = core.get_app_snapshot();
    let shell = ShellRuntime::new(
        initial_snap.settings.minimize_to_tray,
        initial_snap.settings.show_connection_popup,
    );

    let shell_for_window = shell.clone();
    let core_for_setup = core.clone();
    let shell_for_setup = shell.clone();

    tauri::Builder::default()
        .manage(core)
        .manage(shell)
        .invoke_handler(tauri::generate_handler![
            commands::get_app_snapshot,
            commands::list_models,
            commands::get_adapter_info,
            commands::start_scan,
            commands::stop_scan,
            commands::list_paired_devices,
            commands::connect_device,
            commands::disconnect_device,
            commands::remove_saved_device,
            commands::open_windows_bt_settings,
            commands::set_anc_state,
            commands::set_ai_anc,
            commands::set_anc_button_cycle,
            commands::apply_eq_preset,
            commands::set_graphic_eq,
            commands::set_parametric_eq,
            commands::save_custom_eq,
            commands::delete_custom_eq,
            commands::import_eq_json,
            commands::export_eq_json,
            commands::apply_personalized_eq,
            commands::set_audio_enhancers,
            commands::set_button_action,
            commands::reset_buttons,
            commands::set_touch_config,
            commands::set_head_controls,
            commands::set_modes,
            commands::set_multipoint,
            commands::set_rgb_lighting,
            commands::set_focus_white_noise,
            commands::run_fit_test,
            commands::set_device_color,
            commands::rename_device,
            commands::reset_device,
            commands::start_local_ota,
            commands::cancel_ota,
            commands::set_simulator_enabled,
            commands::set_simulator_model,
            commands::update_simulator_state,
            commands::trigger_simulator_popup,
            commands::update_app_settings,
            commands::show_popup,
            commands::hide_popup,
            commands::get_protocol_logs,
            commands::clear_protocol_logs,
        ])
        .on_window_event(move |window, event| {
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    if shell_for_window.minimize_to_tray.load(Ordering::SeqCst) {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
            }
        })
        .setup(move |app| {
            let app_handle = app.handle().clone();

            if start_minimized || smoke_test {
                if let Some(main_win) = app.get_webview_window("main") {
                    let _ = main_win.hide();
                }
            }

            // Initialize Windows system tray icon + live battery tooltip + AppCore-wired menu.
            let snap = core_for_setup.get_app_snapshot();
            let initial_tray = tray::TrayBatteryVisual::from_snapshot(&snap);
            tray::setup_tray(&app_handle, core_for_setup.clone(), &initial_tray)?;

            // Sync OS autostart state into AppCore settings cleanly (non-smoke mode).
            if !smoke_test {
                let os_autostart = autostart::is_autostart_enabled();
                if snap.settings.auto_start != os_autostart {
                    let _ = core_for_setup.update_app_settings(AppSettingsPatch {
                        auto_start: Some(os_autostart),
                        ..Default::default()
                    });
                }
            }

            // Spawn background Tokio pump forwarding `CoreEvent` to Tauri events, tray, and popup.
            let mut rx = core_for_setup.subscribe_events();
            let pump_handle = app_handle.clone();
            let pump_shell = shell_for_setup.clone();
            tauri::async_runtime::spawn(async move {
                while let Ok(event) = rx.recv().await {
                    match event {
                        CoreEvent::StateUpdated(snap) => {
                            pump_shell
                                .minimize_to_tray
                                .store(snap.settings.minimize_to_tray, Ordering::SeqCst);
                            pump_shell
                                .show_connection_popup
                                .store(snap.settings.show_connection_popup, Ordering::SeqCst);
                            let _ = pump_handle.emit(EVENT_STATE_UPDATED, &*snap);
                            let h_tray = pump_handle.clone();
                            let visual = tray::TrayBatteryVisual::from_snapshot(&snap);
                            let _ = pump_handle.run_on_main_thread(move || {
                                let _ = tray::update_tray(&h_tray, &visual);
                            });
                        }
                        CoreEvent::PopupTriggered(payload) => {
                            if pump_shell.show_connection_popup.load(Ordering::SeqCst) {
                                let _ = popup::show_popup_window(&pump_handle, Some(&payload));
                            }
                        }
                        CoreEvent::ScanDiscovered(dev) => {
                            let _ = pump_handle.emit(EVENT_SCAN_DISCOVERED, &dev);
                        }
                        CoreEvent::ProtocolLog(entry) => {
                            let _ = pump_handle.emit(EVENT_PROTOCOL_LOG, &entry);
                        }
                    }
                }
            });

            // Self-verifying `--smoke-test` mode for CI and local release checks.
            if smoke_test {
                let c = core_for_setup.clone();
                let h = app_handle.clone();
                let main_ok = app.get_webview_window("main").is_some();
                let popup_ok = app.get_webview_window("popup").is_some();
                let dev_build = cfg!(dev);
                let require_embedded = cfg!(not(debug_assertions)) || !dev_build;
                let embedded_index = app.asset_resolver().get("index.html".into());
                let index_html_bytes = embedded_index.as_ref().map(|a| a.bytes.len()).unwrap_or(0);
                let index_has_script = embedded_index
                    .as_ref()
                    .map(|a| String::from_utf8_lossy(&a.bytes).contains("<script"))
                    .unwrap_or(false);

                tauri::async_runtime::spawn(async move {
                    let models = c.list_models();
                    let snap = c.get_app_snapshot();
                    let adapter = c.get_adapter_info().await;
                    let tray_visual = tray::TrayBatteryVisual::from_snapshot(&snap);

                    let (tx, rx) = tokio::sync::oneshot::channel();
                    let h_tray = h.clone();
                    let tray_rgba_len = tray::render_tray_icon_rgba(&tray_visual).len();
                    let tray_registered = h.tray_by_id(tray::TRAY_ID).is_some();
                    let _ = h.run_on_main_thread(move || {
                        let _ = tray::update_tray(&h_tray, &tray_visual);
                        let _ = tx.send(true);
                    });
                    let _ = rx.await;

                    // Wait briefly for WebView2 initial navigation so win.url() settles
                    // (up to 10s ceiling for cold CI runners; exits immediately once settled).
                    let mut main_url = String::new();
                    let mut popup_url = String::new();
                    for _ in 0..100 {
                        let (url_tx, url_rx) = tokio::sync::oneshot::channel();
                        let h_urls = h.clone();
                        let _ = h.run_on_main_thread(move || {
                            let m = h_urls
                                .get_webview_window("main")
                                .and_then(|w| w.url().ok())
                                .map(|u| u.to_string())
                                .unwrap_or_default();
                            let p = h_urls
                                .get_webview_window("popup")
                                .and_then(|w| w.url().ok())
                                .map(|u| u.to_string())
                                .unwrap_or_default();
                            let _ = url_tx.send((m, p));
                        });
                        if let Ok((m, p)) = url_rx.await {
                            main_url = m;
                            popup_url = p;
                        }
                        let settled = !main_url.is_empty()
                            && main_url != "about:blank"
                            && !popup_url.is_empty()
                            && popup_url != "about:blank";
                        if settled {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }

                    let simulator_enabled = snap.settings.simulator_enabled;
                    let conn_label = connection_state_label(snap.connection_state);
                    let active_device_label = snap
                        .active_device
                        .as_ref()
                        .map(|d| d.name.as_str())
                        .unwrap_or("none");
                    let battery_empty = snap.battery.left.is_none()
                        && snap.battery.right.is_none()
                        && snap.battery.case_level.is_none();

                    let validation = validate_smoke_state(&SmokeCheckInput {
                        require_embedded,
                        dev_build,
                        main_ok,
                        popup_ok,
                        main_url: &main_url,
                        popup_url: &popup_url,
                        index_html_bytes,
                        index_has_script,
                        simulator_enabled,
                        connection_state: snap.connection_state,
                        has_active_device: snap.active_device.is_some(),
                        battery_empty,
                    });

                    let exit_code = match validation {
                        Ok(()) => {
                            println!(
                                "SMOKE_TEST_OK: models={} active_model={} simulator_enabled={} connection_state={} active_device={} adapter_present={} classic={} le={} main_win={} popup_win={} tray_registered={} tray_rgba_bytes={} dev_build={} main_url={} popup_url={} index_html_bytes={} index_has_script={}",
                                models.len(),
                                snap.active_model.model_id,
                                simulator_enabled,
                                conn_label,
                                active_device_label,
                                adapter.is_present,
                                adapter.is_classic_supported,
                                adapter.is_low_energy_supported,
                                main_ok,
                                popup_ok,
                                tray_registered,
                                tray_rgba_len,
                                dev_build,
                                main_url,
                                popup_url,
                                index_html_bytes,
                                index_has_script
                            );
                            let _ = std::io::Write::flush(&mut std::io::stdout());
                            0
                        }
                        Err(failures) => {
                            eprintln!(
                                "SMOKE_TEST_FAIL: {} | simulator_enabled={} connection_state={} active_device={} dev_build={} main_url={} popup_url={} index_html_bytes={} index_has_script={}",
                                failures.join("; "),
                                simulator_enabled,
                                conn_label,
                                active_device_label,
                                dev_build,
                                main_url,
                                popup_url,
                                index_html_bytes,
                                index_has_script
                            );
                            let _ = std::io::Write::flush(&mut std::io::stderr());
                            1
                        }
                    };

                    if let Some(ref tmp) = smoke_storage_path {
                        let _ = std::fs::remove_file(tmp);
                    }

                    tokio::time::sleep(Duration::from_millis(150)).await;
                    let h_exit = h.clone();
                    let _ = h.run_on_main_thread(move || {
                        tray::teardown_tray(&h_exit);
                        h_exit.exit(exit_code);
                    });
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running QCY Audio Desktop");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_app_url_classifier_accepts_custom_protocol_and_rejects_dev_url() {
        assert!(is_embedded_app_url("http://tauri.localhost/"));
        assert!(is_embedded_app_url(
            "http://tauri.localhost/index.html?window=popup"
        ));
        assert!(is_embedded_app_url("https://tauri.localhost/"));
        assert!(is_embedded_app_url("tauri://localhost/"));
        assert!(!is_embedded_app_url("http://localhost:1420/"));
        assert!(!is_embedded_app_url(
            "http://localhost:1420/index.html?window=popup"
        ));
        assert!(!is_embedded_app_url("about:blank"));
        assert!(!is_embedded_app_url(""));
    }

    #[test]
    fn smoke_state_validation_blocks_dev_url_and_default_simulator_regressions() {
        assert!(validate_smoke_state(&SmokeCheckInput {
            require_embedded: true,
            dev_build: false,
            main_ok: true,
            popup_ok: true,
            main_url: "http://tauri.localhost/",
            popup_url: "http://tauri.localhost/index.html?window=popup",
            index_html_bytes: 480,
            index_has_script: true,
            simulator_enabled: false,
            connection_state: ConnectionState::Disconnected,
            has_active_device: false,
            battery_empty: true,
        })
        .is_ok());

        let err = validate_smoke_state(&SmokeCheckInput {
            require_embedded: true,
            dev_build: true,
            main_ok: true,
            popup_ok: true,
            main_url: "http://localhost:1420/",
            popup_url: "http://localhost:1420/index.html?window=popup",
            index_html_bytes: 0,
            index_has_script: false,
            simulator_enabled: true,
            connection_state: ConnectionState::Connected,
            has_active_device: true,
            battery_empty: false,
        })
        .unwrap_err();
        assert!(err.iter().any(|m| m.contains("custom-protocol")));
        assert!(err.iter().any(|m| m.contains("localhost:1420")));
        assert!(err.iter().any(|m| m.contains("embedded index.html")));
        assert!(err.iter().any(|m| m.contains("simulator_enabled=false")));
        assert!(err.iter().any(|m| m.contains("disconnected")));
        assert!(err.iter().any(|m| m.contains("active_device=none")));
    }
}
