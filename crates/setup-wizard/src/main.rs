#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use claude_bridge_core::{app_sync::apply_all_detected, load_config, telemetry::TelemetryStore};
use serde::Deserialize;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const WIZARD_HTML: &str = include_str!("../../../ui/installer.html");
const APP_PAYLOAD: &[u8] = include_bytes!(env!("CSP_PAYLOAD_FILE"));
const APP_ICON_ICO: &[u8] = include_bytes!("../../../assets/app.ico");
const NOTIFY_ICON_ICO: &[u8] = include_bytes!("../../../assets/notify.ico");
const TRAY_ONLINE_PNG: &[u8] = include_bytes!("../../../assets/tray-online.png");
const TRAY_OFFLINE_PNG: &[u8] = include_bytes!("../../../assets/tray-offline.png");

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(windows)]
const DETACHED_PROCESS: u32 = 0x0000_0008;
#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

#[derive(Debug, Clone)]
enum WizardEvent {
    PathPicked(String),
    StepProgress {
        step_id: String,
        status: String,
        progress: u8,
        log_line: String,
    },
    InstallDone {
        ok: bool,
        installed_path: String,
        error: String,
    },
    RequestClose,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IpcMessage {
    cmd: String,
    #[serde(default)]
    install_dir: Option<String>,
    #[serde(default)]
    current_path: Option<String>,
    #[serde(default = "default_true")]
    desktop_shortcut: bool,
    #[serde(default = "default_true")]
    start_menu_shortcut: bool,
    #[serde(default = "default_true")]
    auto_start: bool,
    #[serde(default = "default_true")]
    auto_sync: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone)]
struct InstallOptions {
    install_dir: PathBuf,
    desktop_shortcut: bool,
    start_menu_shortcut: bool,
    auto_start: bool,
    auto_sync: bool,
}

fn default_install_dir() -> PathBuf {
    #[cfg(windows)]
    {
        let local = std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(r"C:\Users\Default\AppData\Local"));
        local.join("Programs").join("ClaudeSwitchPro")
    }
    #[cfg(not(windows))]
    {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"));
        home.join(".local").join("share").join("ClaudeSwitchPro")
    }
}

fn current_arch_label() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

fn stop_running_instances() {
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "claude-switch-pro.exe"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        thread::sleep(Duration::from_millis(350));
    }
}

fn ps_escape(s: &str) -> String {
    s.replace('\'', "''")
}

