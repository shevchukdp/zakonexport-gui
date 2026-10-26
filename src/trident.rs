//! Trident native FTS index reader (v0.13.0 — experimental).
//!
//! The legacy main application (the 2002–2005 `zakon-gui.exe` suite) shipped a
//! native full-text index in `SOFT\`. This module reads the parts of that
//! index that can be decoded today, so the exporter can search "like the main
//! app" without the DLL:
//!
//! | file        | role                                                |
//! |-------------|-----------------------------------------------------|
//! | windows.kod | code table: 80 lines `<code> <sep> <byte>` (CRLF)   |
//! | lm.a        | case-fold tables (text, sections `E:` `G:` `R:` `U:`) |
//! | lmu.voc     | word dictionary (Ukrainian) — B-tree + records     |
//! | lmr.voc     | word dictionary (Russian)                           |
//! | lmur.vt     | postings (Ukrainian): a packed u16 doc-number stream|
//! | lmru.vt     | postings (Russian)                                  |
//! | lmur.m      | metadata registry (fields: `Subject`/`Object` …)    |
//! | lmru.m      | metadata registry (Russian)                         |
//! | lmu.g/.gw   | morphology grammar / graph weights (Ukrainian)      |
//! | lmr.g/.gw   | morphology grammar / graph weights (Russian)        |
//! | lmsup*.dll  | the 16/32-bit engine (exports `LMaster_*`)          |
//!
//! Word bytes live in a custom "LmCode" byte space: Cyrillic letter slots
//! occupy 0x80–0xEF (upper-case slots 0x80–0x9F, lower-case 0xA0–0xAF and
//! 0xE0–0xEF, plus the Ukrainian extras 0xF4–0xF9), Latin letters and digits
//! are plain ASCII. `windows.kod` maps the app's 2-digit codes to those
//! bytes (a raw byte when the value is >= 0x80, two ASCII hex digits below);
//! `lm.a` holds the case-fold tables:
//!
//! - `E:` — Latin identity pairs (`A a` … `Z z`);
//! - `G:` — cross-language anchors: Latin A ↔ `84 a4` (А/а), O ↔ `96 e6`
//!   (О/о), U ↔ `9c ec` (У/у), plus the standalone pair `9f ef` (Щ/щ);
//! - `R:` — 32 fold pairs, Russian;
//! - `U:` — 32 fold pairs, Ukrainian (the main range 0x9A/0x9B/0x9D is left
//!   unused; the extras are `f4 f5`, `f6 f7`, `f8 f9`).
//!
//! All binary files share a 0x3C header: a `FREE` marker followed by a table
//! of u32 section offsets.
//!
//! What is decoded today: `windows.kod`, `lm.a`, the file headers, the
//! dictionary records (`[0x84][word][00 00]`) and the u16 posting stream.
//! What is still open (v0.14.0 roadmap): the full character correspondence
//! (only the `G:`-section anchors are confirmed; the rest is a custom slot
//! order), the `.g`/`.gw` morphology graphs, the exact per-word posting
//! boundaries and the oracle-validated doc-number join.
//!
//! `tsearch` is therefore labelled *experimental*: it encodes a query
//! through the (partial) character table, looks the term up in the
//! dictionary (prefix-style, with suffix trimming) and reads a posting
//! slice from `.vt`. It reports its coverage so the caller knows how much
//! of the query was actually encoded.

use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// windows.kod — the code table
// ---------------------------------------------------------------------------

/// Parsed `windows.kod` code table.
///
/// The file is a sequence of lines `<code> <sep> <value>\r\n` (80 lines).
/// `<code>` is two ASCII hex digits; `<sep>` is `=` (direct mapping) or `>`
/// (fold / equivalence); `<value>` is a single raw byte when its value is
/// >= 0x80 (an optional trailing space precedes the CRLF on some lines) and
/// two ASCII hex digits when it is < 0x80.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kod {
    /// code (0x00-0xFF) -> LmCode byte; `None` = not mapped.
    map: [Option<u8>; 256],
    /// how many lines were accepted by the parser
    lines: usize,
}

