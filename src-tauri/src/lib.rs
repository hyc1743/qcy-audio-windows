pub mod autostart;
pub mod commands;
pub mod popup;
pub mod tray;

use qcy_core::{AppCore, AppSettingsPatch, CoreEvent};
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    let smoke_test = args.iter().any(|a| a == "--smoke-test");
    let start_minimized = args.iter().any(|a| a == "--minimized");

    let core = if smoke_test {
        let tmp = std::env::temp_dir().join("qcy_desktop_smoke_test_state.json");
        let _ = std::fs::remove_file(&tmp);
        AppCore::with_storage_path(tmp)
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

                    println!(
                        "SMOKE_TEST_OK: models={} active_model={} adapter_present={} classic={} le={} main_win={} popup_win={} tray_registered={} tray_rgba_bytes={}",
                        models.len(),
                        snap.active_model.model_id,
                        adapter.is_present,
                        adapter.is_classic_supported,
                        adapter.is_low_energy_supported,
                        main_ok,
                        popup_ok,
                        tray_registered,
                        tray_rgba_len
                    );
                    let _ = std::io::Write::flush(&mut std::io::stdout());

                    tokio::time::sleep(Duration::from_millis(250)).await;
                    let h_exit = h.clone();
                    let _ = h.run_on_main_thread(move || {
                        tray::teardown_tray(&h_exit);
                        h_exit.exit(0);
                    });
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running QCY Audio Desktop");
}
