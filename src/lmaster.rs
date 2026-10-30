//! v0.41.0 (roadmap v0.42.0): the Rust `LMaster_*` driver — the FFI facade
//! over `lmsup32.dll` (a 32-bit PE32 engine, machine 0x14C I386) through the
//! 32-bit helper `ffi32/lmsup32-ffi32.exe`.
//!
//! `lmsup32.dll` imports only KERNEL32 / USER32 / ADVAPI32 (self-contained;
//! it does NOT depend on the 16-bit `lmsup.dll`), so a 32-bit process can
//! `LoadLibraryW` it. A 64-bit process cannot (ERROR_BAD_FORMAT / err 126),
//! so the i686 helper (committed as a release asset) is the FFI facade: it
//! loads the DLL, resolves the four `LMaster_*` exports
//! (`LoadLibraryW` + `GetProcAddress`), and drives them over a line-based
//! stdin/stdout protocol that this module speaks.
//!
//! Shipped scope (honest, v0.41.0):
//! - the FFI facade: `EXPORTS` resolves 4/4 and `LMaster_Lang` is drivable
//!   (e.g. `"ne"` -> 69);
//! - `LMaster_Begin` / `LMaster_Tran` (the DLL join) are blocked on the
//!   engine's non-exported Lm database-path global (`DAT_1002a30c` = `"lm.a"`,
//!   set only by the legacy app's non-exported init) -> `LmOpen Error`. That
//!   is documented, not a defect.
//! - the Rust-side corpus cross-check ([`corpus_crosscheck`], the exporter's
//!   own [`crate::trident::TridentIndex`] reader) is the ground-truth join
//!   for this version.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// The four LMaster_* exports (the C ABI of lmsup32.dll). The signatures are
// the decompiled shapes (lmsup32_all.c): Begin returns the handle-block
// pointer (whose first word is the engine handle); End frees it; Tran takes
// a pointer to the handle value + writes the result into outbuf and returns
// 0/1; Lang classifies a string into a packed u32. They are documented here
// as type aliases so the facade's shape is self-describing; the calls
// themselves live in the i686 helper (a 64-bit process cannot call them).
// ---------------------------------------------------------------------------
pub type LMasterBegin =
    unsafe extern "system" fn(dir: *mut u8, tranopen: *mut u8) -> *mut std::ffi::c_void;
pub type LMasterEnd = unsafe extern "system" fn(handle: *mut std::ffi::c_void) -> i32;
pub type LMasterTran =
    unsafe extern "system" fn(handle: *mut i32, out: *mut u8, size: i32, mode: i32) -> i32;
pub type LMasterLang = unsafe extern "system" fn(s: *mut u8) -> u32;

/// One parsed line of the helper's stdout protocol (pure, testable).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LmLine {
    /// `EXPORTS OK 4/4` / `EXPORTS FAIL <reason>`
    Exports {
        ok: bool,
        resolved: u32,
        total: u32,
        reason: Option<String>,
    },
    /// `BEGIN OK handle=<hex>` / `BEGIN FAIL <reason>`
    Begin {
        ok: bool,
        handle: Option<String>,
        reason: Option<String>,
    },
    /// `LANG <u32>`
    Lang(u32),
    /// `TRAN <ret> <outlen>`
    Tran { ret: i32, outlen: i32 },
    /// `TRAN FAIL <reason>` (emitted when the engine was never begun)
    TranFail(String),
    /// `HEX <space-hex bytes>`
    Hex(String),
    /// `END <ret>` (the helper appends ` (never begun)` when it never began)
    End(i32),
    /// `DIAG <file> <bool> (cwd=...)`
    Diag(String),
    /// `BYE`
    Bye,
    /// an unrecognized line (kept verbatim for the CLI to show)
    Raw(String),
}

