//! Сигнальні документи (додатки / Word-вложения).
//!
//! The old program (zak_work.exe) resolves a document's signal attachment as
//! `<base>\signal\<docnum>.<ext>` and probes a fixed list of extensions in
//! order (decomp FUN_00655e60). Zip/Rar entries are single-file Word/RTF
//! wrappers. This module is the clean probe: no registry logic replicated.
//!
//! Both configured bases are meant to carry signal docs (main base +
//! State-Audit-Service/GK). On a given install a base's `signal` folder may be
//! empty (then no attachments for that base) — the probe simply returns None.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Probe order from the decompiled extension try-table (13 exts, +xlsx).
pub const SIGNAL_EXTS: &[&str] = &[
    "hlp", "doc", "rtf", "txt", "asc", "xls", "jpg", "gif", "pdf", "htm", "zip", "rar", "docx",
    "xlsx",
];

/// Signal sub-folder under the base dir. Case-insensitive on Windows: the data
/// uses `Signal` (main) / `SIGNAL` (GK); the old program probes `signal`.
pub const SIGNAL_DIR: &str = "signal";

/// Probe `<baza>\signal\<docnum>.<ext>` in SIGNAL_EXTS order; the first
/// existing file wins. `None` when the base has no attachment for this doc.
pub fn find_signal(baza: &Path, docnum: u32) -> Option<PathBuf> {
    let dir = baza.join(SIGNAL_DIR);
    for ext in SIGNAL_EXTS {
        let p = dir.join(format!("{docnum}.{ext}"));
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// True when the base has a (non-empty) signal store.
pub fn signal_dir_has_files(baza: &Path) -> bool {
    let dir = baza.join(SIGNAL_DIR);
    match std::fs::read_dir(&dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .any(|e| e.path().is_file()),
        Err(_) => false,
    }
}

/// Copy the signal attachment for `docnum` into `dest_dir`, named
/// `<stem>.<signal-ext>` so it sits beside the exported document (same stem,
/// the attachment keeps its own extension). Returns the destination path, or
/// `None` when the document has no signal attachment.
pub fn copy_signal(baza: &Path, docnum: u32, dest_dir: &Path, stem: &str) -> Result<Option<PathBuf>> {
    let src = match find_signal(baza, docnum) {
        Some(p) => p,
        None => return Ok(None),
    };
    let ext = src
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_else(|| "bin".to_string());
    let dest = dest_dir.join(format!("{stem}.{ext}"));
    std::fs::copy(&src, &dest)
        .with_context(|| format!("copy signal {} -> {}", src.display(), dest.display()))?;
    Ok(Some(dest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_finds_first_existing_ext() {
        // build a temp base with a signal dir
        let base = PathBuf::from(r"C:\temp\opencode\signal_probe_base");
        let sig = base.join(SIGNAL_DIR);
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&sig).unwrap();
        std::fs::write(sig.join("4242.doc"), b"WORD").unwrap();
        std::fs::write(sig.join("4243.zip"), b"PK").unwrap();
        // 4242 has .doc (probe order hits .doc before later exts)
        let p = find_signal(&base, 4242).unwrap();
        assert!(p.file_name().unwrap().to_string_lossy() == "4242.doc");
        // 4243 only has .zip
        let z = find_signal(&base, 4243).unwrap();
        assert!(z.file_name().unwrap().to_string_lossy() == "4243.zip");
        // 4244 absent
        assert!(find_signal(&base, 4244).is_none());
        // store is non-empty
        assert!(signal_dir_has_files(&base));
        // copy keeps stem + original ext
        let dest = PathBuf::from(r"C:\temp\opencode\signal_probe_out");
        let _ = std::fs::remove_dir_all(&dest);
        std::fs::create_dir_all(&dest).unwrap();
        let c = copy_signal(&base, 4243, &dest, "00043_9").unwrap().unwrap();
        assert!(c.file_name().unwrap().to_string_lossy() == "00043_9.zip");
        assert!(c.is_file());
        assert!(copy_signal(&base, 4244, &dest, "x").unwrap().is_none());
        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::remove_dir_all(&dest);
    }

    #[test]
    fn ext_order_is_stable() {
        // .doc must be probed before .docx etc. (matches decomp order)
        assert_eq!(SIGNAL_EXTS[0], "hlp");
        assert_eq!(SIGNAL_EXTS[1], "doc");
        assert!(SIGNAL_EXTS.contains(&"zip"));
        assert!(SIGNAL_EXTS.contains(&"docx"));
    }
}
