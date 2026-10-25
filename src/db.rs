//! .mb/.db access for the Zakon BAZA format.
//!
//! Document rendering (dictionary token expansion) lives in render.rs; this
//! module holds the container reader, the cp1251 codec and the search engine,
//! which runs over each document's *expanded* text (token ids replaced with
//! their words.db words) so Cyrillic queries match what the app displays.

use anyhow::{bail, Context, Result};
use encoding_rs::WINDOWS_1251;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

pub const MAGIC: u16 = 0xF44C;
/// alternative container magic found in the additional (GK) bases; the
/// count/size/data layout is identical there, only the signature differs
pub const MAGIC_ALT: u16 = 0xCA0F;
/// When the Zakon updater rewrites a base it re-stamps the containers while
/// keeping the layout (observed 2026-10-21: 0x064D for the main base's
/// re-synced files, 0xD70F for the fully rewritten GK base). Header layout
/// (count u16BE @7, u24BE size table @6, chunks @9+3*count) verified identical.
pub const MAGIC_NEW: u16 = 0x064D;
pub const MAGIC_NEW_ALT: u16 = 0xD70F;

// ============================================================
// File reading
// ============================================================

pub struct MbFile {
    pub path: PathBuf,
    pub check: u16,
    pub data: Vec<u8>,
}

impl MbFile {
    pub fn open(path: &Path) -> Result<Self> {
        let data = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
        if data.len() < 10 {
            bail!("{}: too small to be a BAZA container", path.display());
        }
        let check = u16::from_le_bytes([data[0], data[1]]);
        let magic = u16::from_le_bytes([data[2], data[3]]);
        if !matches!(magic, MAGIC | MAGIC_ALT | MAGIC_NEW | MAGIC_NEW_ALT) {
            bail!("{}: bad magic {magic:#4x}", path.display());
        }
        Ok(MbFile { path: path.to_path_buf(), check, data })
    }
}

// ============================================================
// Codec: cp1251 (Ukrainian letters at B0/AF/BA/BF/B2/B4 handled by
// encoding_rs).
// ============================================================

fn build_table() -> [char; 256] {
    let mut t = ['\u{FFFD}' as char; 256];
    for (i, slot) in t.iter_mut().enumerate() {
        let byte = [i as u8];
        let (cow, _, _) = WINDOWS_1251.decode(&byte);
        *slot = cow.chars().next().unwrap_or('\u{FFFD}');
    }
    t
}

fn table() -> &'static [char; 256] {
    use std::sync::OnceLock;
    static T: OnceLock<[char; 256]> = OnceLock::new();
    T.get_or_init(build_table)
}

/// cp1251 -> String, keeping spaces and newlines, dropping other controls.
pub fn decode_display(raw: &[u8]) -> String {
    let t = table();
    raw.iter()
        .filter(|&&b| b >= 0x20)
        .map(|&b| t[b as usize])
        .collect()
}

/// cp1251 -> String keeping newline as line separator (for expanded docs).
pub fn decode_keep_newlines(raw: &[u8]) -> String {
    let t = table();
    raw.iter()
        .filter(|&&b| b >= 0x20 || b == b'\n')
        .map(|&b| t[b as usize])
        .collect()
}

/// Expanded doc bytes -> tidy UTF-8 text (trimmed lines, blank lines collapsed).
pub fn tidy_expanded(raw: &[u8]) -> String {
    tidy_expanded_impl(raw, false)
}

/// Same as tidy_expanded, but keeps every line's leading whitespace — the
/// source encodes centering and list steps with indentation, so the preview
/// and the MD/RTF/DOCX exports keep the original look. Trailing whitespace
/// is still dropped and blank-line runs still collapse.
pub fn tidy_expanded_pretty(raw: &[u8]) -> String {
    tidy_expanded_impl(raw, true)
}