/// Parse one protocol line (the helper emits exactly one line per command).
/// Pure and total: an unrecognized line is [`LmLine::Raw`], never a panic.
pub fn parse_lm_line(line: &str) -> LmLine {
    let t = line.trim();
    if t.is_empty() {
        return LmLine::Raw(String::new());
    }
    let mut parts = t.splitn(2, ' ');
    let head = parts.next().unwrap_or("");
    let rest = parts.next().unwrap_or("").trim();
    match head {
        "EXPORTS" => {
            // `EXPORTS OK 4/4` or `EXPORTS FAIL <reason>`
            let mut p = rest.splitn(2, ' ');
            let kind = p.next().unwrap_or("");
            let tail = p.next().unwrap_or("").trim().to_string();
            if kind == "OK" {
                // tail looks like `4/4`
                let (resolved, total) = tail.split_once('/').map_or((0, 4), |pair| {
                    (
                        pair.0.trim().parse().unwrap_or(0),
                        pair.1.trim().parse().unwrap_or(4),
                    )
                });
                LmLine::Exports {
                    ok: true,
                    resolved,
                    total,
                    reason: None,
                }
            } else {
                LmLine::Exports {
                    ok: false,
                    resolved: 0,
                    total: 4,
                    reason: Some(tail),
                }
            }
        }
        "BEGIN" => {
            let mut p = rest.splitn(2, ' ');
            let kind = p.next().unwrap_or("");
            let tail = p.next().unwrap_or("").trim().to_string();
            if kind == "OK" {
                // tail looks like `handle=1234`
                let handle = tail
                    .strip_prefix("handle=")
                    .map(|s| s.to_string())
                    .filter(|s| !s.is_empty());
                LmLine::Begin {
                    ok: true,
                    handle,
                    reason: None,
                }
            } else {
                LmLine::Begin {
                    ok: false,
                    handle: None,
                    reason: Some(tail),
                }
            }
        }
        "LANG" => LmLine::Lang(rest.parse().unwrap_or(0)),
        "TRAN" => {
            // `TRAN FAIL <reason>` (never begun) or `TRAN <ret> <outlen>`
            let mut p = rest.splitn(2, ' ');
            let first = p.next().unwrap_or("");
            if first == "FAIL" {
                LmLine::TranFail(p.next().unwrap_or("").to_string())
            } else {
                let ret = first.parse().unwrap_or(0);
                let outlen = p.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                LmLine::Tran { ret, outlen }
            }
        }
        "HEX" => LmLine::Hex(rest.to_string()),
        "END" => LmLine::End(rest.split_whitespace().next().and_then(|s| s.parse().ok()).unwrap_or(0)),
        "DIAG" => LmLine::Diag(t.to_string()),
        "BYE" => LmLine::Bye,
        _ => LmLine::Raw(t.to_string()),
    }
}

/// A structured report of one helper run (the FFI facade results).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LmasterReport {
    /// the helper exe that produced this report
    pub helper: PathBuf,
    /// the DLL directory spoken as the first protocol line
    pub dll_dir: String,
    /// the `EXPORTS` outcome (`true` = 4/4 resolved)
    pub exports_ok: Option<bool>,
    /// the number of `LMaster_*` exports the helper resolved (0..=4)
    pub exports_resolved: u32,
    /// the `EXPORTS` failure reason (e.g. `"LoadLibraryW failed (err 126)"`)
    pub exports_reason: Option<String>,
    /// the `BEGIN` outcome (`Some(true)` = the engine handle was returned)
    pub begin_ok: Option<bool>,
    /// the `BEGIN` failure reason (e.g. `"LMaster_Begin returned null (engine init failed)"`)
    pub begin_reason: Option<String>,
    /// every `LANG <u32>` value observed (the Lang classifier is drivable)
    pub langs: Vec<u32>,
    /// every `TRAN <ret> ...` return code (empty when Begin failed)
    pub tran_rets: Vec<i32>,
    /// every `TRAN FAIL <reason>` reason (e.g. `["not-begun"]` when Begin failed)
    pub tran_fails: Vec<String>,
    /// the `END <ret>` code (the `Some(0)` the helper reports when it never began)
    pub end: Option<i32>,
    /// the raw response lines, verbatim (for the CLI to print)
    pub lines: Vec<String>,
}

