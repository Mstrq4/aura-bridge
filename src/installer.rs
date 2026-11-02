#![windows_subsystem = "windows"]
#![allow(non_snake_case, dead_code, unused_imports)]

use std::ffi::OsStr;
use std::fs;
use std::iter::once;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::ptr::null_mut;

const APP_PAYLOAD: &[u8] = include_bytes!("../claude-switch-pro.exe");

type HWND = *mut std::ffi::c_void;
type LPCWSTR = *const u16;
type UINT = u32;

const MB_OK: UINT = 0x00000000;
const MB_ICONINFORMATION: UINT = 0x00000040;

#[link(name = "user32")]
extern "system" {
    fn MessageBoxW(hWnd: HWND, lpText: LPCWSTR, lpCaption: LPCWSTR, uType: UINT) -> i32;
}

fn to_wide_chars(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(once(0)).collect()
}

fn main() {
    let local_app_data = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(r"C:\Users\samsung\AppData\Local"));

    let target_dir = local_app_data.join("Programs").join("ClaudeSwitchPro");
    let _ = fs::create_dir_all(&target_dir);

    let target_exe = target_dir.join("claude-switch-pro.exe");
    if let Err(e) = fs::write(&target_exe, APP_PAYLOAD) {
        let msg = format!("فشل في نسخ ملف التطبيق: {}\nالمسار: {}", e, target_exe.display());
        let msg_w = to_wide_chars(&msg);
        let cap_w = to_wide_chars("خطأ في التثبيت");
        unsafe {
            MessageBoxW(null_mut(), msg_w.as_ptr(), cap_w.as_ptr(), 0x10 /* MB_ICONERROR */);
        }
        return;
    }

    // Create Desktop Shortcut and Start Menu shortcut via PowerShell
    let target_str = target_exe.to_string_lossy();
    let ps_script = format!(
        "$ws = New-Object -ComObject WScript.Shell; \
         $d = [Environment]::GetFolderPath('Desktop'); \
         $s1 = $ws.CreateShortcut($d + '\\Claude Switch Pro.lnk'); \
         $s1.TargetPath = '{0}'; \
         $s1.Description = 'Claude Switch Pro — Resilient Bridge & Rate-Limit Shield'; \
         $s1.WorkingDirectory = '{1}'; \
         $s1.Save(); \
         $sm = [Environment]::GetFolderPath('Programs'); \
         $s2 = $ws.CreateShortcut($sm + '\\Claude Switch Pro.lnk'); \
         $s2.TargetPath = '{0}'; \
         $s2.Description = 'Claude Switch Pro'; \
         $s2.WorkingDirectory = '{1}'; \
         $s2.Save();",
        target_str.replace("'", "''"),
        target_dir.to_string_lossy().replace("'", "''")
    );

    let _ = Command::new("powershell")
        .args(&["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps_script])
        .output();

    // Spawn the installed app
    let _ = Command::new(&target_exe).spawn();

    // Success dialog
    let success_text = "تم تثبيت Claude Switch Pro بنجاح!\n\n• تم إنشاء اختصار على سطح المكتب وقائمة ابدأ.\n• التطبيق يعمل الآن في الخلفية في صينية النظام بجانب الساعة.\n• عند إغلاق الواجهة سيبقى التطبيق يعمل في شريط المهام كبديل كامل لـ CC Switch.";
    let success_w = to_wide_chars(success_text);
    let title_w = to_wide_chars("Claude Switch Pro — اكتمل التثبيت");

    unsafe {
        MessageBoxW(null_mut(), success_w.as_ptr(), title_w.as_ptr(), MB_OK | MB_ICONINFORMATION);
    }
}
