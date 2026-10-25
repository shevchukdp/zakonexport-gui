//! zakonexport-gui — double-clickable GUI executable of the Zakon DB
//! exporter. Release builds use the windows subsystem, so starting it does
//! NOT open a console window; the GUI window appears immediately.
//! The engine lives in the lib target (`zakon_exporter`).
//!
//! Args (optional): --base general|audit|<path> | --baza <path> | --guitest
//! Startup errors (bad base path, engine failure) are shown in a native
//! message box, which also works without a console.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// Startup errors must be visible even without a console.
fn fatal_box(msg: &str) {
    use winapi::um::winuser::{MessageBoxW, MB_ICONERROR, MB_SETFOREGROUND, MB_TOPMOST};
    unsafe {
        let t = to_wide("zakonexport-gui — помилка");
        let m = to_wide(msg);
        MessageBoxW(
            std::ptr::null_mut(),
            m.as_ptr(),
            t.as_ptr(),
            MB_ICONERROR | MB_TOPMOST | MB_SETFOREGROUND,
        );
    }
}

fn main() {
    let mut baza = PathBuf::from(zakon_exporter::DEFAULT_BAZA);
    let mut guitest = false;
    // true when --baza/--base explicitly set the base (run_gui then never
    // consults the remembered config and never asks interactively)
    let mut explicit = false;
    let usage = "Використання: zakonexport-gui [--base general|audit|<path>] [--baza <шлях>] [--guitest]";
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if (a == "--baza" || a == "--base") && i + 1 >= args.len() {
            fatal_box(&format!("{a} потребує значення шляху.\n\n{usage}"));
            return;
        }
        let next = args.get(i + 1).cloned();
        let mut consumed = 1;
        match a.as_str() {
            "--baza" => {
                baza = PathBuf::from(next.unwrap());
                explicit = true;
                consumed = 2;
            }
            "--base" => {
                let sel = next.unwrap().to_lowercase();
                baza = match sel.as_str() {
                    "general" | "основна" | "1" => PathBuf::from(zakon_exporter::DEFAULT_BAZA),
                    "audit" | "гк" | "gk" | "2" => PathBuf::from(zakon_exporter::BASE_AUDIT),
                    other => PathBuf::from(other),
                };
                explicit = true;
                consumed = 2;
            }
            "--guitest" => guitest = true,
            other => {
                fatal_box(&format!(
                    "невідомий аргумент: {other}\n\n{usage}"
                ));
                return;
            }
        }
        i += consumed;
    }
    // A missing base dir is handled INSIDE run_gui: it falls back to the
    // remembered config (%APPDATA%\zakonexport\zakonexport.toml) and, as a
    // last resort, asks the user to pick the folder — no fatal box here.
    if let Err(e) = zakon_exporter::gui::run_gui(baza, guitest, explicit) {
        fatal_box(&format!("Помилка запуску: {e}"));
        std::process::exit(1);
    }
}
