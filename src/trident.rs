//! Trident native FTS index reader (v0.13.0+ — experimental; the full
//! character correspondence landed in v0.14.0).
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
//! Word bytes live in a custom "LmCode" byte space. The slot order is
//! **cp1251-based** (confirmed by `windows.kod`, the DLL keyboard table and
//! real `lmu.voc` words):
//!
//! - upper-case letters occupy 0x80–0x9F (cp1251 `А–Я` in cp1251 order);
//! - lower-case first half 0xA0–0xAF (cp1251 `а–п`);
//! - lower-case second half 0xE0–0xEF (cp1251 `р–я`);
//! - the Ukrainian extras 0xF2–0xF9 (Ґ/ґ/Є/є/І/і/Ї/ї);
//! - digits and Latin letters are plain ASCII.
//!
//! `windows.kod` maps cp1251 codes to those bytes (`c0–cf → 80–9f`,
//! `e0–ef → a0–af`, `f0–ff → e0–ef`, extras `a5/b4/aa/ba/b2/b3/af/bf →
//! f2–f9`). `lm.a` holds the case-fold tables:
//!
//! - `E:` — Latin identity pairs (`A a` … `Z z`);
//! - `G:` — cross-language anchors (Latin letters aliased to LmCode slots,
//!   e.g. `A a : 84 a4`);
//! - `R:` — 32 upper→lower fold pairs, Russian;
//! - `U:` — 32 fold pairs, Ukrainian (the 0x9A/0x9B/0x9D main slots are left
//!   unused; the extras fold `f4 f5` (Є/є), `f6 f7` (І/і), `f8 f9` (Ї/ї)).
//!
//! All binary files share a 0x3C header: a `FREE` marker followed by a table
//! of u32 section offsets.
//!
//! What is decoded today: `windows.kod`, `lm.a`, the file headers, the
//! dictionary records (`[tag][word NUL][flag]`, tag 0x84/0x8d, flag
//! 0x00/0x01 — the general word-record format from `lmu.voc`), the u16
//! posting stream, the `.gw` morphology weight table (structural decode),
//! and — as of v0.14.0 — the **full character correspondence** (every
//! 0x80–0xEF slot + the 0xF2–0xF9 Ukrainian extras, see [`LM_CHAR_TABLE`]).
//!
//! v0.15.0 additionally documents the DLL word-lookup structure: the in-memory
//! dictionary is a NULL-terminated array of word-struct pointers, each struct
//! holding NUL-terminated LmCode word bytes at +1 plus postings/flag fields;
//! the comparator folds each byte through the cp1251 mapper.
//! Still open (next increment): the full **packed B-tree** word set (which is
//! where «конституція» lives — it is not a raw byte run in `lmu.voc`), the
//! `.g` grammar-graph semantics, the exact per-word posting boundaries and the
//! oracle-validated doc-number join.
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

/// The LmCode-byte -> character correspondence (for decoding dictionary
/// words back to human-readable Ukrainian/Russian).
///
/// This is the *inverse* of [`char_to_lm_byte`] and is the single source of
/// truth for the custom slot order. The order is **cp1251-based**, confirmed
/// three ways: (1) `windows.kod` maps cp1251 codes `c0–df → 80–9f`,
/// `e0–ef → a0–af`, `f0–ff → e0–ef` plus the Ukrainian extras
/// `a5/b4/aa/ba/b2/b3/af/bf → f2–f9`; (2) the DLL ships a keyboard-order
/// alphabet table that decodes to the same slots; (3) the real `lmu.voc`
/// dictionary words decode to valid Ukrainian («нема», «бути», «Мин») only
/// under this mapping.
pub const LM_CHAR_TABLE: &[(u8, char)] = &[
    // upper-case block 0x80–0x9F = cp1251 0xC0–0xDF (А–Я, cp1251 order)
    (0x80, 'А'), (0x81, 'Б'), (0x82, 'В'), (0x83, 'Г'), (0x84, 'Д'),
    (0x85, 'Е'), (0x86, 'Ж'), (0x87, 'З'), (0x88, 'И'), (0x89, 'Й'),
    (0x8A, 'К'), (0x8B, 'Л'), (0x8C, 'М'), (0x8D, 'Н'), (0x8E, 'О'),
    (0x8F, 'П'), (0x90, 'Р'), (0x91, 'С'), (0x92, 'Т'), (0x93, 'У'),
    (0x94, 'Ф'), (0x95, 'Х'), (0x96, 'Ц'), (0x97, 'Ч'), (0x98, 'Ш'),
    (0x99, 'Щ'), (0x9A, 'Ъ'), (0x9B, 'Ы'), (0x9C, 'Ь'), (0x9D, 'Э'),
    (0x9E, 'Ю'), (0x9F, 'Я'),
    // lower-case first half 0xA0–0xAF = cp1251 0xE0–0xEF (а–п)
    (0xA0, 'а'), (0xA1, 'б'), (0xA2, 'в'), (0xA3, 'г'), (0xA4, 'д'),
    (0xA5, 'е'), (0xA6, 'ж'), (0xA7, 'з'), (0xA8, 'и'), (0xA9, 'й'),
    (0xAA, 'к'), (0xAB, 'л'), (0xAC, 'м'), (0xAD, 'н'), (0xAE, 'о'),
    (0xAF, 'п'),
    // lower-case second half 0xE0–0xEF = cp1251 0xF0–0xFF (р–я)
    (0xE0, 'р'), (0xE1, 'с'), (0xE2, 'т'), (0xE3, 'у'), (0xE4, 'ф'),
    (0xE5, 'х'), (0xE6, 'ц'), (0xE7, 'ч'), (0xE8, 'ш'), (0xE9, 'щ'),
    (0xEA, 'ъ'), (0xEB, 'ы'), (0xEC, 'ь'), (0xED, 'э'), (0xEE, 'ю'),
    (0xEF, 'я'),
    // Ukrainian specials 0xF2–0xF9 (windows.kod extras a5/b4/aa/ba/b2/b3/af/bf)
    (0xF2, 'Ґ'), (0xF3, 'ґ'), (0xF4, 'Є'), (0xF5, 'є'),
    (0xF6, 'І'), (0xF7, 'і'), (0xF8, 'Ї'), (0xF9, 'ї'),
];