/// Fold the raw helper stdout into a [`LmasterReport`] (pure).
pub fn parse_report(helper: &Path, dll_dir: &str, text: &str) -> LmasterReport {
    let mut rep = LmasterReport {
        helper: helper.to_path_buf(),
        dll_dir: dll_dir.to_string(),
        exports_ok: None,
        exports_resolved: 0,
        exports_reason: None,
        begin_ok: None,
        begin_reason: None,
        langs: Vec::new(),
        tran_rets: Vec::new(),
        tran_fails: Vec::new(),
        end: None,
        lines: Vec::new(),
    };
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        rep.lines.push(t.to_string());
        match parse_lm_line(t) {
            LmLine::Exports {
                ok,
                resolved,
                reason,
                ..
            } => {
                rep.exports_ok = Some(ok);
                rep.exports_resolved = resolved;
                if !ok {
                    rep.exports_reason = reason;
                }
            }
            LmLine::Begin { ok, reason, .. } => {
                rep.begin_ok = Some(ok);
                if !ok {
                    rep.begin_reason = reason;
                }
            }
            LmLine::Lang(v) => rep.langs.push(v),
            LmLine::Tran { ret, .. } => rep.tran_rets.push(ret),
            LmLine::TranFail(reason) => rep.tran_fails.push(reason),
            LmLine::End(v) => rep.end = Some(v),
            LmLine::Hex(_) | LmLine::Diag(_) | LmLine::Bye | LmLine::Raw(_) => {}
        }
    }
    rep
}

/// The driver's outcome (zero-panic: a missing helper or a spawn failure is a
/// status, never a panic).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LmasterStatus {
    /// the i686 helper exe was not found in any candidate location
    NoHelper { tried: Vec<PathBuf> },
    /// the helper ran and produced a report
    Ok(LmasterReport),
    /// the helper could not be spawned / did not exit cleanly
    SpawnError(String),
}

/// Locate + speak the FFI facade protocol.
#[derive(Debug, Clone)]
pub struct LmasterDriver {
    pub helper: PathBuf,
}

impl LmasterDriver {
    /// Locate the i686 helper, in priority order: the explicit `--ffi32`
    /// path, then the `ZAKON_FFI32` env override, then next to the running
    /// exe, then the current directory, then the in-repo build location.
    /// Returns the first existing candidate; [`LmasterStatus::NoHelper`]
    /// (with the full search trail) when none exists.
    pub fn locate(explicit: Option<&Path>) -> Result<Self, LmasterStatus> {
        let mut tried: Vec<PathBuf> = Vec::new();
        if let Some(p) = explicit {
            tried.push(p.to_path_buf());
            if p.is_file() {
                return Ok(Self {
                    helper: p.to_path_buf(),
                });
            }
        }
        if let Ok(env) = std::env::var("ZAKON_FFI32") {
            let p = PathBuf::from(env);
            tried.push(p.clone());
            if p.is_file() {
                return Ok(Self { helper: p });
            }
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let p = dir.join("lmsup32-ffi32.exe");
                tried.push(p.clone());
                if p.is_file() {
                    return Ok(Self { helper: p });
                }
            }
        }
        {
            let p = Path::new("lmsup32-ffi32.exe").to_path_buf();
            tried.push(p.clone());
            if p.is_file() {
                return Ok(Self { helper: p });
            }
        }
        {
            let p = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("ffi32")
                .join("target")
                .join("i686-pc-windows-gnu")
                .join("release")
                .join("lmsup32-ffi32.exe");
            tried.push(p.clone());
            if p.is_file() {
                return Ok(Self { helper: p });
            }
        }
        Err(LmasterStatus::NoHelper { tried })
    }

    /// Speak the protocol: the first line is the DLL directory (where
    /// `lmsup32.dll` lives), then each command, then `BYE`. Never panics —
    /// spawn / exit problems are [`LmasterStatus::SpawnError`].
    pub fn run(&self, dll_dir: &str, commands: &[&str]) -> LmasterStatus {
        let mut prog = std::process::Command::new(&self.helper);
        prog.stdin(std::process::Stdio::piped());
        prog.stdout(std::process::Stdio::piped());
        prog.stderr(std::process::Stdio::null());
        let out = match prog.spawn() {
            Ok(mut child) => {
                let stdin = match child.stdin.take() {
                    Some(s) => s,
                    None => return LmasterStatus::SpawnError("stdin not piped".into()),
                };
                let mut w = std::io::BufWriter::new(stdin);
                let _ = writeln!(w, "{dll_dir}");
                for c in commands {
                    let _ = writeln!(w, "{c}");
                }
                let _ = writeln!(w, "BYE");
                let _ = w.flush();
                // drop the writer -> close stdin -> the helper sees EOF
                let _ = drop(w);
                match child.wait_with_output() {
                    Ok(o) if o.status.success() => o,
                    Ok(o) => {
                        return LmasterStatus::SpawnError(format!(
                            "helper exited with {}",
                            o.status
                        ));
                    }
                    Err(e) => return LmasterStatus::SpawnError(e.to_string()),
                }
            }
            Err(e) => return LmasterStatus::SpawnError(e.to_string()),
        };
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        LmasterStatus::Ok(parse_report(&self.helper, dll_dir, &text))
    }
}

