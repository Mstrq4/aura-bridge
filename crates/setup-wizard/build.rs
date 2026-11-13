use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn find_rc_compiler() -> Option<(PathBuf, &'static str)> {
    let llvm_candidates = [
        r"C:\Program Files\LLVM\bin\llvm-rc.exe",
        r"C:\Program Files (x86)\LLVM\bin\llvm-rc.exe",
    ];
    for c in llvm_candidates {
        let p = Path::new(c);
        if p.exists() {
            return Some((p.to_path_buf(), "llvm-rc"));
        }
    }
    if let Ok(out) = Command::new("where").arg("llvm-rc.exe").output() {
        if out.status.success() {
            if let Some(first) = String::from_utf8_lossy(&out.stdout).lines().next() {
                let p = PathBuf::from(first.trim());
                if p.exists() {
                    return Some((p, "llvm-rc"));
                }
            }
        }
    }
    if let Ok(out) = Command::new("where").arg("rc.exe").output() {
        if out.status.success() {
            if let Some(first) = String::from_utf8_lossy(&out.stdout).lines().next() {
                let p = PathBuf::from(first.trim());
                if p.exists() {
                    return Some((p, "rc"));
                }
            }
        }
    }
    let windres_candidates = [r"E:\Strawberry\c\bin\windres.exe"];
    for c in windres_candidates {
        let p = Path::new(c);
        if p.exists() {
            return Some((p.to_path_buf(), "windres"));
        }
    }
    if let Ok(out) = Command::new("where").arg("windres.exe").output() {
        if out.status.success() {
            if let Some(first) = String::from_utf8_lossy(&out.stdout).lines().next() {
                let p = PathBuf::from(first.trim());
                if p.exists() {
                    return Some((p, "windres"));
                }
            }
        }
    }
    None
}

fn locate_payload(manifest_dir: &Path) -> Option<PathBuf> {
    if let Ok(explicit) = env::var("CSP_PAYLOAD_PATH") {
        let p = PathBuf::from(explicit);
        if p.exists() && fs::metadata(&p).map(|m| m.len() > 1024).unwrap_or(false) {
            return Some(p);
        }
    }

    let workspace_root = manifest_dir.parent()?.parent()?;
    let bin_name = if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        "claude-switch-pro.exe"
    } else {
        "claude-switch-pro"
    };

    let mut candidates = vec![
        workspace_root.join(bin_name),
        workspace_root.join("target").join("release").join(bin_name),
    ];

    if let Ok(target_dir) = env::var("CARGO_TARGET_DIR") {
        let td = PathBuf::from(target_dir);
        if let Ok(triple) = env::var("TARGET") {
            candidates.push(td.join(&triple).join("release").join(bin_name));
        }
        candidates.push(td.join("release").join(bin_name));
    }
    if let Ok(triple) = env::var("TARGET") {
        candidates.push(
            workspace_root
                .join("target")
                .join(triple)
                .join("release")
                .join(bin_name),
        );
    }

    candidates
        .into_iter()
        .find(|p| p.exists() && fs::metadata(p).map(|m| m.len() > 1024).unwrap_or(false))
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    println!("cargo:rerun-if-changed=setup-wizard.rc");
    println!("cargo:rerun-if-changed=../../assets/setup.ico");
    println!("cargo:rerun-if-env-changed=CSP_PAYLOAD_PATH");

    let payload_path = match locate_payload(&manifest_dir) {
        Some(p) => {
            println!("cargo:rerun-if-changed={}", p.display());
            p
        }
        None => {
            let dummy = out_dir.join("empty_payload.bin");
            let _ = fs::write(&dummy, b"");
            println!(
                "cargo:warning=claude-switch-pro payload binary not found; setup wizard built with stub payload (build claude-switch-pro first for redistributable setup exe)"
            );
            dummy
        }
    };
    println!(
        "cargo:rustc-env=CSP_PAYLOAD_FILE={}",
        payload_path.display()
    );

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let rc_path = manifest_dir.join("setup-wizard.rc");
    let res_path = out_dir.join("setup-wizard.res");
    if let Some((compiler, kind)) = find_rc_compiler() {
        let ok = match kind {
            "llvm-rc" => Command::new(&compiler)
                .current_dir(&manifest_dir)
                .args(["/fo"])
                .arg(&res_path)
                .arg(&rc_path)
                .status()
                .map(|s| s.success())
                .unwrap_or(false),
            "rc" => Command::new(&compiler)
                .current_dir(&manifest_dir)
                .arg("/nologo")
                .arg(format!("/fo{}", res_path.display()))
                .arg(&rc_path)
                .status()
                .map(|s| s.success())
                .unwrap_or(false),
            _ => Command::new(&compiler)
                .current_dir(&manifest_dir)
                .args(["-O", "coff", "-i"])
                .arg(&rc_path)
                .arg("-o")
                .arg(&res_path)
                .status()
                .map(|s| s.success())
                .unwrap_or(false),
        };
        if ok && res_path.exists() {
            println!(
                "cargo:rustc-link-arg-bin=ClaudeSwitchPro-Setup={}",
                res_path.display()
            );
        }
    }
}