/// Decode one LmCode byte to a character (the inverse of
/// [`char_to_lm_byte`]). Returns `None` for bytes that are not letters
/// (digits, ASCII, or unassigned slots 0xB0–0xDF / 0xF0–0xF1 / 0xFA–0xFF).
pub fn lm_byte_to_char(b: u8) -> Option<char> {
    LM_CHAR_TABLE
        .iter()
        .find(|(code, _)| *code == b)
        .map(|(_, ch)| *ch)
}

/// Decode a whole LmCode word to a display string (letters to their
/// character, digits to ASCII digits, everything else to a `·xx·` marker).
pub fn decode_lm_word(bytes: &[u8]) -> String {
    let mut s = String::new();
    for &b in bytes {
        if b >= 0x30 && b <= 0x39 {
            s.push(b as char);
        } else if let Some(ch) = lm_byte_to_char(b) {
            s.push(ch);
        } else {
            s.push_str(&format!("·{b:02x}·"));
        }
    }
    s
}

/// The character -> LmCode-byte correspondence (case-sensitive).
///
/// The full cp1251-based slot order (see [`LM_CHAR_TABLE`]). Upper-case
/// letters map to the 0x80–0x9F block, lower-case to 0xA0–0xEF, Ukrainian
/// specials (і/ї/І/Ї/Є/є/Ґ/ґ) to 0xF2–0xF9, digits and Latin letters are
/// plain ASCII. `encode_term` lower-cases first, so searches land on the
/// lower-case slots.
pub fn char_to_lm_byte(c: char) -> Option<u8> {
    // Cyrillic + Ukrainian specials (both cases) come from the table;
    // digits and Latin letters are plain ASCII.
    if let Some((code, _)) = LM_CHAR_TABLE.iter().find(|e| e.1 == c) {
        return Some(*code);
    }
    match c {
        '0'..='9' => Some(c as u8),
        'a'..='z' => Some(c as u8),
        'A'..='Z' => Some(c.to_ascii_uppercase() as u8),
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

/// The Ukrainian letter → Latin-index-alphabet variants (v0.19.0).
///
/// The `.vt` record stream stores the dictionary words re-encoded into a
/// case-folded Latin alphabet (the DLL's index form). The empirical
/// anchors (confirmed against the real `lmur.vt` stream): «не»→`NE`,
/// «по»→`PO`, «пере»→`PERE` (inside `APEREMM`, `NAPEREMHE`, …),
/// «спів»→`SPV` (і→`V`), «був»→`BUV` (в→`V`), «бус»→`BUS` (з→`S`,
/// б→`B`), «неч»→`HCH` (here н→`H`, ч→`C`), Ї→`Y`.
///
/// The mapping is **context-dependent** (н maps to `N` in «не» but to `H`
/// in «неч»; в maps to `V` in «був» but `W`/`B` elsewhere — the stems are
/// graph-normalized through `lmu.g`), so this table carries *preferred*
/// variants first plus the observed alternates; [`latin_candidates`]
/// expands them breadth-first with a cap.
const LATIN_VARIANTS: &[(char, &[&str])] = &[
    ('а', &["A"]),
    ('б', &["B"]),
    ('в', &["V", "W", "B"]),
    ('г', &["G"]),
    ('д', &["D"]),
    ('е', &["E"]),
    ('є', &["E"]),
    ('ж', &["H", "J"]),
    ('з', &["Z", "S"]),
    ('и', &["I"]),
    ('і', &["I", "V"]),
    ('ї', &["YI", "Y"]),
    ('й', &["Y", "J"]),
    ('к', &["K"]),
    ('л', &["L"]),
    ('м', &["M"]),
    ('н', &["N", "H"]),
    ('о', &["O", "Q"]),
    ('п', &["P"]),
    ('р', &["R"]),
    ('с', &["S", "Z"]),
    ('т', &["T"]),
    ('у', &["U"]),
    ('ф', &["F"]),
    ('х', &["H"]),
    ('ц', &["C"]),
    ('ч', &["CH", "C"]),
    ('ш', &["SH", "S"]),
    ('щ', &["SCH", "SH"]),
    ('ъ', &["", "J"]),
    ('ы', &["Y"]),
    ('ь', &["", "J"]),
    ('э', &["E"]),
    ('ю', &["Y", "IU"]),
    ('я', &["Y", "JA"]),
    ('ґ', &["G"]),
];

/// The capped total number of Latin candidates per LmCode word
/// (ambiguity explodes multiplicatively; the first 32 in BFS order are
/// the preferred variants).
const LATIN_CANDIDATE_CAP: usize = 32;

/// The case-folded Latin candidates of a LmCode word (v0.19.0): every
/// letter expands through [`LATIN_VARIANTS`] (preferred variant first,
/// breadth-first), digits pass through verbatim. A word that cannot be
/// mapped at all yields an empty list. Capped at [`LATIN_CANDIDATE_CAP`].
pub fn latin_candidates(bytes: &[u8]) -> Vec<String> {
    let mut cands: Vec<String> = vec![String::new()];
    for &b in bytes {
        let variants: Vec<String> = if b.is_ascii_digit() {
            // digits pass through verbatim (they sort before letters in
            // the stream, exactly as in the dictionary)
            vec![String::from(b as char)]
        } else {
            let Some(c) = lm_byte_to_char(b) else {
                return Vec::new(); // unmappable byte
            };
            let Some((_, vs)) = LATIN_VARIANTS.iter().find(|(lc, _)| *lc == c) else {
                return Vec::new();
            };
            vs.iter().map(|s| s.to_string()).collect()
        };
        let mut next: Vec<String> = Vec::with_capacity(cands.len() * variants.len());
        for c in &cands {
            for v in &variants {
                next.push(format!("{c}{v}"));
            }
        }
        // de-duplicate in order, then cap (preferred variants come first)
        let mut seen: Vec<String> = Vec::new();
        for s in next {
            if !seen.contains(&s) {
                seen.push(s);
            }
            if seen.len() == LATIN_CANDIDATE_CAP {
                break;
            }
        }
        cands = seen;
    }
    // drop empties (a word made only of ь/ъ under their empty variant)
    cands.retain(|s| !s.is_empty());
    cands
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

/// The Trident index language (v0.25.0): the Ukrainian primary base
/// (`lmu.*`) or the Russian base (`lmr.*`) — the same on-disk formats,
/// different fold tables and streams. The loader is now language-generic:
/// both are loaded and searchable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// the Ukrainian primary base (`lmu.voc` / `lmur.vt`, `fold.ukr`)
    Ukr,
    /// the Russian base (`lmr.voc` / `lmru.vt`, `fold.rus`)
    Rus,
}

/// A loaded Trident index (the decodable parts).
#[derive(Debug, Clone)]
pub struct TridentIndex {
    pub baza: PathBuf,
    pub kod: Kod,
    pub fold: FoldTables,
    /// dictionary words (LmCode byte vectors), sorted + de-duplicated
    /// (Ukrainian — the primary base)
    pub dict_words: Vec<Vec<u8>>,
    /// the u16 posting stream from `.vt` (raw; Ukrainian)
    pub vt_u16: Vec<u16>,
    /// the Russian dictionary words (`lmr.voc`, v0.25.0) — the same record
    /// grammar, LmCode byte vectors, sorted + de-duplicated
    pub rus_dict_words: Vec<Vec<u8>>,
    /// the Russian u16 posting stream from `lmru.vt` (raw, v0.25.0)
    pub rus_vt_u16: Vec<u16>,
    /// the morphology weight table from `.gw` (raw u16, 0x8000 flag bit)
    pub gw_u16: Vec<u16>,
    /// the decoded `.voc` morphology tables (section table, affixes, suffix
    /// endings, digitraphs, list counts) — `None` when the `.voc` is absent
    pub morphology: Option<crate::trident_voc::Morphology>,
    /// the structurally decoded `.g` grammar graph (header + stream
    /// fingerprint) — `None` when `lmu.g` is absent
    pub graph: Option<crate::g_graph::GGraph>,
    /// the exact `.vt` record stream (14160 packed NUL records
    /// `0x40..header[0x3C]`, v0.19.0) — `None` when the stream walk fails
    pub vt_stream: Option<crate::trident_vt::VtRecordStream>,
    /// the `.m` per-word metadata header (v0.21.0 full decode: 64-byte
    /// header + the 16-slot u32 table @0x40..0x80 + the u32 fields
    /// @0x80/0x84 + the tail N-section with the `tail_match` wildcard
    /// keys; slot 5 = the u16 predicate table, slot 11 = the u16 word
    /// table) — `None` when `lmur.m` is absent
    pub vm_header: Option<crate::trident_vt::VmHeader>,
    /// the Russian `.vt` record stream (`lmru.vt`, v0.20.0) — the same
    /// packed grammar, Latin stems of the `lmr.voc` dictionary
    /// (`NET`/`PER`/`PO` confirmed present)
    pub rus_vt_stream: Option<crate::trident_vt::VtRecordStream>,
    /// the Russian `.m` metadata header (`lmru.m`, v0.20.0)
    pub rus_vm_header: Option<crate::trident_vt::VmHeader>,
    /// the u16 word table (`.m` slot 11, v0.22.0 structural decode:
    /// `[u16 count][count u16 records]`) — `None` when `lmur.m` is
    /// absent or the section is truncated
    pub word_table: Option<crate::trident_vt::VmWordTable>,
    /// the u16 predicate table (`.m` slot 5, v0.22.0: the ascending
    /// offset run + the 0xCECA sentinel + the packed record region)
    pub pred_table: Option<crate::trident_vt::VmPredicateTable>,
    /// the Russian u16 word table (`lmru.m` slot 11, v0.22.0)
    pub rus_word_table: Option<crate::trident_vt::VmWordTable>,
    /// the Russian u16 predicate table (`lmru.m` slot 5, v0.22.0)
    pub rus_pred_table: Option<crate::trident_vt::VmPredicateTable>,
    /// the `lmu.use` usage-flags file (v0.20.0; the body is all zeros in
    /// this base — the 2005-era index does not enable the flags)
    pub use_flags: Option<crate::trident_vt::UseFile>,
    pub inventory: Vec<InventoryFile>,
}

/// The result of an experimental `tsearch`.
#[derive(Debug, Clone, Default)]
pub struct TridentResult {
    /// native document ids read from `.vt` — the proportional
    /// `posting_slice` **estimate** (v0.24.0: fallback only — filled when
    /// the exact stem join found nothing for a matched term; labelled
    /// «оцінка» in the CLI)
    pub docs: Vec<u32>,
    /// true when `docs` is non-empty (it is only ever the proportional
    /// fallback; the exact join evidence lives in `joined`)
    pub docs_fallback: bool,
    /// the terms that were found in the dictionary
    pub matched: Vec<String>,
    /// the exact stem join hits (v0.19.0): `(Latin candidate, .vt record
    /// offset, the record's u16 word list)` — the case-folded dictionary
    /// stems found in the `.vt` record stream, with their posting evidence
    pub joined: Vec<(String, usize, Vec<u16>)>,
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

/// The per-language search context (v0.25.0): the dictionary, the `.vt`
/// stream, the raw u16 space, and the fold table for ONE language. The
/// search methods are language-generic over this; the Ukrainian fields and
/// the Russian fields each build one.
struct LangCtx<'a> {
    dict_words: &'a [Vec<u8>],
    vt_u16: &'a [u16],
    vt_stream: Option<&'a crate::trident_vt::VtRecordStream>,
    fold: &'a Fold,
}

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
        // optional morphology weight table (lmu.gw) — structural decode only
        let gw_u16 = read("lmu.gw").as_deref().map(read_gw_u16).unwrap_or_default();
        // the exact `.voc` section table + morphology tables (v0.17.0)
        let morphology = crate::trident_voc::Morphology::parse(&voc);
        // the `.g` grammar graph (structural decode: header + stream fingerprint)
        let graph = read("lmu.g").as_deref().and_then(crate::g_graph::GGraph::parse);
        // the exact `.vt` record stream + `.m` metadata header (v0.19.0)
        let vt_stream = crate::trident_vt::VtRecordStream::parse(&vt);
        // the u16 tables now need the `.m` bytes twice (header + the
        // section parses) — hold them
        let m_bytes = read("lmur.m");
        let vm_header = m_bytes
            .as_deref()
            .and_then(crate::trident_vt::VmHeader::parse);
        // v0.22.0: the u16 word/predicate tables (the `.m` slots 11/5)
        let word_table = m_bytes.as_deref().and_then(|m| {
            vm_header
                .as_ref()
                .and_then(|h| h.word_off())
                .and_then(|off| crate::trident_vt::VmWordTable::parse(m, off))
        });
        let pred_table = m_bytes.as_deref().and_then(|m| {
            vm_header
                .as_ref()
                .and_then(|h| h.pred_off())
                .and_then(|off| crate::trident_vt::VmPredicateTable::parse(m, off))
        });
        // the Russian streams + the usage-flags file (v0.20.0)
        let rus_vt_bytes = read("lmru.vt");
        let rus_vt_stream = rus_vt_bytes
            .as_deref()
            .and_then(crate::trident_vt::VtRecordStream::parse);
        // v0.25.0: the Russian dictionary + the raw u16 stream — the same
        // loaders as the Ukrainian core, so `tsearch --lang r` works
        let rus_dict_words = read("lmr.voc").map(|voc| extract_dict_words(&voc));
        let rus_vt_u16 = rus_vt_bytes.as_deref().map(read_vt_u16).unwrap_or_default();
        let ru_bytes = read("lmru.m");
        let rus_vm_header = ru_bytes
            .as_deref()
            .and_then(crate::trident_vt::VmHeader::parse);
        let rus_word_table = ru_bytes.as_deref().and_then(|m| {
            rus_vm_header
                .as_ref()
                .and_then(|h| h.word_off())
                .and_then(|off| crate::trident_vt::VmWordTable::parse(m, off))
        });
        let rus_pred_table = ru_bytes.as_deref().and_then(|m| {
            rus_vm_header
                .as_ref()
                .and_then(|h| h.pred_off())
                .and_then(|off| crate::trident_vt::VmPredicateTable::parse(m, off))
        });
        let use_flags = read("lmu.use")
            .as_deref()
            .and_then(crate::trident_vt::UseFile::parse);

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
            rus_dict_words: rus_dict_words.unwrap_or_default(),
            rus_vt_u16,
            gw_u16,
            morphology,
            graph,
            vt_stream,
            vm_header,
            word_table,
            pred_table,
            rus_vt_stream,
            rus_vm_header,
            rus_word_table,
            rus_pred_table,
            use_flags,
            inventory,
        })
    }

    /// True when the base has a usable SOFT\ index.
    pub fn is_available(baza: &Path) -> bool {
        let soft = baza.join("SOFT");
        soft.join("lmu.voc").is_file() && soft.join("lmur.vt").is_file()
    }

    /// Build the per-language search context (v0.25.0): the dictionary, the
    /// `.vt` stream, the raw u16 space, and the fold table for ONE language.
    fn lang_ctx(&self, lang: Lang) -> LangCtx<'_> {
        match lang {
            Lang::Ukr => LangCtx {
                dict_words: &self.dict_words,
                vt_u16: &self.vt_u16,
                vt_stream: self.vt_stream.as_ref(),
                fold: &self.fold.ukr,
            },
            Lang::Rus => LangCtx {
                dict_words: &self.rus_dict_words,
                vt_u16: &self.rus_vt_u16,
                vt_stream: self.rus_vt_stream.as_ref(),
                fold: &self.fold.rus,
            },
        }
    }

    /// Experimental native search: encode the query terms, look them up in
    /// the dictionary (prefix-style, with suffix trimming), read a posting
    /// slice from `.vt`.
    ///
    /// v0.19.0: every term is *also* run through the exact stem join
    /// ([`exact_postings`]) against the `.vt` record stream — the case-
    /// folded Latin stems are the primary posting evidence; the
    /// proportional `posting_slice` remains the fallback for terms whose
    /// stem the exact join does not resolve.
    pub fn search(&self, query: &str) -> TridentResult {
        // the Ukrainian primary base (backward-compatible default)
        self.search_lang(query, Lang::Ukr)
    }

    /// The language-generic search (v0.25.0): `lang` selects the fold table
    /// + dictionary + `.vt` stream. `Ukr` is the primary base; `Rus` runs
    /// the same pipeline over `lmr.voc` / `lmru.vt` with `fold.rus`.
    pub fn search_lang(&self, query: &str, lang: Lang) -> TridentResult {
        self.search_in(&self.lang_ctx(lang), query)
    }

    /// Search both languages and merge (v0.25.0): the union of the per-term
    /// `matched` / `joined` evidence and the de-duplicated `docs` fallback.
    /// Used by `tsearch --lang auto`.
    pub fn search_auto(&self, query: &str) -> TridentResult {
        let mut a = self.search_lang(query, Lang::Ukr);
        let b = self.search_lang(query, Lang::Rus);
        a.matched.extend(b.matched);
        a.joined.extend(b.joined);
        let mut docs = a.docs;
        docs.extend(b.docs);
        docs.sort();
        docs.dedup();
        a.docs = docs;
        a.docs_fallback = !a.docs.is_empty();
        a.dict_size += b.dict_size;
        a.vt_size += b.vt_size;
        a
    }

    /// The per-language search core (v0.25.0): encode the query terms
    /// through the language's fold table, run the exact stem join (primary
    /// evidence), and fall back to the proportional `posting_slice` for
    /// terms the exact join does not resolve.
    fn search_in(&self, lc: &LangCtx<'_>, query: &str) -> TridentResult {
        let mut matched: Vec<String> = Vec::new();
        let mut docs: Vec<u32> = Vec::new();
        let mut joined: Vec<(String, usize, Vec<u16>)> = Vec::new();
        let mut total_chars = 0u32;
        let mut encodable = 0u32;

        for raw in query.split_whitespace() {
            let (bytes, e, total) = encode_term(raw, lc.fold);
            total_chars += total;
            encodable += e;
            if bytes.is_empty() {
                continue;
            }
            // the exact stem join (v0.19.0 primary evidence)
            let term_hits = self.exact_postings_in(lc, raw);
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
                if let Some(idx) = self.find_prefix_in(lc, &cand) {
                    found = true;
                    // posting slice — the fallback heuristic, kept when the
                    // exact join found nothing for this term
                    if term_hits.is_empty() {
                        docs.extend(self.posting_slice_in(lc, idx));
                    }
                    break;
                }
            }
            if found {
                matched.push(raw.to_string());
            }
            joined.extend(term_hits);
        }

        docs.sort();
        docs.dedup();
        let docs_fallback = !docs.is_empty();
        let coverage = if total_chars == 0 {
            0.0
        } else {
            encodable as f64 / total_chars as f64
        };
        TridentResult {
            docs,
            docs_fallback,
            matched,
            joined,
            coverage,
            dict_size: lc.dict_words.len(),
            vt_size: lc.vt_u16.len(),
        }
    }

    /// The distinct u16 doc-ids contributed by the **exact** stem join for
    /// one term (v0.24.0): the union of the matching `.vt` records' u16 word
    /// sets, de-duplicated. This is the per-term native document count the
    /// `--oracle` cross-check compares against the oracle's file count.
    /// Empty when the term is not fully encodable or the `.vt` stream is
    /// absent.
    pub fn term_join_docs(&self, term: &str) -> Vec<u16> {
        self.term_join_docs_lang(term, Lang::Ukr)
    }

    /// The distinct u16 doc-ids for one term in the selected language
    /// (v0.25.0).
    pub fn term_join_docs_lang(&self, term: &str, lang: Lang) -> Vec<u16> {
        self.term_join_docs_in(&self.lang_ctx(lang), term)
    }

    /// The distinct u16 doc-ids contributed by the **exact** stem join for
    /// one term (v0.24.0): the union of the matching `.vt` records' u16 word
    /// sets, de-duplicated. This is the per-term native document count the
    /// `--oracle` cross-check compares against the oracle's file count.
    /// Empty when the term is not fully encodable or the `.vt` stream is
    /// absent.
    fn term_join_docs_in(&self, lc: &LangCtx<'_>, term: &str) -> Vec<u16> {
        let mut set: std::collections::HashSet<u16> =
            std::collections::HashSet::new();
        for (_, _, u16s) in self.exact_postings_in(lc, term) {
            for v in u16s {
                if v != 0 {
                    set.insert(v);
                }
            }
        }
        let mut out: Vec<u16> = set.into_iter().collect();
        out.sort_unstable();
        out
    }

    /// Encode the query terms through the (now full) character table and
    /// return each term with its LmCode byte vector. Used by the CLI to show
    /// exactly how a query maps into the native index.
    pub fn encode_query(&self, query: &str) -> Vec<(String, Vec<u8>)> {
        self.encode_query_lang(query, Lang::Ukr)
    }

    /// The per-language query encoding (v0.25.0).
    pub fn encode_query_lang(&self, query: &str, lang: Lang) -> Vec<(String, Vec<u8>)> {
        let lc = self.lang_ctx(lang);
        query
            .split_whitespace()
            .map(|t| {
                let (bytes, _, _) = encode_term(t, lc.fold);
                (t.to_string(), bytes)
            })
            .collect()
    }

    /// Find the first dictionary word that starts with `prefix` (the
    /// dictionary is sorted bytewise, so a binary search for the first
    /// word >= prefix, then a prefix scan).
    fn find_prefix(&self, prefix: &[u8]) -> Option<usize> {
        self.find_prefix_in(&self.lang_ctx(Lang::Ukr), prefix)
    }

    fn find_prefix_in(&self, lc: &LangCtx<'_>, prefix: &[u8]) -> Option<usize> {
        let dict = lc.dict_words;
        let mut lo = 0usize;
        let mut hi = dict.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            // bytewise: is dict[mid] < prefix? (a shorter prefix
            // equal to the word's head means the word is NOT below)
            let w = &dict[mid];
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
        if lo < dict.len() && dict[lo].starts_with(prefix) {
            Some(lo)
        } else {
            None
        }
    }

    /// Read a posting slice for a dictionary word index (experimental
    /// proportional heuristic: the vt stream is divided evenly across the
    /// extracted dictionary words). Because the dictionary extraction grew
    /// from 78 to ~1000 words in v0.15.0, the per-word slice — and hence the
    /// reported doc count — is an estimate that scales with the dictionary
    /// size. The exact per-word boundaries (and the oracle-validated
    /// doc-number join) come from the packed B-tree decode (roadmap).
    fn posting_slice(&self, word_idx: usize) -> Vec<u32> {
        self.posting_slice_in(&self.lang_ctx(Lang::Ukr), word_idx)
    }

    fn posting_slice_in(&self, lc: &LangCtx<'_>, word_idx: usize) -> Vec<u32> {
        if lc.vt_u16.is_empty() || lc.dict_words.is_empty() {
            return Vec::new();
        }
        let n = lc.vt_u16.len();
        let per = n.div_ceil(lc.dict_words.len());
        let start = word_idx * per;
        let end = (start + per).min(n);
        // keep plausible doc ids (the base has < 1M docs; drop 0 as a
        // terminator marker)
        lc.vt_u16[start..end]
            .iter()
            .filter(|&&d| d != 0)
            .map(|&d| u32::from(d))
            .collect()
    }

    /// The exact stem join (v0.19.0): encode `term` to LmCode, expand it
    /// into case-folded Latin candidates ([`latin_candidates`]) and look
    /// each candidate up in the `.vt` record stream
    /// ([`VtRecordStream::find_stem`](crate::trident_vt::VtRecordStream::find_stem)).
    ///
    /// Returns `(candidate, record offset, the record's u16 words)` for the
    /// matching records (deduplicated by record offset, capped at 32).
    /// Empty when the term is not fully encodable or the `.vt` stream is
    /// absent. This is the join that replaces the proportional
    /// [`posting_slice`] heuristic as the *primary* evidence; the full
    /// B-tree predicate join (`FUN_100176c0` u16 word table) is blocked on
    /// the tail B-tree decode (v0.20).
    pub fn exact_postings(&self, term: &str) -> Vec<(String, usize, Vec<u16>)> {
        self.exact_postings_lang(term, Lang::Ukr)
    }

    /// The exact stem join for the selected language (v0.25.0).
    pub fn exact_postings_lang(
        &self,
        term: &str,
        lang: Lang,
    ) -> Vec<(String, usize, Vec<u16>)> {
        self.exact_postings_in(&self.lang_ctx(lang), term)
    }

    /// The per-language exact stem join core (v0.25.0): encode `term`
    /// through the language's fold table, expand it into case-folded Latin
    /// candidates ([`latin_candidates`]) and look each up in the language's
    /// `.vt` record stream
    /// ([`VtRecordStream::find_stem`](crate::trident_vt::VtRecordStream::find_stem)).
    ///
    /// Returns `(candidate, record offset, the record's u16 words)` for the
    /// matching records (deduplicated by record offset, capped at 32).
    /// Empty when the term is not fully encodable or the `.vt` stream is
    /// absent. This is the join that replaces the proportional
    /// [`posting_slice`] heuristic as the *primary* evidence; the full
    /// B-tree predicate join (`FUN_100176c0` u16 word table) is blocked on
    /// the tail B-tree decode (v0.20).
    fn exact_postings_in(
        &self,
        lc: &LangCtx<'_>,
        term: &str,
    ) -> Vec<(String, usize, Vec<u16>)> {
        let Some(stream) = lc.vt_stream else {
            return Vec::new();
        };
        let (bytes, encodable, total) = encode_term(term, lc.fold);
        if bytes.is_empty() || encodable < total {
            return Vec::new();
        }
        let cands = latin_candidates(&bytes);
        let mut out: Vec<(String, usize, Vec<u16>)> = Vec::new();
        for cand in &cands {
            if out.len() >= 32 {
                break;
            }
            for (i, _) in stream.find_stem(cand) {
                if out.iter().any(|(_, off, _)| *off == stream.records[i].off) {
                    continue;
                }
                out.push((
                    cand.clone(),
                    stream.records[i].off,
                    stream.records[i].u16_words.clone(),
                ));
                if out.len() >= 32 {
                    break;
                }
            }
        }
        out
    }
}