/// The Rust-side ground-truth join for one corpus query (the cross-check
/// target for the DLL join): the query is run through the exporter's own
/// [`crate::trident::TridentIndex`] reader and the joined native u16
/// evidence is collected. This is the deterministic reference that
/// `LMaster_Tran` (once the Lm path global is unblocked) should agree with.
#[derive(Debug, Clone, PartialEq)]
pub struct CorpusJoin {
    pub query: String,
    /// the fraction of the query that could be encoded (0.0..=1.0)
    pub coverage: f64,
    /// the terms found in the dictionary
    pub matched: Vec<String>,
    /// the distinct native u16 doc-ids joined (the ground-truth evidence)
    pub docs: BTreeSet<u16>,
}

/// Run every non-`#` line of a corpus through the Rust reader and collect
/// the per-query ground-truth join. `lang` selects the language stream
/// (Ukrainian is the primary base).
pub fn corpus_crosscheck(
    baza: &Path,
    corpus: &Path,
    lang: crate::trident::Lang,
) -> Result<Vec<CorpusJoin>, String> {
    let Some(idx) = crate::trident::TridentIndex::open(baza) else {
        return Err(format!("no Trident index at {}", baza.display()));
    };
    let text =
        std::fs::read_to_string(corpus).map_err(|e| format!("corpus unreadable: {e}"))?;
    let queries: Vec<&str> = text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    if queries.is_empty() {
        return Err(format!("corpus empty: {}", corpus.display()));
    }
    let mut out = Vec::with_capacity(queries.len());
    for q in queries {
        let r = idx.search_lang(q, lang);
        let docs: BTreeSet<u16> = r
            .joined
            .iter()
            .flat_map(|(_, _, u)| u.iter().copied())
            .collect();
        out.push(CorpusJoin {
            query: q.to_string(),
            coverage: r.coverage,
            matched: r.matched.clone(),
            docs,
        });
    }
    Ok(out)
}