impl Default for Kod {
    fn default() -> Self {
        Kod {
            map: [None; 256],
            lines: 0,
        }
    }
}

impl Kod {
    /// Parse the raw bytes of a `windows.kod` file.
    pub fn parse(data: &[u8]) -> Self {
        let mut map = [None; 256];
        let mut lines = 0usize;
        // Each line: [code:2 ascii hex][sp][sep][sp][value]... where the
        // value is either one raw byte (>=0x80) or two ascii hex digits.
        let mut i = 0usize;
        while i + 5 <= data.len() {
            // code = two ascii hex chars
            if let (Some(hi), Some(lo)) = (hex_val(data[i]), hex_val(data[i + 1])) {
                let code = hi * 16 + lo;
                // expect: space, sep ('='|'>'), space
                if i + 4 < data.len()
                    && data[i + 2] == b' '
                    && (data[i + 3] == b'=' || data[i + 3] == b'>')
                    && data[i + 4] == b' '
                {
                    let val = data[i + 5];
                    if val >= 0x80 {
                        // raw byte value
                        map[usize::from(code)] = Some(val);
                        lines += 1;
                        // advance past: code(2)+sp+sep+sp+val(1)
                        i += 6;
                        if i < data.len() && data[i] == b' ' {
                            i += 1; // optional trailing space
                        }
                        continue;
                    }
                    // two ascii hex digits
                    if i + 6 < data.len() {
                        if let (Some(a), Some(b)) = (hex_val(val), hex_val(data[i + 6])) {
                            map[usize::from(code)] = Some(a * 16 + b);
                            lines += 1;
                            // advance past: code(2)+sp+sep+sp+val(2)
                            i += 7;
                            continue;
                        }
                    }
                    i += 1;
                    continue;
                }
            }
            i += 1;
        }
        Kod { map, lines }
    }

    /// Map an app code to its LmCode byte.
    pub fn code_to_byte(&self, code: u8) -> Option<u8> {
        self.map[usize::from(code)]
    }

    /// Number of defined codes.
    pub fn len(&self) -> usize {
        self.map.iter().filter(|x| x.is_some()).count()
    }

    /// Number of lines the parser accepted (the real table has 80).
    pub fn lines(&self) -> usize {
        self.lines
    }
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// lm.a — the case-fold tables
// ---------------------------------------------------------------------------

/// A case-fold map: upper-case LmCode byte -> lower-case LmCode byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fold {
    /// upper byte -> lower byte (`None` = byte is not an upper-case letter)
    fold: [Option<u8>; 256],
    pairs: usize,
}

impl Default for Fold {
    fn default() -> Self {
        Fold {
            fold: [None; 256],
            pairs: 0,
        }
    }
}

impl Fold {
    /// Extract the fold pairs from one section body of `lm.a`.
    ///
    /// The real format: CRLF-terminated lines `  <hi> <lo>` where each pair
    /// is two raw bytes >= 0x80 (the Ukrainian extras look like `f4 f5`).
    /// A line with one or zero high bytes is skipped. The first pair of a
    /// duplicated `upper` wins.
    pub fn parse_section(body: &[u8]) -> Self {
        let mut fold: [Option<u8>; 256] = [None; 256];
        let mut pairs = 0usize;
        let mut start = 0usize;
        for i in 0..=body.len() {
            let line_end = if i == body.len() { body.len() } else { i };
            if i == body.len() || body[i] == b'\n' {
                // collect the high bytes of this line
                let highs: Vec<u8> = body[start..line_end]
                    .iter()
                    .copied()
                    .filter(|&x| x >= 0x80)
                    .collect();
                if let (Some(upper), Some(lower)) = (highs.first().copied(), highs.get(1).copied()) {
                    let u = usize::from(upper);
                    if fold[u].is_none() {
                        fold[u] = Some(lower);
                        pairs += 1;
                    }
                }
                start = i + 1;
            }
        }
        Fold { fold, pairs }
    }

    /// Fold one byte (upper -> lower). Unknown bytes pass through unchanged.
    pub fn fold(&self, b: u8) -> u8 {
        self.fold[usize::from(b)].unwrap_or(b)
    }

