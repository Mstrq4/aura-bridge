use claude_bridge_core::{app_sync, load_config, start_bridge_on_listener, BridgeState};
use std::sync::Arc;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_updater::UpdaterExt;

const TRAY_ONLINE_PNG: &[u8] = include_bytes!("../../assets/tray-online.png");
const TRAY_OFFLINE_PNG: &[u8] = include_bytes!("../../assets/tray-offline.png");

struct AppBridgeHandle {
    bridge_url: String,
    state: Arc<BridgeState>,
}

#[tauri::command]
fn get_bridge_url(handle: tauri::State<'_, AppBridgeHandle>) -> String {
    handle.bridge_url.clone()
}

#[tauri::command]
fn reload_bridge_config(handle: tauri::State<'_, AppBridgeHandle>) {
    handle.state.reload_config();
}

#[tauri::command]
async fn install_github_update(app: tauri::AppHandle) -> Result<String, String> {
    let updater = app.updater().map_err(|e| format!("Updater init failed: {e}"))?;
    let update = updater
        .check()
        .await
        .map_err(|e| format!("Update check failed: {e}"))?;
    match update {
        Some(upd) => {
            let ver = upd.version.clone();
            upd.download_and_install(|_, _| {}, || {})
                .await
                .map_err(|e| format!("Install failed: {e}"))?;
            Ok(format!("Updated to v{ver}. Please restart the application."))
        }
        None => Ok("Already running the latest version.".to_string()),
    }
}

fn focus_main_window(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
    }
}

pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    let start_minimized = args.iter().any(|a| a == "--minimized" || a == "--tray");

    let cfg = load_config();
    let host = if cfg.host.trim().is_empty() {
        "127.0.0.1".to_string()
    } else {
        cfg.host.clone()
    };
    let port = cfg.port;
    let bridge_url = format!("http://{host}:{port}");
    let app_version = env!("CARGO_PKG_VERSION").to_string();
    let bridge_state = BridgeState::new(app_version);

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            focus_main_window(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppBridgeHandle {
            bridge_url: bridge_url.clone(),
            state: bridge_state.clone(),
        })
        .invoke_handler(tauri::generate_handler![
            get_bridge_url,
            reload_bridge_config,
            install_github_update
        ])
        .setup(move |app| {
            // Synchronously bind socket BEFORE showing window so UI never sees cold boot offline flash
            let mut listener_bound = false;
            match std::net::TcpListener::bind((host.as_str(), port)) {
                Ok(std_listener) => {
                    if std_listener.set_nonblocking(true).is_ok() {
                        listener_bound = true;
                        let st = bridge_state.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Ok(tokio_listener) =
                                tokio::net::TcpListener::from_std(std_listener)
                            {
                                let _ = start_bridge_on_listener(tokio_listener, st).await;
                            }
                        });
                    }
                }
                Err(err) => {
                    eprintln!("Bridge bind warning on {host}:{port}: {err}");
                }
            }

            // Build native System Tray menu
            let item_open =
                MenuItem::with_id(app, "open", "فتح لوحة التحكم | Open Dashboard", true, None::<&str>)?;
            let item_sync_code =
                MenuItem::with_id(app, "sync_code", "مزامنة Claude Code CLI", true, None::<&str>)?;
            let item_sync_dsh =
                MenuItem::with_id(app, "sync_dsh", "مزامنة DeepSeek Harness", true, None::<&str>)?;
            let item_reload =
                MenuItem::with_id(app, "reload", "إعادة تحميل الإعدادات (.env)", true, None::<&str>)?;
            let item_sep = PredefinedMenuItem::separator(app)?;
            let item_quit =
                MenuItem::with_id(app, "quit", "خروج نهائي | Quit", true, None::<&str>)?;

            let tray_menu = Menu::with_items(
                app,
                &[
                    &item_open,
                    &item_sync_code,
                    &item_sync_dsh,
                    &item_reload,
                    &item_sep,
                    &item_quit,
                ],
            )?;

            let icon_bytes = if listener_bound {
                TRAY_ONLINE_PNG
            } else {
                TRAY_OFFLINE_PNG
            };
            let tray_img = Image::from_bytes(icon_bytes).or_else(|_| {
                app.default_window_icon()
                    .cloned()
                    .ok_or_else(|| tauri::Error::AssetNotFound("tray icon".into()))
            })?;

            let st_for_menu = bridge_state.clone();
            TrayIconBuilder::with_id("main-tray")
                .icon(tray_img)
                .tooltip("Claude Switch Pro — Aura Bridge (18787)")
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        focus_main_window(tray.app_handle());
                    }
                })
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "open" => focus_main_window(app),
                    "sync_code" => {
                        let c = st_for_menu.config.read().unwrap().clone();
                        let _ = app_sync::apply_claude_code(&c);
                    }
                    "sync_dsh" => {
                        let c = st_for_menu.config.read().unwrap().clone();
                        let _ = app_sync::apply_deepseek_harness(&c);
                    }
                    "reload" => {
                        st_for_menu.reload_config();
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            if start_minimized {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.hide();
                }
            } else {
                focus_main_window(app.handle());
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        });

    builder
        .run(tauri::generate_context!())
        .expect("error while running Claude Switch Pro");
}
