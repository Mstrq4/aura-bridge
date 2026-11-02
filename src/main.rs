#![windows_subsystem = "windows"]
#![allow(non_snake_case, dead_code, unused_imports)]

use std::ffi::OsStr;
use std::iter::once;
use std::net::TcpStream;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

type HWND = *mut std::ffi::c_void;
type HMENU = *mut std::ffi::c_void;
type HICON = *mut std::ffi::c_void;
type HINSTANCE = *mut std::ffi::c_void;
type HCURSOR = *mut std::ffi::c_void;
type HBRUSH = *mut std::ffi::c_void;
type LRESULT = isize;
type UINT = u32;
type WPARAM = usize;
type LPARAM = isize;
type BOOL = i32;
type LPCWSTR = *const u16;

#[repr(C)]
struct WNDCLASSEXW {
    cbSize: UINT,
    style: UINT,
    lpfnWndProc: Option<unsafe extern "system" fn(HWND, UINT, WPARAM, LPARAM) -> LRESULT>,
    cbClsExtra: i32,
    cbWndExtra: i32,
    hInstance: HINSTANCE,
    hIcon: HICON,
    hCursor: HCURSOR,
    hbrBackground: HBRUSH,
    lpszMenuName: LPCWSTR,
    lpszClassName: LPCWSTR,
    hIconSm: HICON,
}

#[repr(C)]
struct POINT {
    x: i32,
    y: i32,
}

#[repr(C)]
struct MSG {
    hwnd: HWND,
    message: UINT,
    wParam: WPARAM,
    lParam: LPARAM,
    time: u32,
    pt: POINT,
}