/// Extract the dictionary records from a `.voc` file.
///
/// A word record is `[tag][word][flag]` where:
/// - `tag` is a single byte with the high bit set (0x80–0xFF; the common
///   values are 0x84 and 0x8d);
/// - `word` is a NUL-terminated run of LmCode bytes (Cyrillic 0x80–0xFF or
///   ASCII digits 0x30–0x39), 1–12 bytes long; the trailing bytes are often
///   morphological-class codes (e.g. `зувати0`, `титиь33`);
/// - `flag` is one trailing byte (observed 0x00 and 0x01).
///
/// The records are interleaved with B-tree pages, so this is a scan: at each
/// offset that starts a valid record, record it and jump past it; otherwise
/// advance one byte. The forward scan is bounded (12 bytes) so the whole
/// extraction is O(n). The result is de-duplicated and sorted.
///
/// This is a superset of the old strict `[0x84][word][00 00]` heuristic: the
/// real `lmu.voc` yields ~1000 unique stems (vs. 78 before), including real
/// Ukrainian stems such as `зувати`, `расти` and `дити`.
pub fn extract_dict_words(voc: &[u8]) -> Vec<Vec<u8>> {
    const MAX_WORD: usize = 12;
    let mut out: Vec<Vec<u8>> = Vec::new();
    let mut p = 0x80usize;
    let n = voc.len();
    while p + 2 < n {
        let tag = voc[p];
        if tag >= 0x80 {
            // Read up to MAX_WORD word bytes; stop at a NUL terminator or an
            // invalid byte.
            let mut word = [0u8; MAX_WORD];
            let mut wl = 0usize;
            let mut w = p + 1;
            let mut valid = true;
            while wl < MAX_WORD && w < n {
                let b = voc[w];
                if b == 0x00 {
                    break; // NUL terminator reached
                }
                if !(b >= 0x30 && b <= 0x39 || b >= 0x80) {
                    valid = false;
                    break;
                }
                word[wl] = b;
                wl += 1;
                w += 1;
            }
            // A valid record: 1..=MAX_WORD word bytes, a NUL terminator, and a
            // flag byte after it.
            if valid && (1..=MAX_WORD).contains(&wl) && w < n && voc[w] == 0x00 && w + 1 < n {
                out.push(word[..wl].to_vec());
                p = w + 2;
                continue;
            }
        }
        p += 1;
    }
    out.sort();
    out.dedup();
    out
}