    /// Number of fold pairs (the real `R:`/`U:` sections have 32 each).
    pub fn len(&self) -> usize {
        self.pairs
    }
}

/// The two case-fold tables from one `lm.a` file (`R` = Russian, `U` =
/// Ukrainian).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FoldTables {
    pub rus: Fold,
    pub ukr: Fold,
}

/// Parse the `R:` and `U:` sections of an `lm.a` file.
///
/// Section tags (`E:` `G:` `R:` `U:`) start a line; a section body runs up
/// to the next line-start tag (or EOF).
pub fn parse_lm_a(data: &[u8]) -> FoldTables {
    let tags: [&[u8]; 4] = [b"E:", b"G:", b"R:", b"U:"];
    // the end of a section body: the next line-start tag, or EOF
    let section_end = |from: usize| -> usize {
        let mut end = data.len();
        for t in tags {
            let mut pos = from;
            while let Some(p) = data[pos..].windows(t.len()).position(|w| w == t) {
                let abs = pos + p;
                if abs == 0 || data[abs - 1] == b'\n' {
                    if abs > from {
                        end = end.min(abs);
                    }
                    break;
                }
                pos = abs + 1;
            }
        }
        end
    };
    let parse_tag = |tag: &[u8]| -> Fold {
        let mut start = 0usize;
        while let Some(p) = data[start..].windows(tag.len()).position(|w| w == tag) {
            let abs = start + p;
            if abs == 0 || data[abs - 1] == b'\n' {
                let s = abs + tag.len();
                let e = section_end(s);
                return Fold::parse_section(&data[s..e]);
            }
            start = abs + 1;
        }
        Fold::default()
    };
    FoldTables {
        rus: parse_tag(b"R:"),
        ukr: parse_tag(b"U:"),
    }
}

// ---------------------------------------------------------------------------
// character -> LmCode byte (partial, documented)
// ---------------------------------------------------------------------------

/// The character -> LmCode-byte correspondence (lower-case bytes).
///
/// Confirmed from the `lm.a` `G:`-section anchors (`A a : 84 a4`, `O o :
/// 96 e6`, `U u : 9c ec`, `9f ef`): а/А = 0xA4/0x84, о/О = 0xE6/0x96,
/// у/У = 0xEC/0x9C, щ/Щ = 0xEF/0x9F. Inferred from dictionary-record
/// frequency (v0.14.0 validation): с/С = 0xAF (the most frequent letter
/// byte), і/ї/І/Ї/и = 0xAD. Latin letters and digits are plain ASCII.
///
/// This is *partial* — the full correspondence (the custom slot order of the
/// 0x80–0x9F range) is the v0.14.0 work. `tsearch` reports how much of a
/// query it could encode.
pub fn char_to_lm_byte(c: char) -> Option<u8> {
    match c {
        // confirmed anchors (lm.a G section)
        'а' | 'А' => Some(0xA4),
        'о' | 'О' => Some(0xE6),
        'у' | 'У' => Some(0xEC),
        'щ' | 'Щ' => Some(0xEF),
        // inferred from dictionary frequency (candidates, v0.14.0)
        'с' | 'С' => Some(0xAF),
        'і' | 'ї' | 'І' | 'Ї' | 'и' | 'И' => Some(0xAD),
        // digits
        '0'..='9' => Some((c as u8) - b'0' + 0x30),
        // Latin
        'a'..='z' | 'A'..='Z' => Some(c.to_ascii_lowercase() as u8),
        _ => None,
    }
}

/// Encode one query term into LmCode bytes. Returns the byte vector (the
/// encodable characters, in order) and how many of the term's characters
/// were encodable.
pub fn encode_term(term: &str, fold: &Fold) -> (Vec<u8>, u32, u32) {
    let chars: Vec<char> = term.chars().collect();
    let mut out: Vec<u8> = Vec::with_capacity(chars.len());
    let mut encodable = 0u32;
    for c in &chars {
        // fold to lower-case first, then map; fold the *byte* once mapped
        // (covers the extras, e.g. 0xF4 -> 0xF5)
        let lower = c.to_lowercase().next().unwrap_or(*c);
        if let Some(b) = char_to_lm_byte(lower) {
            out.push(fold.fold(b));
            encodable += 1;
        }
    }
    (out, encodable, chars.len() as u32)
}

