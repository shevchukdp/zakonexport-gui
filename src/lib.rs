//! Zakon DB Exporter — engine + frontends (lib target).
//!
//! The code is split so two separate executables build from the same engine:
//! `zakon-cli` — the console tool (search / extract / export / bulk / stats,
//! plus the `gui` fallback), and `zakon-gui` — a double-clickable exe that
//! opens the GUI directly (release builds use the windows subsystem, so no
//! console window flashes).

pub mod attach;
pub mod cli;
pub mod db;
pub mod mdconv;
pub mod fields;
pub mod ftsearch;
pub mod gui;
pub mod highlight;
pub mod naming;
pub mod pdf;
pub mod picker;
pub mod printdoc;
pub mod render;
pub mod signal;
pub mod trident;
pub mod update;
pub mod writers;

pub const DEFAULT_BAZA: &str = r"C:\Zakon.Local\Baza";

/// Доп. база Держаудитслужби (ГоловКРУ — органи державного фінансового контролю).
pub const BASE_AUDIT: &str = r"C:\Zakon.Local\Baza.GK";

/// Single source for the app version (bumped together with Cargo.toml).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Program name shown in titles / about dialogs.
pub const APP_NAME: &str = "Zakon DB Exporter";
/// Copy shown in the about dialogs and `about`/`--version`.
pub const COPYRIGHT: &str = "2026 Дмитро Шевчук";

/// Full about text (console + MessageBox in the GUI).
pub fn about_text() -> String {
    format!(
        "{APP_NAME} {VERSION}\n\
         Пошук, фільтрування та експорт баз «Закон».\n\
         Дві бази: загальне законодавство і Держаудитслужба / ГоловКРУ.\n\
         Пошук розгортає стиснутий потоковий формат документів (кирилиця без втрат).\n\
         Додатки (сигнальні документи / Word-вложения): пошук у <база>\\signal\\ і\n\
         копіювання разом з експортом (md/rtf/docx/html/csv/pdf).\n\
         Формати зберігають оформлення: заголовки, центрований запис, списки.\n\
         Зв'язка (--single / «зв'язка (1 файл)»): всі документи в ОДИН файл\n\
         (md/rtf/docx/html/pdf; PDF зі змістом /Outlines, кожен документ з нової сторінки).\n\
         PDF — вбудований сабсет авто-вибраного системного шрифту (кирилиця:\n\
         arial → times/cambria/georgia/tahoma/verdana; без зовнішніх засобів).\n\
         Друк — GDI (системний діалог друку): жирним рядки ВЕЛИКОЮ, списки «•  ».\n\n\
         © {COPYRIGHT}"
    )
}

// ---------------------------------------------------------------------------
// base-directory persistence (moved installs, e.g. another user's C:\Zakon.2\)
// ---------------------------------------------------------------------------

use std::path::{Path, PathBuf};

/// Path of the user config that remembers the last used base directory
/// (`%APPDATA%\zakonexport\zakonexport.toml`). None when APPDATA is unset.
pub fn config_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .map(|d| PathBuf::from(d).join("zakonexport").join("zakonexport.toml"))
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
struct SavedConfig {
    #[serde(default)]
    baza: String,
}

/// The remembered base directory; returned only when the directory still
/// exists (stale entries are ignored).
pub fn load_saved_baza() -> Option<PathBuf> {
    let path = config_path()?;
    let text = std::fs::read_to_string(&path).ok()?;
    let cfg: SavedConfig = toml::from_str(&text).ok()?;
    let b = PathBuf::from(cfg.baza);
    if b.is_dir() {
        Some(b)
    } else {
        None
    }
}

/// Remember the base directory for future runs.
pub fn save_saved_baza(p: &Path) -> std::io::Result<()> {
    let file = config_path().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "APPDATA is not set")
    })?;
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = toml::to_string(&SavedConfig {
        baza: p.display().to_string(),
    })
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    std::fs::write(&file, text)
}

/// The user may pick a folder that CONTAINS the base (`C:\Zakon.2\`) instead
/// of the base folder itself: prefer the picked folder, then its `Baza`
/// subfolder, then the first child folder that looks like a base (has a
/// `text` subfolder).
pub fn auto_base_under(p: &Path) -> Option<PathBuf> {
    if p.join("text").is_dir() {
        return Some(p.to_path_buf());
    }
    let b = p.join("Baza");
    if b.join("text").is_dir() {
        return Some(b);
    }
    let mut kids: Vec<PathBuf> = match std::fs::read_dir(p) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|k| k.is_dir())
            .collect(),
        Err(_) => return None,
    };
    kids.sort();
    kids.into_iter().find(|k| k.join("text").is_dir())
}