fn tidy_expanded_impl(raw: &[u8], keep_indent: bool) -> String {
    let text = decode_keep_newlines(raw);
    let mut lines: Vec<String> = Vec::new();
    for line in text.lines() {
        let l = if keep_indent { line.trim_end() } else { line.trim() };
        if l.is_empty() {
            if lines.last().map_or(true, |s: &String| s.is_empty()) {
                continue;
            }
            lines.push(String::new());
        } else {
            lines.push(l.to_string());
        }
    }
    while lines.last().map_or(false, |s| s.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

/// Case-fold table: fold uppercase Cyrillic (C0..DF) and ASCII (A..Z) to
/// their lowercase cp1251 byte form.
fn fold_table() -> &'static [u8; 256] {
    use std::sync::OnceLock;
    static F: OnceLock<[u8; 256]> = OnceLock::new();
    F.get_or_init(|| {
        let mut f = [0u8; 256];
        for (b, out) in f.iter_mut().enumerate() {
            *out = if (0xC0..=0xDF).contains(&b) { (b + 0x20) as u8 } else { b as u8 };
        }
        f
    })
}

/// char -> folded cp1251 byte map (chars that decode from two byte values are
/// resolved first-wins in favor of the lowercase form).
pub fn char_to_byte() -> &'static HashMap<char, u8> {
    use std::sync::OnceLock;
    static M: OnceLock<HashMap<char, u8>> = OnceLock::new();
    M.get_or_init(|| {
        let t = table();
        let mut m = HashMap::new();
        // first pass: lowercase bytes win
        for b in 0x20..0x100u16 {
            let ch = t[b as usize];
            m.entry(ch).or_insert(b as u8);
        }
        m
    })
}

/// Encode a search query as folded cp1251 bytes. Chars outside cp1251 are dropped.
pub fn query_bytes(query: &str) -> Vec<u8> {
    let m = char_to_byte();
    query
        .chars()
        .flat_map(|c| c.to_lowercase())
        .filter_map(|c| m.get(&c).copied())
        .collect()
}

// ============================================================
// Search over expanded documents
// ============================================================

#[derive(Debug, Clone)]
pub struct Hit {
    pub docnum: u32,
    /// text container file name, e.g. "223.mb"
    pub file: String,
    /// byte offset of the first match within the document's expanded text
    /// (whitespace-normalized: space/newline runs collapsed to one space)
    pub match_off: u64,
    /// number of query occurrences in this document
    pub matches: u32,
    /// human-readable context around the first match
    pub title: String,
}

/// Search mode for multi-term queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchMode {
    /// every term must occur somewhere in the document (default)
    And,
    /// at least one term must occur
    Or,
    /// the whole query must occur as one contiguous normalized phrase
    Phrase,
}

impl MatchMode {
    pub fn name(self) -> &'static str {
        match self {
            MatchMode::And => "AND",
            MatchMode::Or => "OR",
            MatchMode::Phrase => "PHRASE",
        }
    }
}

fn folded_cmp(data: &[u8], pos: usize, qb: &[u8], fold: &[u8; 256]) -> bool {
    data[pos..pos + qb.len()].iter().zip(qb).all(|(&d, &q)| fold[d as usize] == q)
}

/// Scan normalized text for non-overlapping occurrences of one folded term.
/// Returns (occurrence count, first offset).
fn count_term(norm: &[u8], qb: &[u8], fold: &[u8; 256]) -> (usize, Option<usize>) {
    if qb.is_empty() || norm.len() < qb.len() {
        return (0, None);
    }
    let first = qb[0];
    let scan_end = norm.len() - qb.len();
    let mut pos = 0usize;
    let mut first_off: Option<usize> = None;
    let mut count = 0usize;
    while pos <= scan_end {
        if fold[norm[pos] as usize] == first && folded_cmp(&norm, pos, qb, fold) {
            if first_off.is_none() {
                first_off = Some(pos);
            }
            count += 1;
            pos += qb.len();
        } else {
            pos += 1;
        }
    }
    (count, first_off)
}

// ============================================================
// Document-number queries («№ 249» / «249» / «N 249»)
// ============================================================

/// Cut off a leading marker (single char or word) from a «номер» query.
/// Byte-safe: the match length is taken from the char count.
fn strip_lead_marker(t: &str, marker: &str) -> String {
    let n = marker.chars().count();
    let head: String = t.chars().take(n).collect::<String>().to_lowercase();
    if head == marker {
        t.chars().skip(n).collect()
    } else {
        t.to_string()
    }
}