// ---------------------------------------------------------------------------
// the index
// ---------------------------------------------------------------------------

/// One SOFT\ inventory row (name, size, role).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryFile {
    pub name: &'static str,
    pub role: &'static str,
    pub size: u64, // 0 = absent
}

/// A loaded Trident index (the decodable parts).
#[derive(Debug, Clone)]
pub struct TridentIndex {
    pub baza: PathBuf,
    pub kod: Kod,
    pub fold: FoldTables,
    /// dictionary words (LmCode byte vectors), sorted + de-duplicated
    pub dict_words: Vec<Vec<u8>>,
    /// the u16 posting stream from `.vt` (raw)
    pub vt_u16: Vec<u16>,
    pub inventory: Vec<InventoryFile>,
}

/// The result of an experimental `tsearch`.
#[derive(Debug, Clone, Default)]
pub struct TridentResult {
    /// native document ids read from `.vt` (experimental — the exact
    /// per-word boundary decode is v0.14.0)
    pub docs: Vec<u32>,
    /// the terms that were found in the dictionary
    pub matched: Vec<String>,
    /// fraction of the query that could be encoded (0.0..=1.0)
    pub coverage: f64,
    pub dict_size: usize,
    pub vt_size: usize,
}

/// The known SOFT\ inventory (16 files, incl. `windows.kod` + `lm.a`).
pub const INVENTORY: [(&str, &str); 16] = [
    ("lmu.voc", "словник (ukr)"),
    ("lmr.voc", "словник (rus)"),
    ("lmur.vt", "postings (ukr)"),
    ("lmru.vt", "postings (rus)"),
    ("lmur.m", "метадані (ukr)"),
    ("lmru.m", "метадані (rus)"),
    ("lmu.g", "граф (ukr)"),
    ("lmr.g", "граф (rus)"),
    ("lmu.gw", "граф-weight (ukr)"),
    ("lmr.gw", "граф-weight (rus)"),
    ("lmsup.dll", "рушій (16-біт)"),
    ("lmsup32.dll", "рушій (32-біт)"),
    ("windows.kod", "кодовий набір"),
    ("lm.a", "fold-таблиці (E/G/R/U)"),
    ("lmu.use", "usage-прапорці (ukr)"),
    ("lmr.use", "usage-прапорці (rus)"),
];

impl TridentIndex {
    /// Load the Trident index from a base's `SOFT\` folder.
    ///
    /// Returns `None` when the folder (or the core `.voc`/`.vt` files) is
    /// missing — the caller falls back to the built-in engine.
    pub fn open(baza: &Path) -> Option<Self> {
        let soft = baza.join("SOFT");
        if !soft.is_dir() {
            return None;
        }
        let read = |name: &str| -> Option<Vec<u8>> {
            std::fs::read(soft.join(name)).ok()
        };

        // core: dictionary + postings (Ukrainian is the primary base)
        let voc = read("lmu.voc")?;
        let vt = read("lmur.vt")?;

        let kod = read("windows.kod").as_deref().map(Kod::parse).unwrap_or_default();
        let fold = read("lm.a").as_deref().map(parse_lm_a).unwrap_or_default();

        let dict_words = extract_dict_words(&voc);
        let vt_u16 = read_vt_u16(&vt);

        let inventory: Vec<InventoryFile> = INVENTORY
            .iter()
            .map(|(name, role)| InventoryFile {
                name,
                role,
                size: soft.join(name).metadata().map(|m| m.len()).unwrap_or(0),
            })
            .collect();

        Some(TridentIndex {
            baza: baza.to_path_buf(),
            kod,
            fold,
            dict_words,
            vt_u16,
            inventory,
        })
    }

