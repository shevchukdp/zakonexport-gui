//! Pure-Rust full-text search over the base's `.mb` document files.
//!
//! The legacy Trident application's search results (the "oracle") are a
//! case-insensitive **exact-word** scan of the document `.mb` files — *not*
//! the packed B-tree dictionary. This module reproduces that mechanism
//! directly: it encodes the query through the base's **custom codepage** and
//! scans every `.mb` file, reporting `(file, offset)` hits in the same
//! shape the oracle uses.
//!
//! ## Base codepage (discovered from the raw `class1.mb` bytes)
//!
//! The document text is **not** plain cp1251 or LmCode — it is cp1251
//! (standard Cyrillic) with the Ukrainian letters placed in the
//! `0x80–0xBF` symbol range:
//!
//! - standard Cyrillic (cp1251):
//!   - upper `А–П` = `0xC0–0xCF`, `Р–Я` = `0x90–0x9F`;
//!   - lower `а–п` = `0xE0–0xEF`, `р–я` = `0xF0–0xFF`;
//! - Ukrainian overrides:
//!   - `І` = `0xB2`, `і` = `0xB3` (confirmed — «Конституція» in
//!     `class1.mb` = `CA EE ED F1 F2 E8 F2 F3 F6 B3 FF` at the oracle's
//!     own offset 42 = 0x2A);
//!   - `Є` = `0xAA`, `ї` = `0xBF` (probed from the document text);
//!   - `є`/`ґ`/`Ї`/`Г` (the remaining slots) are rare in the base and are
//!     not encoded yet (reported via coverage).
//!
//! ## Reproduction
//!
//! For «конституція» this finds the exact word case-insensitively
//! (lowercase + title-case) across `C:\Zakon.Local\Baza`:
//! 209 (lowercase) + 13 (title) = **222 ≈ the oracle's 223** (99.5%). The
//! stem variant («конституц») is *not* what the oracle counts (it would be
//! ~12 000), confirming the oracle is an exact-word match.

use std::fs;
use std::path::{Path, PathBuf};

/// One `(file, offset)` hit of the full-text scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FtHit {
    /// path relative to the base (e.g. `text\1042.mb`)
    pub file: PathBuf,
    /// byte offset of the match inside the file
    pub offset: u64,
}

/// The result of a full-text scan.
#[derive(Debug, Clone, Default)]
pub struct FtResult {
    pub hits: Vec<FtHit>,
    /// how many `.mb` files were scanned
    pub files_scanned: usize,
    /// query characters that could be encoded (0..=total)
    pub encodable: u32,
    /// total query characters
    pub total: u32,
}

impl FtResult {
    /// `encodable/total` in 0..=1.0 (1.0 when the query is fully encodable)
    pub fn coverage(&self) -> f64 {
        if self.total == 0 {
            return 1.0;
        }
        self.encodable as f64 / self.total as f64
    }
}

// ---------------------------------------------------------------------------
// Base codepage
// ---------------------------------------------------------------------------

/// Lower-case letter → base-codepage byte (lower slots). Digits are stored
/// verbatim. `None` for characters the base codepage does not define
/// (Latin, …) — those are excluded from the encoded query and count against
/// coverage.
fn lower_byte(c: char) -> Option<u8> {
    if c.is_ascii_digit() {
        return Some(c as u8);
    }
    const LOWER: [(char, u8); 36] = [
        ('а', 0xE0), ('б', 0xE1), ('в', 0xE2), ('г', 0xE3), ('д', 0xE4),
        ('е', 0xE5), ('ж', 0xE6), ('з', 0xE7), ('и', 0xE8), ('й', 0xE9),
        ('к', 0xEA), ('л', 0xEB), ('м', 0xEC), ('н', 0xED), ('о', 0xEE),
        ('п', 0xEF),
        ('р', 0xF0), ('с', 0xF1), ('т', 0xF2), ('у', 0xF3), ('ф', 0xF4),
        ('х', 0xF5), ('ц', 0xF6), ('ч', 0xF7), ('ш', 0xF8), ('щ', 0xF9),
        ('ъ', 0xFA), ('ы', 0xFB), ('ь', 0xFC), ('э', 0xFD), ('ю', 0xFE),
        ('я', 0xFF),
        // Ukrainian overrides (base-specific; і/ї confirmed)
        ('і', 0xB3),
        ('ї', 0xBF),
        // є: confirmed at 0xBA by an .mb scan — the words «застосується» and
        // «затверджується» only match with the є-position byte = 0xBA
        ('є', 0xBA),
        // ґ: the base stores «загальний» etc. with the hard-г slot (0xE3);
        // fold ґ onto 0xE3 so Ukrainian query spellings still match
        ('ґ', 0xE3),
    ];
    LOWER.iter().find(|(ch, _)| *ch == c).map(|(_, b)| *b)
}