/// Core of a document-number query: strip any combination of the word
/// «номер», the sign «№» and the markers N / n / Н / н, then take the first
/// remaining token («249», «175-XIV», «249/2»). The token must contain a
/// digit, otherwise the query doesn't look like a document number and the
/// whole call errors out (so «роз'яснення» never silently scans the base).
pub fn parse_number_query(query: &str) -> Result<String> {
    let mut s = query.trim().to_string();
    loop {
        let t = s.trim();
        if t.is_empty() {
            bail!("у запиті немає номера документа: '{query}'");
        }
        let stripped = strip_lead_marker(t, "номер");
        let t2 = stripped.trim();
        if t2.is_empty() {
            bail!("у запиті немає номера документа: '{query}'");
        }
        let c = t2.chars().next().unwrap();
        if c == '№' || c == 'N' || c == 'n' || c == 'Н' || c == 'н' {
            s = t2[c.len_utf8()..].to_string();
            continue;
        }
        s = t2.to_string();
        break;
    }
    // first token, keeping digits + alphanumerics + номера-суфікси
    let tok: String = s
        .split_whitespace()
        .next()
        .unwrap_or("")
        .chars()
        .take_while(|&c| c.is_alphanumeric() || "-/.,()".contains(c))
        .collect();
    if tok.is_empty() || !tok.chars().any(|c| c.is_ascii_digit()) {
        bail!("запит не виглядає як номер документа: '{query}'");
    }
    Ok(tok)
}

/// Occurrences of a document number in whitespace-normalized text. The
/// number must follow a marker — «№» (0xB9), latin N/n (0x4E/0x6E; the fold
/// table does not case-fold ASCII) or cyrillic Н/н (0xCD/0xED) — with one
/// optional space, and may not be followed by another digit, so «249» never
/// matches inside «№ 2490».
fn count_number(norm: &[u8], qb: &[u8], fold: &[u8; 256]) -> (usize, Option<usize>) {
    const MARKS: [u8; 5] = [0xB9, 0x4E, 0x6E, 0xCD, 0xED];
    if qb.is_empty() || norm.len() <= qb.len() {
        return (0, None);
    }
    let mut count = 0usize;
    let mut first_off: Option<usize> = None;
    for pos in 0..norm.len() {
        if !MARKS.contains(&norm[pos]) {
            continue;
        }
        // left boundary of the marker: start of text, space or «. punctuation»
        // (an «н» glued to a word — «отримано 249» — must not count)
        let prev_ok = pos == 0 || {
            let b = norm[pos - 1];
            b == 0x20
                || (0x21..=0x2F).contains(&b) // !\"#$%&'()*+,-./
                || b == 0x3A || b == 0x3B // : ;
                || b == 0xAB || b == 0xBB // « »
        };
        if !prev_ok {
            continue;
        }
        let mut p = pos + 1;
        if norm.get(p) == Some(&0x20) {
            p += 1;
        }
        if p + qb.len() > norm.len() || !folded_cmp(norm, p, qb, fold) {
            continue;
        }
        if let Some(&d) = norm.get(p + qb.len()) {
            if (0x30..=0x39).contains(&d) {
                continue;
            }
        }
        if first_off.is_none() {
            first_off = Some(pos);
        }
        count += 1;
    }
    (count, first_off)
}

/// Split a search query into folded, whitespace-normalized cp1251 terms.
/// And/Or split on whitespace; Phrase keeps the whole query as one term.
pub fn build_terms(query: &str, mode: MatchMode) -> Result<Vec<Vec<u8>>> {
    let parts: Vec<&str> = match mode {
        MatchMode::Phrase => vec![query],
        _ => query.split_whitespace().collect(),
    };
    let mut terms = Vec::new();
    for p in parts {
        let t = normalize_ws(&query_bytes(p));
        if !t.is_empty() {
            terms.push(t);
        }
    }
    if terms.is_empty() {
        bail!("query has no searchable cp1251 characters");
    }
    Ok(terms)
}

/// Collapse whitespace runs (spaces/newlines) into single spaces and drop
/// leading/trailing/edge whitespace, on folded cp1251 bytes. The app's
/// expanded text uses doubled spaces for alignment; the query must match
/// across them (and across line breaks).
fn normalize_ws(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    for &b in data {
        if b == 0x20 || b == b'\n' || b == b'\r' || b == b'\t' {
            if out.last() != Some(&0x20) && !out.is_empty() {
                out.push(0x20);
            }
        } else {
            out.push(b);
        }
    }
    while out.last() == Some(&0x20) {
        out.pop();
    }
    out
}