    /// True when the base has a usable SOFT\ index.
    pub fn is_available(baza: &Path) -> bool {
        let soft = baza.join("SOFT");
        soft.join("lmu.voc").is_file() && soft.join("lmur.vt").is_file()
    }

    /// Experimental native search: encode the query terms, look them up in
    /// the dictionary (prefix-style, with suffix trimming), read a posting
    /// slice from `.vt`.
    pub fn search(&self, query: &str) -> TridentResult {
        let mut matched: Vec<String> = Vec::new();
        let mut docs: Vec<u32> = Vec::new();
        let mut total_chars = 0u32;
        let mut encodable = 0u32;

        for raw in query.split_whitespace() {
            let (bytes, e, total) = encode_term(raw, &self.fold.ukr);
            total_chars += total;
            encodable += e;
            if bytes.is_empty() {
                continue;
            }
            // candidate byte vectors: the whole term + suffix-trimmed
            // variants (drop 1, 2, 3 trailing bytes — stem-style matching).
            // A candidate matches a dictionary word it is a *prefix* of.
            let mut found = false;
            for k in 0..=3u32 {
                if k as usize >= bytes.len() {
                    break;
                }
                let cand: Vec<u8> =
                    bytes.iter().take(bytes.len() - k as usize).copied().collect();
                if cand.is_empty() {
                    continue;
                }
                if let Some(idx) = self.find_prefix(&cand) {
                    found = true;
                    // posting slice (experimental proportional heuristic)
                    docs.extend(self.posting_slice(idx));
                    break;
                }
            }
            if found {
                matched.push(raw.to_string());
            }
        }

        docs.sort();
        docs.dedup();
        let coverage = if total_chars == 0 {
            0.0
        } else {
            encodable as f64 / total_chars as f64
        };
        TridentResult {
            docs,
            matched,
            coverage,
            dict_size: self.dict_words.len(),
            vt_size: self.vt_u16.len(),
        }
    }