#[cfg(windows)]
fn run_hidden_powershell(script: &str) -> Result<String, String> {
    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW);
    let out = cmd.output().map_err(|e| format!("PowerShell launch failed: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

fn pick_folder_dialog(initial: &str) -> Option<String> {
    #[cfg(windows)]
    {
        let escaped = ps_escape(initial);
        let script = format!(
            "Add-Type -AssemblyName System.Windows.Forms; \
             $d = New-Object System.Windows.Forms.FolderBrowserDialog; \
             $d.Description = 'Select installation folder for Claude Switch Pro'; \
             if (Test-Path '{escaped}') {{ $d.SelectedPath = '{escaped}' }}; \
             if ($d.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {{ Write-Output $d.SelectedPath }}"
        );
        let mut cmd = Command::new("powershell");
        cmd.args(["-STA", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &script])
            .creation_flags(CREATE_NO_WINDOW);
        if let Ok(out) = cmd.output() {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = initial;
    }
    None
}

fn perform_installation<F>(opts: &InstallOptions, mut emit: F) -> Result<PathBuf, String>
where
    F: FnMut(&str, &str, u8, &str),
{
    let bin_name = if cfg!(windows) {
        "claude-switch-pro.exe"
    } else {
        "claude-switch-pro"
    };
    let target_exe = opts.install_dir.join(bin_name);
    let assets_dir = opts.install_dir.join("assets");

    emit(
        "stop_existing",
        "running",
        10,
        "[1/6] Stopping existing instances & preparing install directory...",
    );
    stop_running_instances();
    fs::create_dir_all(&assets_dir)
        .map_err(|e| format!("Cannot create install dir {}: {e}", opts.install_dir.display()))?;
    emit(
        "stop_existing",
        "done",
        18,
        &format!("[1/6] Target directory ready: {}", opts.install_dir.display()),
    );

    emit(
        "extract_bin",
        "running",
        28,
        "[2/6] Extracting unified Tauri desktop executable & icons...",
    );
    if APP_PAYLOAD.len() <= 1024 {
        return Err(
            "Embedded app payload missing in this dev/stub build. Rebuild after `claude-switch-pro.exe`."
                .to_string(),
        );
    }
    fs::write(&target_exe, APP_PAYLOAD)
        .map_err(|e| format!("Failed writing {}: {e}", target_exe.display()))?;
    let _ = fs::write(assets_dir.join("app.ico"), APP_ICON_ICO);
    let _ = fs::write(assets_dir.join("notify.ico"), NOTIFY_ICON_ICO);
    let _ = fs::write(assets_dir.join("tray-online.png"), TRAY_ONLINE_PNG);
    let _ = fs::write(assets_dir.join("tray-offline.png"), TRAY_OFFLINE_PNG);
    emit(
        "extract_bin",
        "done",
        40,
        &format!(
            "[2/6] Deployed unified binary ({:.2} MB).",
            APP_PAYLOAD.len() as f64 / (1024.0 * 1024.0)
        ),
    );

    emit(
        "migrate_data",
        "running",
        50,
        "[3/6] Loading and migrating .env & telemetry database...",
    );
    let cfg = load_config();
    let _ = TelemetryStore::load_or_migrate();
    emit(
        "migrate_data",
        "done",
        60,
        "[3/6] Local configuration & telemetry store ready.",
    );

    emit(
        "shortcuts",
        "running",
        70,
        "[4/6] Creating OS shortcuts & Add/Remove Programs registration...",
    );
    #[cfg(windows)]
    {
        let ver = env!("CARGO_PKG_VERSION");
        let exe_str = ps_escape(&target_exe.to_string_lossy());
        let dir_str = ps_escape(&opts.install_dir.to_string_lossy());
        let ico_str = ps_escape(&assets_dir.join("app.ico").to_string_lossy());

        let uninstall_cmd = opts.install_dir.join("Uninstall.cmd");
        let uninstall_body = format!(
            "@echo off\r\n\
             taskkill /F /IM claude-switch-pro.exe >nul 2>&1\r\n\
             timeout /t 1 /nobreak >nul\r\n\
             powershell -NoProfile -ExecutionPolicy Bypass -Command \"\
             $d = [Environment]::GetFolderPath('Desktop'); Remove-Item -Force -ErrorAction SilentlyContinue ($d + '\\Claude Switch Pro.lnk'); \
             $sm = [Environment]::GetFolderPath('Programs'); Remove-Item -Force -ErrorAction SilentlyContinue ($sm + '\\Claude Switch Pro.lnk'); \
             Remove-ItemProperty -Path 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run' -Name 'ClaudeSwitchPro' -ErrorAction SilentlyContinue; \
             Remove-Item -Path 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\ClaudeSwitchPro' -Recurse -Force -ErrorAction SilentlyContinue\"\r\n\
             start \"\" /b cmd /c \"timeout /t 2 >nul & rmdir /S /Q \\\"{}\\\"\"\r\n",
            opts.install_dir.display()
        );
        let _ = fs::write(&uninstall_cmd, uninstall_body);
        let uninst_str = ps_escape(&uninstall_cmd.to_string_lossy());

        let mut script = String::from("$ws = New-Object -ComObject WScript.Shell; ");
        if opts.desktop_shortcut {
            script.push_str(&format!(
                "$d = [Environment]::GetFolderPath('Desktop'); \
                 $s1 = $ws.CreateShortcut($d + '\\Claude Switch Pro.lnk'); \
                 $s1.TargetPath = '{exe_str}'; \
                 $s1.IconLocation = '{ico_str},0'; \
                 $s1.Description = 'Claude Switch Pro — Aura Bridge Control Center'; \
                 $s1.WorkingDirectory = '{dir_str}'; \
                 $s1.Save(); "
            ));
        }
        if opts.start_menu_shortcut {
            script.push_str(&format!(
                "$sm = [Environment]::GetFolderPath('Programs'); \
                 $s2 = $ws.CreateShortcut($sm + '\\Claude Switch Pro.lnk'); \
                 $s2.TargetPath = '{exe_str}'; \
                 $s2.IconLocation = '{ico_str},0'; \
                 $s2.Description = 'Claude Switch Pro'; \
                 $s2.WorkingDirectory = '{dir_str}'; \
                 $s2.Save(); \
                 $reg = 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\ClaudeSwitchPro'; \
                 New-Item -Path $reg -Force | Out-Null; \
                 Set-ItemProperty -Path $reg -Name 'DisplayName' -Value 'Claude Switch Pro — Aura Bridge Control Center'; \
                 Set-ItemProperty -Path $reg -Name 'DisplayVersion' -Value '{ver}'; \
                 Set-ItemProperty -Path $reg -Name 'Publisher' -Value 'Mstrq4 (Open Source)'; \
                 Set-ItemProperty -Path $reg -Name 'DisplayIcon' -Value '{ico_str}'; \
                 Set-ItemProperty -Path $reg -Name 'InstallLocation' -Value '{dir_str}'; \
                 Set-ItemProperty -Path $reg -Name 'UninstallString' -Value 'cmd.exe /c \"{uninst_str}\"'; \
                 Set-ItemProperty -Path $reg -Name 'NoModify' -Value 1 -Type DWord; \
                 Set-ItemProperty -Path $reg -Name 'NoRepair' -Value 1 -Type DWord; "
            ));
        }
        let _ = run_hidden_powershell(&script);
    }
    emit(
        "shortcuts",
        "done",
        80,
        "[4/6] Shortcuts & Windows Uninstall registration complete.",
    );

    emit(
        "sync_apps",
        "running",
        88,
        "[5/6] Configuring startup preference & syncing AI developer tools...",
    );
    #[cfg(windows)]
    {
        let exe_quoted = format!("\"{}\" --minimized", ps_escape(&target_exe.to_string_lossy()));
        let run_script = if opts.auto_start {
            format!(
                "New-Item -Path 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run' -Force | Out-Null; \
                 Set-ItemProperty -Path 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run' -Name 'ClaudeSwitchPro' -Value '{exe_quoted}'"
            )
        } else {
            "Remove-ItemProperty -Path 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run' -Name 'ClaudeSwitchPro' -ErrorAction SilentlyContinue".to_string()
        };
        let _ = run_hidden_powershell(&run_script);
    }
    if opts.auto_sync {
        let sync_res = apply_all_detected(&cfg);
        emit(
            "sync_apps",
            "done",
            94,
            &format!("[5/6] Tool sync result: {}", sync_res),
        );
    } else {
        emit("sync_apps", "done", 94, "[5/6] Startup configured (tool sync skipped by user).");
    }

    emit("verify", "running", 97, "[6/6] Verifying installed binary...");
    if !target_exe.exists() {
        return Err(format!("Verification failed: {} does not exist", target_exe.display()));
    }
    emit(
        "verify",
        "done",
        100,
        &format!("[6/6] Verified {} successfully!", target_exe.display()),
    );

    Ok(target_exe)
}

fn launch_installed_app(exe_path: &Path) {
    if !exe_path.exists() {
        return;
    }
    let mut cmd = Command::new(exe_path);
    if let Some(dir) = exe_path.parent() {
        cmd.current_dir(dir);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    let _ = cmd.spawn();
}

fn spawn_install_worker(opts: InstallOptions, proxy: EventLoopProxy<WizardEvent>) {
    thread::spawn(move || {
        let res = perform_installation(&opts, |step_id, status, progress, log_line| {
            let _ = proxy.send_event(WizardEvent::StepProgress {
                step_id: step_id.to_string(),
                status: status.to_string(),
                progress,
                log_line: log_line.to_string(),
            });
        });
        match res {
            Ok(installed_exe) => {
                let _ = proxy.send_event(WizardEvent::InstallDone {
                    ok: true,
                    installed_path: installed_exe.to_string_lossy().to_string(),
                    error: String::new(),
                });
            }
            Err(err) => {
                let _ = proxy.send_event(WizardEvent::InstallDone {
                    ok: false,
                    installed_path: opts.install_dir.to_string_lossy().to_string(),
                    error: err,
                });
            }
        }
    });
}

fn main() {
    let silent = std::env::args().any(|arg| {
        let lower = arg.to_ascii_lowercase();
        lower == "--silent" || lower == "/s" || lower == "/silent"
    });

    if silent {
        let opts = InstallOptions {
            install_dir: default_install_dir(),
            desktop_shortcut: true,
            start_menu_shortcut: true,
            auto_start: true,
            auto_sync: true,
        };
        match perform_installation(&opts, |_step, _status, _pct, _log| {}) {
            Ok(exe) => {
                launch_installed_app(&exe);
                return;
            }
            Err(e) => {
                eprintln!("Silent installation failed: {e}");
                std::process::exit(1);
            }
        }
    }

    let event_loop = EventLoopBuilder::<WizardEvent>::with_user_event().build();
    let proxy_ipc = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("Claude Switch Pro — Setup Wizard")
        .with_inner_size(LogicalSize::new(900.0, 640.0))
        .with_min_inner_size(LogicalSize::new(780.0, 560.0))
        .build(&event_loop)
        .expect("Failed to create installer window");

    let webview_ref = std::rc::Rc::new(std::cell::RefCell::new(None::<wry::WebView>));
    let wv_ipc_handle = webview_ref.clone();

    let default_dir = default_install_dir();
    let existing_installed = default_dir
        .join(if cfg!(windows) {
            "claude-switch-pro.exe"
        } else {
            "claude-switch-pro"
        })
        .exists();
    let init_payload = json!({
        "version": env!("CARGO_PKG_VERSION"),
        "arch": current_arch_label(),
        "defaultPath": default_dir.to_string_lossy(),
        "existingInstalled": existing_installed,
    });

    let mut web_ctx = wry::WebContext::new(Some(
        std::env::temp_dir().join("ClaudeSwitchPro-Setup-WebView2"),
    ));
    let webview = WebViewBuilder::new_with_web_context(&mut web_ctx)
        .with_html(WIZARD_HTML)
        .with_ipc_handler(move |req| {
            let Ok(msg) = serde_json::from_str::<IpcMessage>(req.body()) else {
                return;
            };
            match msg.cmd.as_str() {
                "ready" => {
                    if let Some(wv) = wv_ipc_handle.borrow().as_ref() {
                        let js = format!("window.__INSTALLER_INIT__ && window.__INSTALLER_INIT__({init_payload});");
                        let _ = wv.evaluate_script(&js);
                    }
                }
                "browse_folder" => {
                    let cur = msg
                        .current_path
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or_else(|| default_dir.to_string_lossy().to_string());
                    let p = proxy_ipc.clone();
                    thread::spawn(move || {
                        if let Some(picked) = pick_folder_dialog(&cur) {
                            let _ = p.send_event(WizardEvent::PathPicked(picked));
                        }
                    });
                }
                "start_install" => {
                    let dir = msg
                        .install_dir
                        .filter(|s| !s.trim().is_empty())
                        .map(PathBuf::from)
                        .unwrap_or_else(default_install_dir);
                    let opts = InstallOptions {
                        install_dir: dir,
                        desktop_shortcut: msg.desktop_shortcut,
                        start_menu_shortcut: msg.start_menu_shortcut,
                        auto_start: msg.auto_start,
                        auto_sync: msg.auto_sync,
                    };
                    spawn_install_worker(opts, proxy_ipc.clone());
                }
                "launch_and_close" => {
                    let dir = msg
                        .install_dir
                        .filter(|s| !s.trim().is_empty())
                        .map(PathBuf::from)
                        .unwrap_or_else(default_install_dir);
                    let exe = dir.join(if cfg!(windows) {
                        "claude-switch-pro.exe"
                    } else {
                        "claude-switch-pro"
                    });
                    launch_installed_app(&exe);
                    let _ = proxy_ipc.send_event(WizardEvent::RequestClose);
                }
                "close" => {
                    let _ = proxy_ipc.send_event(WizardEvent::RequestClose);
                }
                _ => {}
            }
        })
        .build(&window)
        .expect("Failed to initialize Setup WebView2");

    *webview_ref.borrow_mut() = Some(webview);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(WizardEvent::RequestClose) => {
                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(WizardEvent::PathPicked(path)) => {
                if let Some(wv) = webview_ref.borrow().as_ref() {
                    let p_json = serde_json::to_string(&path).unwrap_or_default();
                    let _ = wv.evaluate_script(&format!(
                        "window.__INSTALLER_PATH__ && window.__INSTALLER_PATH__({p_json});"
                    ));
                }
            }
            Event::UserEvent(WizardEvent::StepProgress {
                step_id,
                status,
                progress,
                log_line,
            }) => {
                if let Some(wv) = webview_ref.borrow().as_ref() {
                    let payload = json!({
                        "stepId": step_id,
                        "status": status,
                        "progress": progress,
                        "logLine": log_line,
                    });
                    let _ = wv.evaluate_script(&format!(
                        "window.__INSTALLER_STEP__ && window.__INSTALLER_STEP__({payload});"
                    ));
                }
            }
            Event::UserEvent(WizardEvent::InstallDone {
                ok,
                installed_path,
                error,
            }) => {
                if let Some(wv) = webview_ref.borrow().as_ref() {
                    let payload = json!({
                        "ok": ok,
                        "installedPath": installed_path,
                        "error": error,
                    });
                    let _ = wv.evaluate_script(&format!(
                        "window.__INSTALLER_DONE__ && window.__INSTALLER_DONE__({payload});"
                    ));
                }
            }
            _ => {}
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ipc_start_install_payload_and_escapes_ps() {
        let raw = r#"{"cmd":"start_install","installDir":"C:\\Apps\\O'Brien","desktopShortcut":false,"startMenuShortcut":true,"autoStart":true,"autoSync":false}"#;
        let msg: IpcMessage = serde_json::from_str(raw).unwrap();
        assert_eq!(msg.cmd, "start_install");
        assert!(!msg.desktop_shortcut);
        assert!(msg.start_menu_shortcut);
        assert!(msg.auto_start);
        assert!(!msg.auto_sync);
        assert_eq!(ps_escape(msg.install_dir.as_deref().unwrap()), "C:\\Apps\\O''Brien");
        assert!(WIZARD_HTML.contains("__INSTALLER_STEP__"));
    }
}