#[repr(C)]
struct GUID {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

#[repr(C)]
struct NOTIFYICONDATAW {
    cbSize: u32,
    hWnd: HWND,
    uID: u32,
    uFlags: u32,
    uCallbackMessage: u32,
    hIcon: HICON,
    szTip: [u16; 128],
    dwState: u32,
    dwStateMask: u32,
    szInfo: [u16; 256],
    uTimeoutOrVersion: u32,
    szInfoTitle: [u16; 64],
    dwInfoFlags: u32,
    guidItem: GUID,
    hBalloonIcon: HICON,
}

const WM_DESTROY: UINT = 0x0002;
const WM_USER: UINT = 0x0400;
const WM_TRAYICON: UINT = WM_USER + 101;

const WM_LBUTTONUP: LPARAM = 0x0202;
const WM_LBUTTONDBLCLK: LPARAM = 0x0203;
const WM_RBUTTONUP: LPARAM = 0x0205;

const NIM_ADD: u32 = 0x00000000;
const NIM_DELETE: u32 = 0x00000002;

const NIF_MESSAGE: u32 = 0x00000001;
const NIF_ICON: u32 = 0x00000002;
const NIF_TIP: u32 = 0x00000004;

const TPM_RIGHTBUTTON: UINT = 0x0002;
const TPM_RETURNCMD: UINT = 0x0100;
const MF_STRING: UINT = 0x0000;
const MF_SEPARATOR: UINT = 0x0800;

const IDI_APPLICATION: usize = 32512;

const CMD_OPEN_DASHBOARD: usize = 1001;
const CMD_RESTART_BRIDGE: usize = 1002;
const CMD_CHECK_STATUS: usize = 1003;
const CMD_EXIT: usize = 1009;

const BRIDGE_HOST: &str = "127.0.0.1";
const BRIDGE_PORT: u16 = 18787;
const BRIDGE_URL: &str = "http://127.0.0.1:18787/";

#[link(name = "user32")]
extern "system" {
    fn RegisterClassExW(lpwcx: *const WNDCLASSEXW) -> u16;
    fn CreateWindowExW(
        dwExStyle: u32,
        lpClassName: LPCWSTR,
        lpWindowName: LPCWSTR,
        dwStyle: u32,
        X: i32,
        Y: i32,
        nWidth: i32,
        nHeight: i32,
        hWndParent: HWND,
        hMenu: HMENU,
        hInstance: HINSTANCE,
        lpParam: *mut std::ffi::c_void,
    ) -> HWND;
    fn DefWindowProcW(hWnd: HWND, Msg: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    fn PostQuitMessage(nExitCode: i32);
    fn GetMessageW(lpMsg: *mut MSG, hWnd: HWND, wMsgFilterMin: UINT, wMsgFilterMax: UINT) -> BOOL;
    fn TranslateMessage(lpMsg: *const MSG) -> BOOL;
    fn DispatchMessageW(lpMsg: *const MSG) -> LRESULT;
    fn LoadIconW(hInstance: HINSTANCE, lpIconName: usize) -> HICON;
    fn CreatePopupMenu() -> HMENU;
    fn AppendMenuW(hMenu: HMENU, uFlags: UINT, uIDNewItem: usize, lpNewItem: LPCWSTR) -> BOOL;
    fn TrackPopupMenu(
        hMenu: HMENU,
        uFlags: UINT,
        x: i32,
        y: i32,
        nReserved: i32,
        hWnd: HWND,
        prcRect: *const std::ffi::c_void,
    ) -> BOOL;
    fn DestroyMenu(hMenu: HMENU) -> BOOL;
    fn GetCursorPos(lpPoint: *mut POINT) -> BOOL;
    fn SetForegroundWindow(hWnd: HWND) -> BOOL;
    fn GetModuleHandleW(lpModuleName: LPCWSTR) -> HINSTANCE;
}

#[link(name = "shell32")]
extern "system" {
    fn Shell_NotifyIconW(dwMessage: u32, lpData: *mut NOTIFYICONDATAW) -> BOOL;
}

static IS_RUNNING: AtomicBool = AtomicBool::new(true);

fn to_wide_chars(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(once(0)).collect()
}

fn is_bridge_online() -> bool {
    TcpStream::connect((BRIDGE_HOST, BRIDGE_PORT)).is_ok()
}

fn locate_bridge_root() -> Option<PathBuf> {
    let candidate = PathBuf::from(r"H:\Projects\Backend-API\Active\claude-responses-bridge");
    if candidate.join("src").join("server.mjs").exists() {
        return Some(candidate);
    }
    None
}

fn ensure_bridge_running() {
    if is_bridge_online() {
        return;
    }

    if let Some(bridge_root) = locate_bridge_root() {
        let server_script = bridge_root.join("src").join("server.mjs");
        let mut cmd = Command::new("node");
        cmd.arg(&server_script)
            .current_dir(&bridge_root)
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);

        if let Ok(_) = cmd.spawn() {
            for _ in 0..25 {
                thread::sleep(Duration::from_millis(200));
                if is_bridge_online() {
                    return;
                }
            }
        }
    }
}

fn locate_edge() -> Option<PathBuf> {
    let candidates = [
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    ];
    for p in &candidates {
        let path = Path::new(p);
        if path.exists() {
            return Some(path.to_path_buf());
        }
    }
    None
}

fn launch_dashboard_window() {
    ensure_bridge_running();

    let runtime = locate_edge();
    let app_dir = std::env::var("LOCALAPPDATA")
        .map(|p| PathBuf::from(p).join("ClaudeSwitchPro"))
        .unwrap_or_else(|_| PathBuf::from(r"C:\Users\samsung\AppData\Local\ClaudeSwitchPro"));

    let profile_dir = app_dir.join("profile");
    let _ = std::fs::create_dir_all(&profile_dir);

    if let Some(edge_path) = runtime {
        let mut app_cmd = Command::new(edge_path);
        app_cmd
            .arg(format!("--app={}", BRIDGE_URL))
            .arg("--window-size=1280,840")
            .arg("--window-position=140,90")
            .arg(format!("--user-data-dir={}", profile_dir.display()))
            .arg("--disable-features=TranslateUI")
            .arg("--no-first-run")
            .arg("--no-default-browser-check");

        let _ = app_cmd.spawn();
    } else {
        let _ = Command::new("cmd")
            .args(&["/c", "start", BRIDGE_URL])
            .spawn();
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAYICON => {
            if lparam == WM_LBUTTONUP || lparam == WM_LBUTTONDBLCLK {
                launch_dashboard_window();
            } else if lparam == WM_RBUTTONUP {
                let hmenu = CreatePopupMenu();
                if !hmenu.is_null() {
                    let title_wide = to_wide_chars("لوحة التحكم (Claude Switch Pro)");
                    let restart_wide = to_wide_chars("إعادة تشغيل الجسر");
                    let status_wide = to_wide_chars("فحص حالة الربط (18787)");
                    let exit_wide = to_wide_chars("خروج نهائي (Exit)");

                    AppendMenuW(hmenu, MF_STRING, CMD_OPEN_DASHBOARD, title_wide.as_ptr());
                    AppendMenuW(hmenu, MF_STRING, CMD_RESTART_BRIDGE, restart_wide.as_ptr());
                    AppendMenuW(hmenu, MF_STRING, CMD_CHECK_STATUS, status_wide.as_ptr());
                    AppendMenuW(hmenu, 0x0800 /* MF_SEPARATOR */, 0, null());
                    AppendMenuW(hmenu, MF_STRING, CMD_EXIT, exit_wide.as_ptr());

                    let mut pt = POINT { x: 0, y: 0 };
                    GetCursorPos(&mut pt);
                    SetForegroundWindow(hwnd);

                    let cmd = TrackPopupMenu(
                        hmenu,
                        TPM_RIGHTBUTTON | TPM_RETURNCMD,
                        pt.x,
                        pt.y,
                        0,
                        hwnd,
                        null(),
                    );
                    DestroyMenu(hmenu);

                    match cmd as usize {
                        CMD_OPEN_DASHBOARD => {
                            launch_dashboard_window();
                        }
                        CMD_RESTART_BRIDGE => {
                            // Kill existing bridge process if needed and respawn
                            ensure_bridge_running();
                            launch_dashboard_window();
                        }
                        CMD_CHECK_STATUS => {
                            launch_dashboard_window();
                        }
                        CMD_EXIT => {
                            IS_RUNNING.store(false, Ordering::SeqCst);
                            PostQuitMessage(0);
                        }
                        _ => {}
                    }
                }
            }
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn main() {
    ensure_bridge_running();

    unsafe {
        let class_name = to_wide_chars("ClaudeSwitchProTrayClass");
        let hinstance = GetModuleHandleW(null());
        let hicon = LoadIconW(null_mut(), IDI_APPLICATION);

        let wnd_class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as UINT,
            style: 0,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: hicon,
            hCursor: null_mut(),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: class_name.as_ptr(),
            hIconSm: hicon,
        };

        RegisterClassExW(&wnd_class);

        let window_title = to_wide_chars("Claude Switch Pro Hidden Window");
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            window_title.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            hinstance,
            null_mut(),
        );

        if hwnd.is_null() {
            return;
        }

        let mut nid = std::mem::zeroed::<NOTIFYICONDATAW>();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        nid.uCallbackMessage = WM_TRAYICON;
        nid.hIcon = hicon;

        let tip = "Claude Responses Switch Pro — متصل على 18787";
        let tip_wide = to_wide_chars(tip);
        let len = tip_wide.len().min(nid.szTip.len() - 1);
        nid.szTip[..len].copy_from_slice(&tip_wide[..len]);

        Shell_NotifyIconW(NIM_ADD, &mut nid);

        // Auto launch dashboard window on initial startup
        launch_dashboard_window();

        let mut msg = std::mem::zeroed::<MSG>();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        Shell_NotifyIconW(NIM_DELETE, &mut nid);
    }
}
