//! Bake the release date into the binary, single-sourced from `Cargo.toml`'s
//! `[package.metadata] date` (mirrors how `CARGO_PKG_VERSION` is surfaced via
//! `env!` in `src/lib.rs`). The date is re-read whenever `Cargo.toml` changes,
//! so a release-date bump triggers a rebuild. Dependency-free: the
//! `[package.metadata]` section is parsed by hand (no build-dependency), so the
//! build stays a pure compile (no C toolchain, no extra crates).
fn main() {
    let release_date = std::env::var("ZAKON_RELEASE_DATE_OVERRIDE")
        .ok()
        .or_else(|| metadata_date(&std::path::Path::new(env!("CARGO_MANIFEST_DIR"))))
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=ZAKON_RELEASE_DATE={}", release_date);
    // re-run this build script (and rebuild) when the manifest date changes
    println!("cargo:rerun-if-changed=Cargo.toml");
}

/// Read `[package.metadata] date = "YYYY-MM-DD"` from `Cargo.toml`.
///
/// A minimal section-aware line scan (the value is a plain string we control),
/// so the build script needs no `toml` build-dependency. Returns `None` when
/// the section or key is absent.
fn metadata_date(manifest_dir: &std::path::Path) -> Option<String> {
    let toml = std::fs::read_to_string(manifest_dir.join("Cargo.toml")).ok()?;
    let mut in_meta = false;
    for line in toml.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            // a new table header ends the `[package.metadata]` section
            in_meta = t == "[package.metadata]";
            continue;
        }
        if in_meta && t.starts_with("date") {
            // `date = "2026-10-28"` (allow surrounding whitespace / comments)
            let rest = t.strip_prefix("date").unwrap_or("");
            let rest = rest.trim_start();
            let rest = rest.strip_prefix('=').unwrap_or(rest).trim();
            let val = rest.trim_matches('"').trim();
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}