fn title_from_context(data: &[u8], pos: usize, max_chars: usize) -> String {
    let a = pos.saturating_sub(64);
    let b = (pos + 160).min(data.len());
    let clean: String = decode_keep_newlines(&data[a..b])
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    let joined: String = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    let cut: String = joined.chars().take(max_chars).collect();
    // ellipses show the snippet is a fragment (cut on both sides)
    let lead = if a > 0 { "…" } else { "" };
    let tail = if b < data.len() || cut.chars().count() < joined.chars().count() { "…" } else { "" };
    format!("{lead}{cut}{tail}").trim_end().to_string()
}

/// Search all documents. The text is the *expanded* token stream (dict ids
/// resolved through words.db/words.mb), so the query matches the visible text.
/// `progress(done, total, found)` — total = text containers.
pub fn search_all<P>(baza: &Path, query: &str, mode: MatchMode, progress: &P) -> Result<Vec<Hit>>
where
    P: Fn(usize, usize, usize) -> bool + Sync,
{
    let terms = build_terms(query, mode)?;
    if !baza.join("text").is_dir() {
        bail!("text/ directory not found in {}", baza.display());
    }
    let base = crate::render::Base::load(baza)?;
    search_base(&base, baza, &terms, mode, progress)
}

/// Same search with a preloaded `Base` — the GUI reuses one load for the
/// pass over hit fields. `terms` must come from `build_terms`.
pub fn search_base<P>(
    base: &crate::render::Base,
    baza: &Path,
    terms: &[Vec<u8>],
    mode: MatchMode,
    progress: &P,
) -> Result<Vec<Hit>>
where
    P: Fn(usize, usize, usize) -> bool + Sync,
{
    scan_base(base, baza, &ScanTerms::Terms { terms, mode }, progress)
}

/// Number search (документ-номер) with a preloaded `Base`; `core` comes
/// from `parse_number_query` (e.g. «249»).
pub fn search_number_base<P>(
    base: &crate::render::Base,
    baza: &Path,
    core: &str,
    progress: &P,
) -> Result<Vec<Hit>>
where
    P: Fn(usize, usize, usize) -> bool + Sync,
{
    scan_base(base, baza, &ScanTerms::Number { core: query_bytes(core) }, progress)
}

/// Number search over all documents: parses «№ 249» / «249» / «N 249» /
/// «номер 249» into its number core and scans every document's expanded
/// text for «№ core» occurrences (the requisites of the document are part
/// of the text, so the document's own number matches).
pub fn search_number_all<P>(baza: &Path, query: &str, progress: &P) -> Result<Vec<Hit>>
where
    P: Fn(usize, usize, usize) -> bool + Sync,
{
    let core = parse_number_query(query)?;
    if !baza.join("text").is_dir() {
        bail!("text/ directory not found in {}", baza.display());
    }
    let base = crate::render::Base::load(baza)?;
    search_number_base(&base, baza, &core, progress)
}

/// What the scanner counts in each document's normalized text.
pub enum ScanTerms<'a> {
    /// multi-term AND / OR / PHRASE search
    Terms { terms: &'a [Vec<u8>], mode: MatchMode },
    /// document-number search: «№ / n / н <core>» occurrences
    Number { core: Vec<u8> },
}

impl ScanTerms<'_> {
    /// Evaluate over one document's normalized, folded cp1251 text:
    /// (matched?, occurrence count, offset of the first match)
    fn eval(&self, norm: &[u8], fold: &[u8; 256]) -> (bool, usize, Option<usize>) {
        match self {
            ScanTerms::Terms { terms, mode } => {
                let mut total_occ = 0usize;
                let mut first_off: Option<usize> = None;
                let mut all_present = true;
                let mut any_present = false;
                for qb in terms.iter() {
                    let (n, off) = count_term(norm, qb, fold);
                    if n > 0 {
                        total_occ += n;
                        any_present = true;
                        if first_off.map_or(true, |o| off.unwrap_or(o) < o) {
                            first_off = off;
                        }
                    } else {
                        all_present = false;
                    }
                }
                let matched = match mode {
                    MatchMode::And => all_present && first_off.is_some(),
                    MatchMode::Or | MatchMode::Phrase => any_present,
                };
                (matched, total_occ, first_off)
            }
            ScanTerms::Number { core } => {
                let (n, off) = count_number(norm, core, fold);
                (n > 0, n, off)
            }
        }
    }
}