    /// Find the first dictionary word that starts with `prefix` (the
    /// dictionary is sorted bytewise, so a binary search for the first
    /// word >= prefix, then a prefix scan).
    fn find_prefix(&self, prefix: &[u8]) -> Option<usize> {
        let mut lo = 0usize;
        let mut hi = self.dict_words.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            // bytewise: is dict_words[mid] < prefix? (a shorter prefix
            // equal to the word's head means the word is NOT below)
            let w = &self.dict_words[mid];
            let below = w
                .iter()
                .zip(prefix.iter())
                .find(|(a, b)| a != b)
                .map(|(a, b)| a < b)
                .unwrap_or(w.len() < prefix.len());
            if below {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.dict_words.len() && self.dict_words[lo].starts_with(prefix) {
            Some(lo)
        } else {
            None
        }
    }

    /// Read a posting slice for a dictionary word index (experimental
    /// proportional heuristic — the exact per-word boundaries come from the
    /// `.m`/`.voc` index in v0.14.0).
    fn posting_slice(&self, word_idx: usize) -> Vec<u32> {
        if self.vt_u16.is_empty() || self.dict_words.is_empty() {
            return Vec::new();
        }
        let n = self.vt_u16.len();
        let per = n.div_ceil(self.dict_words.len());
        let start = word_idx * per;
        let end = (start + per).min(n);
        // keep plausible doc ids (the base has < 1M docs; drop 0 as a
        // terminator marker)
        self.vt_u16[start..end]
            .iter()
            .filter(|&&d| d != 0)
            .map(|&d| u32::from(d))
            .collect()
    }
}

/// Extract the dictionary records `[0x84][word][00 00]` from a `.voc` file.
///
/// The records are interleaved with B-tree pages, so this is a scan: at each
/// offset that starts a valid record, record it and jump past it; otherwise
/// advance one byte. The result is de-duplicated and sorted.
pub fn extract_dict_words(voc: &[u8]) -> Vec<Vec<u8>> {
    let mut out: Vec<Vec<u8>> = Vec::new();
    let mut p = 0x80usize;
    while p + 2 < voc.len() {
        if voc[p] == 0x84 {
            let mut w = 1usize;
            while p + w < voc.len() && voc[p + w] != 0x00 {
                w += 1;
            }
            if (1..=6).contains(&w) && voc[p + w] == 0x00 && voc[p + w + 1] == 0x00 {
                out.push(voc[p + 1..p + w].to_vec());
                p += w + 2;
                continue;
            }
        }
        p += 1;
    }
    out.sort();
    out.dedup();
    out
}

/// Read the u16 posting stream from a `.vt` file (data starts at 0x40).
pub fn read_vt_u16(vt: &[u8]) -> Vec<u16> {
    let start = 0x40.min(vt.len());
    let n = (vt.len() - start) / 2;
    (0..n)
        .map(|i| u16::from_le_bytes([vt[start + i * 2], vt[start + i * 2 + 1]]))
        .collect()
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kod_parses_raw_and_hex_values() {
        // real windows.kod line shapes: raw byte >= 0x80 (optional trailing
        // space) and two ascii hex digits < 0x80
        let data = b"c0 = \x80 \r\n60 > 27\r\nab = 18\r\na6 = 7c\r\n";
        let k = Kod::parse(data);
        assert_eq!(k.code_to_byte(0xC0), Some(0x80));
        assert_eq!(k.code_to_byte(0x60), Some(0x27));
        assert_eq!(k.code_to_byte(0xAB), Some(0x18));
        assert_eq!(k.code_to_byte(0xA6), Some(0x7C));
        assert_eq!(k.code_to_byte(0xC1), None);
        assert_eq!(k.lines(), 4);
    }

    #[test]
    fn kod_parses_the_real_table_shape() {
        // c-block and f-block lines (raw byte >= 0x80, some with trailing sp)
        let data = b"c4 = \x84 \r\ne4 = \xa4 \r\nf6 = \xe6 \r\n";
        let k = Kod::parse(data);
        assert_eq!(k.code_to_byte(0xC4), Some(0x84)); // А
        assert_eq!(k.code_to_byte(0xE4), Some(0xA4)); // а
        assert_eq!(k.code_to_byte(0xF6), Some(0xE6)); // о
    }

    #[test]
    fn kod_rejects_malformed_lines() {
        let data = b"zz = \x80 \r\nx1 = 80 \r\n";
        let k = Kod::parse(data);
        assert_eq!(k.len(), 0);
    }

    #[test]
    fn fold_section_parses_pairs_line_based() {
        // real lm.a R/U section line shape: "  <hi> <lo>\r\n"
        let body = b"  \x80 \xa0\r\n  \x96 \xe6\r\n  \xf4 \xf5\r\n;";
        let f = Fold::parse_section(body);
        assert_eq!(f.fold(0x80), 0xA0);
        assert_eq!(f.fold(0x96), 0xE6);
        assert_eq!(f.fold(0xF4), 0xF5);
        // unknown / lower-case bytes pass through
        assert_eq!(f.fold(0x30), 0x30);
        assert_eq!(f.fold(0xE6), 0xE6);
        assert_eq!(f.len(), 3);
    }

    #[test]
    fn parse_lm_a_splits_r_u_sections() {
        // a minimal lm.a with all four sections (real line shapes)
        let data = b"E:\r\n  A a\r\n;\r\nG:\r\n;\r\nR:\r\n  \x84 \xa4\r\n  \x96 \xe6\r\n;\r\nU:\r\n  \x9c \xec\r\n  \xf6 \xf7\r\n;\r\n";
        let ft = parse_lm_a(data);
        assert_eq!(ft.rus.fold(0x84), 0xA4);
        assert_eq!(ft.rus.fold(0x96), 0xE6);
        assert_eq!(ft.rus.len(), 2);
        assert_eq!(ft.ukr.fold(0x9C), 0xEC);
        assert_eq!(ft.ukr.fold(0xF6), 0xF7);
        assert_eq!(ft.ukr.len(), 2);
    }

    #[test]
    fn char_map_is_case_insensitive_and_partial() {
        assert_eq!(char_to_lm_byte('а'), Some(0xA4));
        assert_eq!(char_to_lm_byte('А'), Some(0xA4));
        assert_eq!(char_to_lm_byte('О'), Some(0xE6));
        assert_eq!(char_to_lm_byte('с'), Some(0xAF)); // inferred
        assert_eq!(char_to_lm_byte('0'), Some(0x30));
        assert_eq!(char_to_lm_byte('к'), None); // not (yet) in the table
    }

    #[test]
    fn encode_term_reports_coverage() {
        let ft = parse_lm_a(b"R:\r\n;\r\nU:\r\n;\r\n");
        // "са0": every character encodable -> full coverage
        let (b, e, t) = encode_term("са0", &ft.ukr);
        assert_eq!((e, t), (3, 3));
        assert_eq!(b, vec![0xAF, 0xA4, 0x30]);
        // "конституція": only о/с/и/у/і encodable -> partial
        let (b2, e2, t2) = encode_term("конституція", &ft.ukr);
        assert_eq!((e2, t2), (5, 11));
        assert_eq!(b2, vec![0xE6, 0xAF, 0xAD, 0xEC, 0xAD]);
    }

    #[test]
    fn dict_words_are_sorted_and_deduped() {
        // synthetic: two identical records + one different
        let mut voc = vec![0u8; 0x100];
        voc[0x84] = 0x84; // a record at 0x84: [84][af 33][00 00]
        voc[0x85] = 0xAF;
        voc[0x86] = 0x33;
        voc[0x87] = 0x00;
        voc[0x88] = 0x00;
        // a duplicate later
        voc[0x90] = 0x84;
        voc[0x91] = 0xAF;
        voc[0x92] = 0x33;
        voc[0x93] = 0x00;
        voc[0x94] = 0x00;
        let words = extract_dict_words(&voc);
        assert_eq!(words, vec![vec![0xAF, 0x33]]);
    }

    #[test]
    fn read_vt_u16_reads_from_0x40() {
        let mut vt = vec![0u8; 0x50];
        vt[0x40] = 0x2A; // 0x122A little-endian
        vt[0x41] = 0x12;
        vt[0x42] = 0x34; // 0x5634
        vt[0x43] = 0x56;
        let u = read_vt_u16(&vt);
        // the whole 0x40..0x50 stream = 8 u16 (the unset tail is zeros)
        assert_eq!(u, vec![0x122A, 0x5634, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn search_finds_prefix_and_trims_suffixes() {
        let mut idx = TridentIndex {
            baza: PathBuf::from("x"),
            kod: Kod::default(),
            fold: FoldTables::default(),
            dict_words: vec![vec![0xAF, 0xA4, 0x30]], // "са0"
            vt_u16: vec![1, 2, 0, 3, 4, 5, 6, 7],
            inventory: Vec::new(),
        };
        let r = idx.search("а0"); // encodes to [0xA4, 0x30] — no match
        assert!(r.matched.is_empty());
        assert!(r.docs.is_empty());
        let r = idx.search("са0"); // exact word
        assert_eq!(r.matched, vec!["са0".to_string()]);
        assert!(!r.docs.is_empty());
        assert_eq!(r.coverage, 1.0);
    }

    /// Integration: load the real index (skipped when the base is absent).
    #[test]
    fn real_index_loads() {
        let baza = Path::new(r"C:\Zakon.Local\Baza");
        if !TridentIndex::is_available(baza) {
            eprintln!("test result: ok (skipped — no SOFT index)");
            return;
        }
        let idx = TridentIndex::open(baza).expect("index should load");
        assert!(idx.kod.len() > 50, "windows.kod should have many codes");
        assert_eq!(idx.fold.rus.len(), 32, "real R section has 32 pairs");
        assert_eq!(idx.fold.ukr.len(), 32, "real U section has 32 pairs");
        assert!(!idx.dict_words.is_empty(), "dict should have words");
        assert!(!idx.vt_u16.is_empty(), "vt should have a u16 stream");
        // the inventory must list 16 files, all present in this base
        assert_eq!(idx.inventory.len(), 16);
        assert!(idx.inventory.iter().all(|f| f.size > 0));
    }
}