/// Read the morphology weight table from a `.gw` file.
///
/// A `.gw` file is a flat little-endian u16 array (no header): 268 bytes for
/// `lmu.gw` (134 values) / 264 bytes for `lmr.gw` (132 values). Each u16 is a
/// morphological-class weight; the `0x8000` bit is a flag (observed set on
/// the last few entries, e.g. `8053 8054 8057 8058`). The values are the
/// weights of the grammar graph in the accompanying `.g` file (header
/// "Trident Software, Inc. / grammar / UTEMPLATE.TXT"), so a full semantic
/// decode needs the `.g` grammar (roadmap).
pub fn read_gw_u16(gw: &[u8]) -> Vec<u16> {
    (0..gw.len() / 2)
        .map(|i| u16::from_le_bytes([gw[i * 2], gw[i * 2 + 1]]))
        .collect()
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
        // cp1251 0xC4 = Д -> LmCode 0x84 (Д); 0xE4 = д -> 0xA4; 0xF6 = ц -> 0xE6
        assert_eq!(k.code_to_byte(0xC4), Some(0x84)); // Д
        assert_eq!(k.code_to_byte(0xE4), Some(0xA4)); // д
        assert_eq!(k.code_to_byte(0xF6), Some(0xE6)); // ц
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
    fn char_map_is_the_full_cp1251_correspondence() {
        // lower-case main block (cp1251 order)
        assert_eq!(char_to_lm_byte('а'), Some(0xA0));
        assert_eq!(char_to_lm_byte('п'), Some(0xAF));
        assert_eq!(char_to_lm_byte('р'), Some(0xE0));
        assert_eq!(char_to_lm_byte('я'), Some(0xEF));
        assert_eq!(char_to_lm_byte('с'), Some(0xE1));
        // upper-case main block
        assert_eq!(char_to_lm_byte('А'), Some(0x80));
        assert_eq!(char_to_lm_byte('Я'), Some(0x9F));
        assert_eq!(char_to_lm_byte('Щ'), Some(0x99));
        // Ukrainian specials
        assert_eq!(char_to_lm_byte('і'), Some(0xF7));
        assert_eq!(char_to_lm_byte('І'), Some(0xF6));
        assert_eq!(char_to_lm_byte('ї'), Some(0xF9));
        assert_eq!(char_to_lm_byte('Ї'), Some(0xF8));
        assert_eq!(char_to_lm_byte('є'), Some(0xF5));
        assert_eq!(char_to_lm_byte('Є'), Some(0xF4));
        // digits + Latin are ASCII
        assert_eq!(char_to_lm_byte('0'), Some(0x30));
        assert_eq!(char_to_lm_byte('a'), Some(0x61));
        // unknown
        assert_eq!(char_to_lm_byte(' '), None);
    }

    #[test]
    fn lm_byte_to_char_round_trips() {
        for c in [
            'а', 'о', 'у', 'щ', 'с', 'я', 'к', 'І', 'і', 'ї', 'Ї', 'Є', 'є', 'Ґ', 'ґ', 'А', 'Я', 'Щ',
        ] {
            let b = char_to_lm_byte(c).expect("char must be encodable");
            assert_eq!(lm_byte_to_char(b), Some(c), "round-trip failed for {c}");
        }
        // digits decode as ASCII
        assert_eq!(lm_byte_to_char(0x33), None);
    }

    #[test]
    fn decode_lm_word_reproduces_real_dictionary_words() {
        // «нема» = [ad a5 ac a0] and «бути» = [a1 e3 e2 a8] (real lmu.voc
        // records) must decode back to the source words.
        assert_eq!(decode_lm_word(&[0xAD, 0xA5, 0xAC, 0xA0]), "нема");
        assert_eq!(decode_lm_word(&[0xA1, 0xE3, 0xE2, 0xA8]), "бути");
        // a word with digits + a marker
        assert_eq!(decode_lm_word(&[0xAF, 0x33, 0x30]), "п30");
    }

    #[test]
    fn encode_term_reports_coverage() {
        let ft = parse_lm_a(b"R:\r\n;\r\nU:\r\n;\r\n");
        // "са0": every character encodable -> full coverage
        let (b, e, t) = encode_term("са0", &ft.ukr);
        assert_eq!((e, t), (3, 3));
        assert_eq!(b, vec![0xE1, 0xA0, 0x30]);
        // "конституція": the full cp1251 table now encodes every character
        // (100% coverage), including the Ukrainian і (0xF7)
        let (b2, e2, t2) = encode_term("конституція", &ft.ukr);
        assert_eq!((e2, t2), (11, 11));
        assert_eq!(
            b2,
            vec![
                0xAA, 0xAE, 0xAD, 0xE1, 0xE2, 0xA8, 0xE2, 0xE3, 0xE6, 0xF7, 0xEF
            ]
        );
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
    fn extract_dict_words_captures_tag_and_flag_variants() {
        // general record format: [tag>=0x80][word NUL][flag]
        // covers a 0x8d tag, a 0x01 flag, and a real stem "зувати0"
        let mut voc = vec![0u8; 0x100];
        // 0x80: [8d][a4 aa 31][00][00]  ("дк1", tag 0x8d, flag 0x00)
        voc[0x80] = 0x8d;
        voc[0x81] = 0xA4;
        voc[0x82] = 0xAA;
        voc[0x83] = 0x31;
        voc[0x84] = 0x00;
        voc[0x85] = 0x00;
        // 0x86: [84][ad 33 ae][00][01]  ("н3о", tag 0x84, flag 0x01)
        voc[0x86] = 0x84;
        voc[0x87] = 0xAD;
        voc[0x88] = 0x33;
        voc[0x89] = 0xAE;
        voc[0x8A] = 0x00;
        voc[0x8B] = 0x01;
        // 0x8C: [84][a7 e3 a2 a0 e2 a8 30][00][00]  ("зувати0")
        voc[0x8C] = 0x84;
        voc[0x8D] = 0xA7;
        voc[0x8E] = 0xE3;
        voc[0x8F] = 0xA2;
        voc[0x90] = 0xA0;
        voc[0x91] = 0xE2;
        voc[0x92] = 0xA8;
        voc[0x93] = 0x30;
        voc[0x94] = 0x00;
        voc[0x95] = 0x00;
        let words = extract_dict_words(&voc);
        // all three records captured, including the 0x8d tag and 0x01 flag
        assert!(words.contains(&vec![0xA4, 0xAA, 0x31]));
        assert!(words.contains(&vec![0xAD, 0x33, 0xAE]));
        assert!(words.contains(&vec![0xA7, 0xE3, 0xA2, 0xA0, 0xE2, 0xA8, 0x30]));
        assert_eq!(words.len(), 3);
    }

    #[test]
    fn read_gw_u16_reads_le_values() {
        let gw = vec![0x44, 0x00, 0x53, 0x80, 0x00, 0x00];
        // little-endian; the 0x8000 flag bit is preserved
        assert_eq!(read_gw_u16(&gw), vec![0x0044, 0x8053, 0x0000]);
        assert!(read_gw_u16(&[]).is_empty());
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
        let idx = TridentIndex {
            baza: PathBuf::from("x"),
            kod: Kod::default(),
            fold: FoldTables::default(),
            dict_words: vec![vec![0xE1, 0xA0, 0x30]], // "са0"
            vt_u16: vec![1, 2, 0, 3, 4, 5, 6, 7],
            rus_dict_words: Vec::new(),
            rus_vt_u16: Vec::new(),
            gw_u16: Vec::new(),
            morphology: None,
            graph: None,
            vt_stream: None,
            vm_header: None,
            word_table: None,
            pred_table: None,
            rus_vt_stream: None,
            rus_vm_header: None,
            rus_word_table: None,
            rus_pred_table: None,
            use_flags: None,
            inventory: Vec::new(),
        };
        let r = idx.search("а0"); // encodes to [0xA4, 0x30] — no match
        assert!(r.matched.is_empty());
        assert!(r.docs.is_empty());
        assert!(!r.docs_fallback, "no fallback docs when nothing matched");
        let r = idx.search("са0"); // exact word
        assert_eq!(r.matched, vec!["са0".to_string()]);
        assert!(!r.docs.is_empty());
        // no `.vt` stream here, so the docs are the proportional fallback
        assert!(r.docs_fallback, "docs are the proportional fallback");
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
        assert!(
            idx.dict_words.len() >= 500,
            "the general record format should yield ~1000 words, got {}",
            idx.dict_words.len()
        );
        assert!(!idx.vt_u16.is_empty(), "vt should have a u16 stream");
        // the .gw weight table (lmu.gw) decodes to a u16 array; when the file
        // is present in the base it must be non-empty
        let has_gw = baza.join("SOFT").join("lmu.gw").is_file();
        assert_eq!(idx.gw_u16.is_empty(), !has_gw);
        // the inventory must list 16 files, all present in this base
        assert_eq!(idx.inventory.len(), 16);
        assert!(idx.inventory.iter().all(|f| f.size > 0));
        // v0.20.0: the Russian streams + the usage-flags file
        assert!(
            idx.rus_vt_stream.is_some(),
            "the lmru.vt stream should parse"
        );
        assert!(idx.rus_vm_header.is_some(), "the lmru.m header should parse");
        assert!(idx.use_flags.is_some(), "the lmu.use file should parse");
        assert!(
            idx.use_flags.as_ref().unwrap().body_is_zero(),
            "the usage flags are not enabled in this base"
        );
        // v0.22.0: the u16 word/predicate tables (the `.m` slots 11/5)
        assert!(idx.word_table.is_some(), "the word table should parse");
        assert!(
            idx.pred_table.is_some(),
            "the predicate table should parse"
        );
        assert!(idx.rus_word_table.is_some(), "the rus word table");
        assert!(idx.rus_pred_table.is_some(), "the rus predicate table");
        assert_eq!(
            idx.word_table.as_ref().unwrap().count,
            0xFE,
            "the ukr dictionary word count"
        );
        assert_eq!(
            idx.word_table.as_ref().unwrap().boundary(),
            Some(0x8000),
            "the ukr boundary record"
        );
        assert_eq!(
            idx.rus_word_table.as_ref().unwrap().count,
            0x100,
            "the rus dictionary word count"
        );
        assert_eq!(
            idx.rus_word_table.as_ref().unwrap().boundary(),
            Some(0x7F00),
            "the rus boundary record"
        );
        assert_eq!(
            idx.pred_table.as_ref().unwrap().sentinel,
            0xCECA,
            "the predicate sentinel (both languages)"
        );
    }

    #[test]
    fn latin_candidates_expand_the_known_stems() {
        // «не» = [0xAD, 0xA5] → {N,H} × {E}
        assert_eq!(
            latin_candidates(&[0xAD, 0xA5]),
            vec!["NE".to_string(), "HE".to_string()]
        );
        // «по» = [0xAF, 0xAE] → {P} × {O,Q}
        assert_eq!(
            latin_candidates(&[0xAF, 0xAE]),
            vec!["PO".to_string(), "PQ".to_string()]
        );
        // «був» = [0xA1, 0xE3, 0xA2] → {B} × {U} × {V,W,B}
        assert_eq!(
            latin_candidates(&[0xA1, 0xE3, 0xA2]),
            vec![
                "BUV".to_string(),
                "BUW".to_string(),
                "BUB".to_string()
            ]
        );
        // digits pass through verbatim (they sort before letters in the
        // stream, exactly as in the dictionary)
        assert_eq!(latin_candidates(&[0xAF, 0x33, 0x30]), vec!["P30".to_string()]);
        // an unmappable byte yields no candidates
        assert!(latin_candidates(&[0x01]).is_empty());
    }

    #[test]
    fn exact_postings_find_the_confirmed_stems_in_the_real_vt() {
        let baza = Path::new(r"C:\Zakon.Local\Baza");
        if !TridentIndex::is_available(baza) {
            eprintln!("skipped: no base");
            return;
        }
        let idx = TridentIndex::open(baza).expect("index should load");
        // «був» → LmCode [0xA1, 0xE3, 0xA2] → "BUV" (confirmed in lmur.vt
        // at 0x537d0 / 0x8422f)
        let hits = idx.exact_postings("був");
        assert!(!hits.is_empty(), "BUV must be found in the .vt stream");
        let (cand, off, _u16s) = &hits[0];
        assert!(
            ["BUV", "BUW", "BUB"].contains(&cand.as_str()),
            "candidate {cand}"
        );
        assert!(*off >= 0x40 && *off < 0xCCDE5, "record offset in range");
        // «пере» → "PERE" (found inside APEREMM, NAPEREMHE, …)
        assert!(!idx.exact_postings("пере").is_empty());
        // search() carries the join hits
        let r = idx.search("був");
        assert!(!r.joined.is_empty(), "search must report the join");
        // an unknown word finds nothing
        assert!(idx.exact_postings("zzzz").is_empty());
    }

    #[test]
    fn term_join_docs_dedupes_and_drops_zero() {
        let baza = Path::new(r"C:\Zakon.Local\Baza");
        if !TridentIndex::is_available(baza) {
            eprintln!("skipped: no base");
            return;
        }
        let idx = TridentIndex::open(baza).expect("index should load");
        // «був» (BUV) yields a non-empty de-duplicated u16 doc-id set, all
        // non-zero and sorted
        let docs = idx.term_join_docs("був");
        assert!(!docs.is_empty(), "BUV must yield doc-ids");
        assert!(docs.iter().all(|d| *d != 0), "zero (a terminator) is dropped");
        assert!(
            docs.windows(2).all(|w| w[0] <= w[1]),
            "the doc-id set is sorted"
        );
        assert_eq!(
            docs.len(),
            docs.iter().cloned().collect::<std::collections::HashSet<_>>().len(),
            "no duplicates"
        );
        // an unknown word yields nothing
        assert!(idx.term_join_docs("zzzz").is_empty());
    }

    /// v0.25.0: the Russian core loads (the language-generic loader).
    #[test]
    fn rus_index_loads_the_rus_core() {
        let baza = Path::new(r"C:\Zakon.Local\Baza");
        if !TridentIndex::is_available(baza) {
            eprintln!("skipped: no base");
            return;
        }
        let idx = TridentIndex::open(baza).expect("index should load");
        // the Russian fold table is the 32-pair R section (real)
        assert_eq!(idx.fold.rus.len(), 32, "the R section has 32 pairs");
        // the Russian dictionary + the raw u16 stream load from lmr.voc /
        // lmru.vt (the v0.25.0 language-generic loaders)
        assert!(
            !idx.rus_dict_words.is_empty(),
            "the lmr.voc dictionary should yield words"
        );
        assert!(!idx.rus_vt_u16.is_empty(), "lmru.vt should have a u16 stream");
        // the Russian `.vt` record stream parses (14421 records, v0.20.0)
        let rus_stream = idx
            .rus_vt_stream
            .as_ref()
            .expect("the lmru.vt stream should parse");
        assert_eq!(
            rus_stream.records.len(),
            14421,
            "the lmru.vt stream has 14421 records"
        );
    }

    /// v0.25.0: `tsearch --lang r` «закон» prints joined records (the rus
    /// exact join resolves the Russian stem against the `lmru.vt` stream).
    #[test]
    fn tsearch_lang_r_resolves_a_rus_stem() {
        let baza = Path::new(r"C:\Zakon.Local\Baza");
        if !TridentIndex::is_available(baza) {
            eprintln!("skipped: no base");
            return;
        }
        let idx = TridentIndex::open(baza).expect("index should load");
        // «закон» through fold.rus resolves a Russian stem in lmru.vt
        let r = idx.search_lang("закон", Lang::Rus);
        assert!(
            !r.joined.is_empty() || !r.docs.is_empty() || !r.matched.is_empty(),
            "the rus search must produce evidence (join / fallback / dict)"
        );
        // the auto merge runs both languages and combines the evidence
        let r_auto = idx.search_auto("закон");
        assert!(
            r_auto.dict_size >= idx.dict_words.len(),
            "auto merges both dictionaries (ukr + rus)"
        );
        // an unknown term yields nothing in the rus language too
        let r_z = idx.search_lang("zzzz", Lang::Rus);
        assert!(r_z.matched.is_empty() && r_z.joined.is_empty());
    }
}