/// Upper-case letter → base-codepage byte (upper slots). Digits verbatim.
fn upper_byte(c: char) -> Option<u8> {
    if c.is_ascii_digit() {
        return Some(c as u8);
    }
    const UPPER: [(char, u8); 36] = [
        ('А', 0xC0), ('Б', 0xC1), ('В', 0xC2), ('Г', 0xC3), ('Д', 0xC4),
        ('Е', 0xC5), ('Ж', 0xC6), ('З', 0xC7), ('И', 0xC8), ('Й', 0xC9),
        ('К', 0xCA), ('Л', 0xCB), ('М', 0xCC), ('Н', 0xCD), ('О', 0xCE),
        ('П', 0xCF),
        ('Р', 0x90), ('С', 0x91), ('Т', 0x92), ('У', 0x93), ('Ф', 0x94),
        ('Х', 0x95), ('Ц', 0x96), ('Ч', 0x97), ('Ш', 0x98), ('Щ', 0x99),
        ('Ъ', 0x9A), ('Ы', 0x9B), ('Ь', 0x9C), ('Э', 0x9D), ('Ю', 0x9E),
        ('Я', 0x9F),
        // Ukrainian overrides (base-specific; І/Є confirmed)
        ('І', 0xB2),
        ('Є', 0xAA),
        // Ґ: the base stores title-case «Г…» with the hard-Г slot (0xC3);
        // fold Ґ onto 0xC3 so Ukrainian query spellings still match
        ('Ґ', 0xC3),
        // Ї: no dedicated upper slot was found in the base — possessives
        // («їхніх» …) are stored with the lower ї byte; fold the title-case
        // query onto 0xBF
        ('Ї', 0xBF),
    ];
    UPPER.iter().find(|(ch, _)| *ch == c).map(|(_, b)| *b)
}

/// Encode a query term to base-codepage bytes for a given case style.
///
/// `title` = false: every character in its lower slot.
/// `title` = true: the first character is uppercased (via the upper slot),
/// the rest lower — the two forms the document text actually stores.
///
/// Returns `(bytes, encodable, total)`; unencodable characters are skipped
/// so a partially-encodable query still searches on its encodable part.
fn encode_term(term: &str, title: bool) -> (Vec<u8>, u32, u32) {
    let chars: Vec<char> = term.chars().collect();
    let total = chars.len() as u32;
    let mut bytes = Vec::new();
    let mut encodable = 0u32;
    for (i, c) in chars.iter().enumerate() {
        let b = if title && i == 0 {
            upper_byte(c.to_uppercase().next().unwrap_or(*c))
        } else {
            lower_byte(*c)
        };
        if let Some(b) = b {
            bytes.push(b);
            encodable += 1;
        }
    }
    (bytes, encodable, total)
}

/// The two case variants searched: all-lowercase and title-case.
fn query_variants(term: &str) -> Vec<(Vec<u8>, u32, u32)> {
    vec![encode_term(term, false), encode_term(term, true)]
}

// ---------------------------------------------------------------------------
// File collection + scan
// ---------------------------------------------------------------------------

/// Collect the `.mb` document files under a base: the top-level `*.mb`
/// plus everything under `text\` (recursive). This is the file set the
/// legacy oracle searched.
fn collect_mb_files(baza: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();
    if let Ok(rd) = fs::read_dir(baza) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_file() && is_mb(&p) {
                files.push(p);
            }
        }
    }
    let text = baza.join("text");
    if text.is_dir() {
        walk_mb(&text, &mut files);
    }
    files.sort();
    files.dedup();
    files
}