/// Shared scanner: expands every document and counts matches per `sc`.
fn scan_base<P>(
    base: &crate::render::Base,
    baza: &Path,
    sc: &ScanTerms,
    progress: &P,
) -> Result<Vec<Hit>>
where
    P: Fn(usize, usize, usize) -> bool + Sync,
{
    let file_ids = base.file_ids();
    let total = file_ids.len();

    let hits: Mutex<Vec<Hit>> = Mutex::new(Vec::new());
    let done = AtomicUsize::new(0);
    let found = AtomicUsize::new(0);

    file_ids.par_iter().for_each(|&file| {
        let path = baza.join("text").join(format!("{file}.mb"));
        let mut file_hits: Vec<Hit> = Vec::new();
        if let Ok(mb) = MbFile::open(&path) {
            let mut last_doc = u32::MAX;
            for &(chunk, docnum) in base.file_docs(file) {
                if docnum == last_doc {
                    continue; // duplicate docnum mapping; first wins
                }
                last_doc = docnum;
                let ok = (|| -> anyhow::Result<Vec<u8>> {
                    let (mode, chunk2, file2) = base.span(docnum)?;
                    if file2 != file || chunk2 != chunk {
                        bail!("docloc mismatch for {docnum}");
                    }
                    let cb = crate::render::chunk_slice(&mb.data, chunk as u32)?;
                    crate::render::expand_chunk(mode, cb, base.words())
                })();
                if let Ok(exp) = ok {
                    let norm = normalize_ws(&exp);
                    let fold = fold_table();
                    let (ok_hit, total_occ, first_off) = sc.eval(&norm, fold);
                    if ok_hit {
                        let off = first_off.unwrap_or(0);
                        let t = title_from_context(&norm, off, 90);
                        let title = if t.is_empty() {
                            format!("документ {docnum}")
                        } else {
                            t
                        };
                        file_hits.push(Hit {
                            docnum,
                            file: format!("{file}.mb"),
                            match_off: off as u64,
                            matches: total_occ.min(u32::MAX as usize) as u32,
                            title,
                        });
                    }
                }
            }
        }
        let d = done.fetch_add(1, Ordering::SeqCst) + 1;
        found.fetch_add(file_hits.len(), Ordering::SeqCst);
        hits.lock().unwrap().extend(file_hits);
        if d % 8 == 0 || d == total {
            progress(d, total, found.load(Ordering::SeqCst));
        }
    });

    let mut all = hits.into_inner().unwrap();
    all.sort_by(|a, b| {
        let ka = file_key(&a.file);
        let kb = file_key(&b.file);
        ka.cmp(&kb).then(a.docnum.cmp(&b.docnum)).then(a.match_off.cmp(&b.match_off))
    });
    Ok(all)
}

fn file_key(file: &str) -> u32 {
    file.trim_end_matches(".mb").parse().unwrap_or(u32::MAX)
}