/// The aggregate of a cross-check run: `(total queries, queries with >=1
/// joined doc, the size of the union of all joined docs)` — one summary
/// line the CLI can print.
pub fn corpus_summary(joins: &[CorpusJoin]) -> (usize, usize, usize) {
    let total = joins.len();
    let with_docs = joins.iter().filter(|j| !j.docs.is_empty()).count();
    let union: BTreeSet<u16> = joins
        .iter()
        .flat_map(|j| j.docs.iter().copied())
        .collect();
    (total, with_docs, union.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_exports_ok() {
        match parse_lm_line("EXPORTS OK 4/4") {
            LmLine::Exports {
                ok,
                resolved,
                total,
                ..
            } => {
                assert!(ok);
                assert_eq!(resolved, 4);
                assert_eq!(total, 4);
            }
            other => panic!("expected Exports, got {other:?}"),
        }
    }

    #[test]
    fn parse_exports_fail() {
        match parse_lm_line("EXPORTS FAIL LoadLibraryW failed (err 126)") {
            LmLine::Exports { ok, resolved, reason, .. } => {
                assert!(!ok);
                assert_eq!(resolved, 0);
                assert!(reason.as_deref().unwrap_or("").contains("err 126"));
            }
            other => panic!("expected Exports, got {other:?}"),
        }
    }

    #[test]
    fn parse_begin_ok_and_fail() {
        match parse_lm_line("BEGIN OK handle=1234") {
            LmLine::Begin { ok, handle, .. } => {
                assert!(ok);
                assert_eq!(handle.as_deref(), Some("1234"));
            }
            other => panic!("expected Begin, got {other:?}"),
        }
        match parse_lm_line("BEGIN FAIL LMaster_Begin returned null (engine init failed)") {
            LmLine::Begin { ok, handle, reason } => {
                assert!(!ok);
                assert!(handle.is_none());
                assert!(reason.as_deref().unwrap_or("").contains("null"));
            }
            other => panic!("expected Begin, got {other:?}"),
        }
    }

    #[test]
    fn parse_lang_tran_end() {
        assert_eq!(parse_lm_line("LANG 69"), LmLine::Lang(69));
        assert_eq!(
            parse_lm_line("TRAN 1 256"),
            LmLine::Tran {
                ret: 1,
                outlen: 256
            }
        );
        assert_eq!(
            parse_lm_line("TRAN FAIL not-begun"),
            LmLine::TranFail("not-begun".into())
        );
        assert_eq!(parse_lm_line("END 0 (never begun)"), LmLine::End(0));
        assert_eq!(parse_lm_line("HEX 00 01 02"), LmLine::Hex("00 01 02".into()));
    }

    #[test]
    fn parse_raw_and_bye() {
        assert_eq!(parse_lm_line("BYE"), LmLine::Bye);
        assert_eq!(
            parse_lm_line("DIAG lm.a true (cwd=C:\\x)"),
            LmLine::Diag("DIAG lm.a true (cwd=C:\\x)".into())
        );
        assert_eq!(parse_lm_line(""), LmLine::Raw(String::new()));
        assert_eq!(
            parse_lm_line("FOO bar"),
            LmLine::Raw("FOO bar".to_string())
        );
    }

    #[test]
    fn parse_report_folds_the_full_observed_session() {
        // the empirically observed run: EXPORTS 4/4, Begin FAIL (LmOpen
        // Error), Lang 69, Tran not begun, End never begun
        let text = concat!(
            "EXPORTS OK 4/4\n",
            "BEGIN FAIL LMaster_Begin returned null (engine init failed)\n",
            "LANG 69\n",
            "TRAN FAIL not-begun\n",
            "END 0 (never begun)\n"
        );
        let rep = parse_report(Path::new("lmsup32-ffi32.exe"), "C:\\soft", text);
        assert_eq!(rep.exports_ok, Some(true));
        assert_eq!(rep.exports_resolved, 4);
        assert_eq!(rep.begin_ok, Some(false));
        assert!(rep.begin_reason.as_deref().unwrap_or("").contains("null"));
        assert_eq!(rep.langs, vec![69u32]);
        assert!(rep.tran_rets.is_empty());
        assert_eq!(rep.tran_fails, vec!["not-begun".to_string()]);
        assert_eq!(rep.exports_reason, None);
        assert_eq!(rep.end, Some(0));
        assert_eq!(rep.lines.len(), 5);
    }

    #[test]
    fn locate_reports_no_helper_with_a_trail() {
        // no --ffi32, no ZAKON_FFI32, and (on a bare checkout) none of the
        // candidate exes exist -> NoHelper with a non-empty search trail
        let _ = std::env::remove_var("ZAKON_FFI32");
        match LmasterDriver::locate(None) {
            Ok(_) => {
                // a checkout that has the in-repo build location resolves —
                // that is a valid success, not a failure
            }
            Err(LmasterStatus::NoHelper { tried }) => {
                assert!(!tried.is_empty());
            }
            other => panic!("expected Ok/NoHelper, got {other:?}"),
        }
    }

    #[test]
    fn corpus_summary_counts() {
        let joins = vec![
            CorpusJoin {
                query: "a".into(),
                coverage: 1.0,
                matched: vec!["a".into()],
                docs: vec![1u16, 2].into_iter().collect(),
            },
            CorpusJoin {
                query: "b".into(),
                coverage: 0.5,
                matched: vec![],
                docs: BTreeSet::new(),
            },
            CorpusJoin {
                query: "c".into(),
                coverage: 1.0,
                matched: vec!["c".into()],
                docs: vec![2u16, 3].into_iter().collect(),
            },
        ];
        let (total, with_docs, union) = corpus_summary(&joins);
        assert_eq!(total, 3);
        assert_eq!(with_docs, 2);
        assert_eq!(union, 3); // {1,2,3}
    }
}