fn is_mb(p: &Path) -> bool {
    p.extension()
        .map(|x| x.to_string_lossy().eq_ignore_ascii_case("mb"))
        .unwrap_or(false)
}

fn walk_mb(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk_mb(&p, out);
            } else if is_mb(&p) {
                out.push(p);
            }
        }
    }
}

/// Count the occurrences of a (byte) pattern in a buffer, recording the
/// start offsets. Non-overlapping (advance past a match), matching the way
/// the oracle counts one document occurrence per hit. O(n) for a short
/// pattern.
fn scan_offsets(buf: &[u8], pat: &[u8]) -> Vec<u64> {
    let mut out = Vec::new();
    let plen = pat.len();
    if plen == 0 || plen > buf.len() {
        return out;
    }
    let mut i = 0usize;
    while i + plen <= buf.len() {
        if &buf[i..i + plen] == pat {
            out.push(i as u64);
            i += plen; // non-overlapping
        } else {
            i += 1;
        }
    }
    out
}

/// v0.39.0 (roadmap v0.40.0): the presence-only variant of
/// [`scan_offsets`] — early-break on the first match. Identical match
/// semantics (byte-exact), but no offset collection: the oracle's
/// distinct-file count needs *presence* (a hit in the file), not the hit
/// offsets. Used for the short (2-byte) variants of [`oracle_multi`] and
/// as the fallback presence probe.
fn scan_present(buf: &[u8], pat: &[u8]) -> bool {
    let plen = pat.len();
    if plen == 0 || plen > buf.len() {
        return false;
    }
    buf.windows(plen).any(|w| w == pat)
}

/// Full-text search a query term across the base's `.mb` document files
/// (lowercase + title-case), returning the `(file, offset)` hits in the
/// oracle's shape.
pub fn ft_search(baza: &Path, term: &str) -> FtResult {
    let variants = query_variants(term);
    // encodable/total are identical for the lower and title variants (same
    // characters, different case) — take the lower variant's counts.
    let enc = variants[0].1;
    let tot = variants[0].2;
    let files = collect_mb_files(baza);
    let mut hits: Vec<FtHit> = Vec::new();
    for f in &files {
        let buf = match fs::read(f) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let rel = f
            .strip_prefix(baza)
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|_| f.clone());
        // collect offsets from each variant, de-duplicate (a byte offset hit
        // by both the lower and the title variant is one hit)
        let mut offsets: Vec<u64> = Vec::new();
        for v in &variants {
            if v.0.is_empty() {
                continue;
            }
            let os = scan_offsets(&buf, &v.0);
            offsets.extend(os);
        }
        offsets.sort();
        offsets.dedup();
        for o in offsets {
            hits.push(FtHit {
                file: rel.clone(),
                offset: o,
            });
        }
    }
    hits.sort_by(|a, b| a.file.cmp(&b.file).then(a.offset.cmp(&b.offset)));
    FtResult {
        hits,
        files_scanned: files.len(),
        encodable: enc,
        total: tot,
    }
}

/// A per-term oracle agreement record: the native exact-join doc count vs
/// the oracle's distinct-file count, and their agreement in 0.0..=1.0.
///
/// The native u16 doc-ids and the oracle's `.mb` files are different
/// identifier spaces (the 2005 docnum space vs the current file set), so the
/// agreement is a *count* agreement (`min/max` of the two document counts),
/// not a set intersection. Both counts zero = vacuous agreement (1.0).
#[derive(Debug, Clone, PartialEq)]
pub struct OracleTerm {
    /// the query term
    pub term: String,
    /// distinct native u16 doc-ids from the exact join for this term
    pub tsearch_docs: usize,
    /// distinct oracle `.mb` files containing the term (lower + title case)
    pub oracle_files: usize,
    /// per-term count agreement in 0.0..=1.0 (1.0 when both are zero)
    pub agreement: f64,
}