// ============================================================
// tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_vectors() {
        let t = table();
        assert_eq!(t[0xCA], 'К');
        assert_eq!(t[0xEA], 'к');
        assert_eq!(t[0xB3], 'і');
        assert_eq!(t[0xAF], 'Ї');
    }

    /// cp1251 bytes of a test string (synthetic doc text)
    fn enc(s: &str) -> Vec<u8> {
        let (cow, _, had) = WINDOWS_1251.encode(s);
        assert!(!had, "cp1251 encode failed");
        cow.to_vec()
    }

    #[test]
    fn number_query_core() {
        assert_eq!(parse_number_query("№ 249").unwrap(), "249");
        assert_eq!(parse_number_query("249").unwrap(), "249");
        assert_eq!(parse_number_query("N 249").unwrap(), "249");
        assert_eq!(parse_number_query("n249").unwrap(), "249");
        assert_eq!(parse_number_query("номер 249").unwrap(), "249");
        assert_eq!(parse_number_query("  №№ 63  ").unwrap(), "63");
        assert_eq!(parse_number_query("175-XIV").unwrap(), "175-XIV");
        assert_eq!(parse_number_query("№ 249/2").unwrap(), "249/2");
        assert!(parse_number_query("").is_err());
        assert!(parse_number_query("роз'яснення").is_err());
        assert!(parse_number_query("№").is_err());
    }

    #[test]
    fn count_number_variants() {
        let fold = fold_table();
        let core = parse_number_query("№ 249").unwrap();
        let qb = query_bytes(&core);
        // «№ 249» in the requisites line + «249» quoted after №
        let doc = enc("від 08 листопада 2018 р. № 249 (далі — Закон № 249)");
        let (n, off) = count_number(&normalize_ws(&doc), &qb, fold);
        assert_eq!(n, 2);
        assert!(off.is_some());
        // latin N and cyrillic н markers
        let doc = enc("published as N 249 by the archival branch н 249");
        let (n, _) = count_number(&normalize_ws(&doc), &qb, fold);
        assert_eq!(n, 2);
        // not a substring of a longer number, and digits alone don't count
        let doc = enc("errors № 2490 and 248 and 2018 and №2489");
        let (n, _) = count_number(&normalize_ws(&doc), &qb, fold);
        assert_eq!(n, 0);
        // marker required: the bare number in «у 249 колективі» doesn't count,
        // and «н» glued to a word is not a marker
        let doc = enc("у 249 колективах згадано 249 разів, отримано 249 письмових");
        let (n, _) = count_number(&normalize_ws(&doc), &qb, fold);
        assert_eq!(n, 0);
        // marker may follow punctuation, not only spaces
        let doc = enc("згідно з «№ 249», а також (№249): правки № 249;");
        let (n, _) = count_number(&normalize_ws(&doc), &qb, fold);
        assert_eq!(n, 3);
        // no space between the marker and the number
        let doc = enc("згідно з №249 та № № 249");
        let (n, _) = count_number(&normalize_ws(&doc), &qb, fold);
        assert_eq!(n, 2);
    }

    #[test]
    fn tidy_pretty_keeps_indent() {
        let raw = enc("\n     РОЗПОРЯДЖЕННЯ\n   \n    ПРО ДЕЩО\n\n\nзавтра \n");
        let t = tidy_expanded(&raw);
        let p = tidy_expanded_pretty(&raw);
        assert_eq!(t, "РОЗПОРЯДЖЕННЯ\n\nПРО ДЕЩО\n\nзавтра");
        assert_eq!(p, "     РОЗПОРЯДЖЕННЯ\n\n    ПРО ДЕЩО\n\nзавтра");
    }

    #[test]
    fn number_match_real_doc() {
        // main-base doc 8477 (Лист ДМС від 30.07.97) carries «N 14/2-466» in
        // its text; the number matcher must hit it without the query marker.
        let baza = std::path::Path::new("C:\\Zakon.Local\\Baza");
        if !baza.exists() {
            return;
        }
        let base = crate::render::Base::load(baza).unwrap();
        let raw = base.doc_text_bytes(baza, 8477).unwrap();
        let norm = normalize_ws(&raw);
        let qb = query_bytes("14/2-466");
        let (n, off) = count_number(&norm, &qb, fold_table());
        assert!(n >= 1, "8477: number «14/2-466» not matched ({n})");
        let ctx = title_from_context(&norm, off.unwrap(), 80);
        eprintln!("context: {ctx}");
        assert!(ctx.contains("14/2-466"), "context: {ctx}");
    }

    #[test]
    fn folded_query() {
        let qb = query_bytes("безспірного");
        assert_eq!(qb[0], b'\xe1');
        assert!(qb.contains(&0xB3)); // і
    }

    #[test]
    fn term_splitting_modes() {
        let and = build_terms("Київ ЯНУКОВИЧ", MatchMode::And).unwrap();
        assert_eq!(and.len(), 2, "And/Or split query on whitespace");
        let or = build_terms("Київ ЯНУКОВИЧ", MatchMode::Or).unwrap();
        assert_eq!(or.len(), 2);
        let phrase = build_terms("Київ ЯНУКОВИЧ", MatchMode::Phrase).unwrap();
        assert_eq!(phrase.len(), 1, "Phrase keeps the whole query as one term");
        assert!(build_terms("", MatchMode::And).is_err());
        assert!(build_terms("🙂🙂", MatchMode::And).is_err(), "non-cp1251 query bails");
    }

    #[test]
    fn doc136974_searchable_and_expanded() {
        let baza = Path::new("C:\\Zakon.Local\\Baza");
        if !baza.exists() {
            return;
        }
        let base = crate::render::Base::load(baza).unwrap();
        let text = base.doc_text(baza, 136974).unwrap();
        assert!(text.contains("П О С Т А Н О В А"));
        assert!(
            text.contains("безспірного"),
            "expanded doc 136974 should contain the query phrase"
        );
    }
}