impl OracleTerm {
    /// The count agreement: `min/max` of the two document counts; 1.0 when
    /// both are zero (vacuous), 0.0 when exactly one is zero.
    pub fn agreement(t_docs: usize, o_files: usize) -> f64 {
        match (t_docs, o_files) {
            (0, 0) => 1.0,
            (0, _) | (_, 0) => 0.0,
            (a, b) => {
                let (hi, lo) = if a >= b { (a, b) } else { (b, a) };
                lo as f64 / hi as f64
            }
        }
    }
}

/// The per-term oracle counts for a set of terms, produced by a **single
/// pass** over the base's `.mb` files (each file is read once; every term is
/// matched within that read). This keeps a 20-query corpus at one full scan
/// instead of twenty.
#[derive(Debug, Clone, Default)]
pub struct OracleCounts {
    /// per term: the distinct file count containing it (in the same order as
    /// the input `terms`)
    pub per_term: Vec<(String, usize)>,
    /// how many `.mb` files were scanned
    pub files_scanned: usize,
}

/// Count, for each `terms[i]`, how many distinct `.mb` files of the base
/// contain the term (lowercase + title-case variants) — in one pass over the
/// files (v0.39.0: a single-pass multi-pattern presence check per file —
/// a 3-byte-prefix bucket + a 256-bit byte set detect ALL the variants in
/// ONE pass over the file, instead of one pass per variant). The
/// distinct-file count is the oracle's "document count" for the term, used
/// by the `--oracle` cross-check and the `--corpus` matrix in `tsearch`.
pub fn oracle_multi(baza: &Path, terms: &[&str]) -> OracleCounts {
    let files = collect_mb_files(baza);
    // encode each term's variants once (lower + title), dropping empties
    let variants: Vec<Vec<Vec<u8>>> = terms
        .iter()
        .map(|t| {
            let mut vs = Vec::new();
            let (b, _, _) = encode_term(t, false);
            if !b.is_empty() {
                vs.push(b);
            }
            let (bt, _, _) = encode_term(t, true);
            if !bt.is_empty() && !vs.contains(&bt) {
                vs.push(bt);
            }
            vs
        })
        .collect();
    // per-term distinct-file counters (a HashSet of the relative file
    // paths); a worker pool over the files (one file per task, counters
    // under a Mutex) keeps the single-pass semantics — every file is
    // counted at most once per term — while scaling across cores.
    let counters: Vec<std::collections::HashSet<PathBuf>> =
        terms.iter().map(|_| std::collections::HashSet::new()).collect();
    let counters = std::sync::Mutex::new(counters);
    let nthreads = std::cmp::min(
        files.len(),
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .min(8),
    );
    // v0.39.0 (roadmap v0.40.0): the single-pass multi-pattern presence
    // check — instead of scanning the (several-GB) corpus once per variant
    // (a 100-query corpus = a couple hundred full passes), ONE pass over a
    // file detects ALL variants at once: a 3-byte-prefix bucket (every
    // variant with len >= 3), a 256-bit byte set (the 1-byte variants,
    // built in the same pass) and a short direct check (2-byte variants).
    // Identical presence semantics to the per-variant `scan_present`, with
    // ~200× less data moved.
    #[derive(Clone, Copy)]
    struct Slot {
        ti: usize,
        vi: usize,
    }
    let mut buckets: std::collections::HashMap<u64, Vec<Slot>> =
        std::collections::HashMap::new();
    for (ti, vs) in variants.iter().enumerate() {
        for (vi, v) in vs.iter().enumerate() {
            if v.len() >= 3 {
                let key = ((v[0] as u64) << 16) | ((v[1] as u64) << 8) | (v[2] as u64);
                buckets.entry(key).or_default().push(Slot { ti, vi });
            }
        }
    }
    let scan_file = |f: &Path| {
        let buf = match fs::read(f) {
            Ok(b) => b,
            Err(_) => return,
        };
        let rel = f
            .strip_prefix(baza)
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|_| f.to_path_buf());
        // the single-pass presence check: per-term hit flags + the 256-bit
        // byte set (for the 1-byte variants), built in the same pass
        let mut hits = vec![false; variants.len()];
        let mut byteset: [u64; 4] = [0; 4];
        let n = buf.len();
        for i in 0..n.saturating_sub(2) {
            let b = buf[i];
            byteset[(b / 64) as usize] |= 1u64 << (b % 64);
            let key = ((b as u64) << 16) | ((buf[i + 1] as u64) << 8) | (buf[i + 2] as u64);
            if let Some(slots) = buckets.get(&key) {
                for s in slots {
                    if !hits[s.ti] {
                        let v = &variants[s.ti][s.vi];
                        if buf[i..i + v.len()] == v[..] {
                            hits[s.ti] = true;
                        }
                    }
                }
            }
        }
        // the last two bytes still feed the byte set (edge)
        for i in n.saturating_sub(2)..n {
            let b = buf[i];
            byteset[(b / 64) as usize] |= 1u64 << (b % 64);
        }
        // the short variants: 1 byte via the byte set, 2 bytes direct
        for (ti, vs) in variants.iter().enumerate() {
            if hits[ti] {
                continue;
            }
            for v in vs {
                if v.len() == 1 {
                    let b = v[0];
                    if byteset[(b / 64) as usize] & (1u64 << (b % 64)) != 0 {
                        hits[ti] = true;
                        break;
                    }
                } else if v.len() == 2 && scan_present(&buf, v) {
                    hits[ti] = true;
                    break;
                }
            }
        }
        let hit_terms: Vec<usize> = (0..variants.len()).filter(|&ti| hits[ti]).collect();
        if hit_terms.is_empty() {
            return;
        }
        let mut guard = counters.lock().unwrap();
        for ti in hit_terms {
            guard[ti].insert(rel.clone());
        }
    };
    if nthreads > 1 {
        // `&Vec` and `&closure` are Copy — each worker moves its own copy
        let files_ref = &files;
        let scan_ref = &scan_file;
        std::thread::scope(|s| {
            let mut handles = Vec::new();
            for w in 0..nthreads {
                handles.push(s.spawn(move || {
                    for (fi, f) in files_ref.iter().enumerate() {
                        if fi % nthreads != w {
                            continue;
                        }
                        scan_ref(f);
                    }
                }));
            }
            for h in handles {
                h.join().unwrap();
            }
        });
    } else {
        for f in &files {
            scan_file(f);
        }
    }
    let counters = counters.lock().unwrap();
    let per_term = terms
        .iter()
        .enumerate()
        .map(|(i, t)| (t.to_string(), counters[i].len()))
        .collect();
    OracleCounts {
        per_term,
        files_scanned: files.len(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_present_early_break_matches_offsets() {
        // v0.39.0: the presence-only scan must agree with
        // `scan_offsets(...).is_empty()` on every edge case
        let buf = b"abcdefabcdef";
        let pats: &[&[u8]] = &[
            b"abc",
            b"def",
            b"abcabc",
            b"xyz",
            b"",
            b"abcdefghij", // longer than the buffer
            b"f",
        ];
        for pat in pats {
            assert_eq!(
                scan_present(buf, pat),
                !scan_offsets(buf, pat).is_empty(),
                "presence must match the offset scan for {pat:?}"
            );
        }
        assert!(scan_present(b"hello", b"llo"));
        assert!(!scan_present(b"", b"x"));
    }

    #[test]
    fn standard_cyrillic_is_cp1251() {
        assert_eq!(lower_byte('к'), Some(0xEA));
        assert_eq!(upper_byte('К'), Some(0xCA));
        assert_eq!(lower_byte('я'), Some(0xFF));
        assert_eq!(upper_byte('Я'), Some(0x9F));
        assert_eq!(lower_byte('р'), Some(0xF0));
        assert_eq!(upper_byte('Р'), Some(0x90));
    }

    #[test]
    fn ukrainian_overrides() {
        assert_eq!(lower_byte('і'), Some(0xB3));
        assert_eq!(upper_byte('І'), Some(0xB2));
        assert_eq!(upper_byte('Є'), Some(0xAA));
        assert_eq!(lower_byte('ї'), Some(0xBF));
        // confirmed by an .mb scan: «застосується» / «затверджується»
        assert_eq!(lower_byte('є'), Some(0xBA));
        // ґ/ґ fold onto the hard-г / hard-Г slots the base actually stores
        assert_eq!(lower_byte('ґ'), Some(0xE3), "lower ґ folds onto г");
        assert_eq!(upper_byte('Ґ'), Some(0xC3), "upper Ґ folds onto Г");
        // Ї folds onto the stored lower-ї byte (no dedicated upper slot)
        assert_eq!(upper_byte('Ї'), Some(0xBF), "upper Ї folds onto ї");
    }

    #[test]
    fn zahalnyi_encodes_with_folded_g() {
        // «загальний» is stored in the base with the hard-г byte; querying it
        // with the soft ґ must encode to the same byte run
        let (b, e, t) = encode_term("загальний", false);
        assert_eq!((e, t), (9, 9), "fully encodable");
        assert_eq!(
            b,
            vec![0xE7, 0xE0, 0xE3, 0xE0, 0xEB, 0xFC, 0xED, 0xE8, 0xE9]
        );
        // title-case first letter folds onto the hard-Г slot
        let (bt, et, tt) = encode_term("Загальний", true);
        assert_eq!((et, tt), (9, 9));
        // З (first, title) -> 0xC7; the 3rd-letter ґ stays in the lower fold
        // slot 0xE3
        assert_eq!(
            bt,
            vec![0xC7, 0xE0, 0xE3, 0xE0, 0xEB, 0xFC, 0xED, 0xE8, 0xE9]
        );
    }

    #[test]
    fn zastosuye_tsya_encodes_confirmed_ye() {
        // «застосується» — the є-position must encode to the confirmed 0xBA
        let (b, e, t) = encode_term("застосується", false);
        assert_eq!((e, t), (12, 12), "fully encodable");
        assert_eq!(
            b,
            vec![
                0xE7, 0xE0, 0xF1, 0xF2, 0xEE, 0xF1, 0xF3, 0xBA, 0xF2, 0xFC,
                0xF1, 0xFF
            ]
        );
    }

    #[test]
    fn konstitutsiya_encodes_as_observed() {
        // «Конституція» as stored in class1.mb (title case)
        let (b, e, t) = encode_term("конституція", true);
        assert_eq!((e, t), (11, 11), "fully encodable");
        assert_eq!(
            b,
            vec![
                0xCA, 0xEE, 0xED, 0xF1, 0xF2, 0xE8, 0xF2, 0xF3, 0xF6, 0xB3,
                0xFF
            ]
        );
        // lowercase
        let (bl, _, _) = encode_term("конституція", false);
        assert_eq!(
            bl,
            vec![
                0xEA, 0xEE, 0xED, 0xF1, 0xF2, 0xE8, 0xF2, 0xF3, 0xF6, 0xB3,
                0xFF
            ]
        );
    }

    #[test]
    fn scan_offsets_finds_matches() {
        let buf = [b'a', b'b', b'c', b'b', b'c', b'd'];
        assert_eq!(
            scan_offsets(&buf, b"bc"),
            vec![1u64, 3u64],
            "non-overlapping starts reported"
        );
        assert!(scan_offsets(&[b'a'], b"ab").is_empty());
    }

    #[test]
    fn oracle_term_agreement_classes() {
        // vacuous: both empty = 1.0
        assert_eq!(OracleTerm::agreement(0, 0), 1.0);
        // one side empty = 0.0
        assert_eq!(OracleTerm::agreement(5, 0), 0.0);
        assert_eq!(OracleTerm::agreement(0, 5), 0.0);
        // count agreement = min/max
        assert_eq!(OracleTerm::agreement(10, 10), 1.0);
        assert_eq!(OracleTerm::agreement(1, 10), 0.1);
        assert!((OracleTerm::agreement(9, 10) - 0.9).abs() < 1e-9);
    }

    #[test]
    fn oracle_multi_multipattern_matches_variants() {
        // v0.39.0 (roadmap v0.40.0): the single-pass multi-pattern
        // presence check (3-byte-prefix bucket + 256-bit byte set + short
        // direct) must count EXACTLY the same distinct files as an
        // independent per-variant reference — across 1/2/3+ byte variants
        // (lower + title case), including a term with zero hits.
        let dir = std::env::temp_dir().join(format!(
            "zakon_oracle_mp_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // cp1251 byte runs (the confirmed encodings from the tests above)
        let zak = [0xE7u8, 0xE0, 0xE3, 0xEB, 0xE5]; // «закон» lower
        let zak_t = [0xC7u8, 0xE0, 0xE3, 0xEB, 0xE5]; // «Закон» title (З = 0xC7)
        let files: Vec<(&str, Vec<u8>)> = vec![
            ("f1.mb", zak.to_vec()), // «закон» lower only
            ("f2.mb", zak_t.to_vec()), // «Закон» title only
            ("f3.mb", vec![0xE7]), // «з» (1 byte)
            ("f4.mb", vec![0xE7, 0xE0]), // «за» (2 bytes)
            ("f5.mb", vec![0xE7, 0xE0, 0xE3, 0xEB]), // «зак» (a 4-byte prefix run)
        ];
        for (name, bytes) in &files {
            std::fs::write(dir.join(name), bytes).unwrap();
        }
        let terms: Vec<&str> = vec!["закон", "з", "за", "конституція"];
        // the independent reference: a plain per-variant windows() scan,
        // mirroring oracle_multi's variant construction (lower + title)
        let mut expected: Vec<(String, usize)> = Vec::new();
        for t in &terms {
            let (b, _, _) = encode_term(t, false);
            let (bt, _, _) = encode_term(t, true);
            let mut vs = vec![b];
            if !bt.is_empty() && !vs.contains(&bt) {
                vs.push(bt);
            }
            let count = files
                .iter()
                .filter(|(_, data)| {
                    vs.iter().any(|v| {
                        !v.is_empty()
                            && data
                                .windows(v.len())
                                .any(|w| w == v.as_slice())
                    })
                })
                .count();
            expected.push((t.to_string(), count));
        }
        let r = oracle_multi(&dir, &terms);
        assert_eq!(r.per_term.len(), terms.len());
        for (t, want) in &expected {
            let got = r
                .per_term
                .iter()
                .find(|(n, _)| n == t)
                .map(|x| x.1)
                .unwrap_or(usize::MAX);
            assert_eq!(
                got, *want,
                "«{t}»: the multi-pattern pass must match the per-variant reference"
            );
        }
        // the zero-hit term stays zero (no false positives from the
        // 3-byte-prefix bucket or the byte set)
        assert_eq!(
            r.per_term.iter().find(|(n, _)| n == "конституція").unwrap().1,
            0
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn oracle_multi_counts_distinct_files_once() {
        let baza = Path::new(r"C:\Zakon.Local\Baza");
        if !baza.join("text").is_dir() {
            eprintln!("oracle multi test: skipped (no base)");
            return;
        }
        let r = oracle_multi(baza, &["конституція"]);
        assert_eq!(r.per_term.len(), 1);
        assert_eq!(r.per_term[0].0, "конституція");
        // the oracle reproduced ~222 hits across ~200 distinct files for this
        // term; the distinct-file count is what the --oracle check uses
        assert!(
            r.per_term[0].1 > 0,
            "конституція must occur in at least one .mb file"
        );
        assert!(r.files_scanned > 0);
    }

    #[test]
    fn real_base_reproduces_oracle() {
        let baza = Path::new(r"C:\Zakon.Local\Baza");
        if !baza.join("text").is_dir() {
            eprintln!("ft search test: skipped (no base)");
            return;
        }
        let r = ft_search(baza, "конституція");
        // 209 (lower) + 13 (title) ≈ 222 ≈ the oracle's 223
        assert!(
            (200..=240).contains(&r.hits.len()),
            "expected ~222 oracle hits, got {}",
            r.hits.len()
        );
        assert_eq!(r.coverage(), 1.0);
        // the oracle's own first hit: class1.mb @ 42
        assert!(
            r.hits
                .iter()
                .any(|h| h.file.to_string_lossy() == "class1.mb" && h.offset == 42),
            "oracle hit class1.mb@42 must be present"
        );
    }
}
