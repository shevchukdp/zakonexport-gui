//! `.vt` posting-stream + `.m` metadata decode (v0.19.0).
//!
//! Empirically mapped byte-for-byte against the real `lmur.vt` / `lmru.vt`
//! / `lmur.m` / `lmru.m` and the decompiled `lmsup32.dll` join chain.
//!
//! ## The 64-byte file header
//!
//! Both `.vt` and `.m` files start with the same 64-byte header:
//!
//! ```text
//!   0x00  vendor string, NUL-padded 16 ("Trident Soft. Inc.")
//!   0x18  file name, NUL-padded 16 ("lmUR.vt", "lmUR.m", …)
//!   0x38  the literal "FREE" (format marker)
//!   0x3C  u32 tail pointer
//! ```
//!
//! For `.vt` files the tail pointer is the **end of the packed record
//! stream**: `lmur.vt` holds `0xCCDE5` = file length − 4, and the contiguous
//! record stream `0x40..0xCCDE5` walks exactly 14160 records and lands
//! precisely on `0xCCDE5`. `lmru.vt` holds `0xD2648` = file length − 4 the
//! same way. For `.m` files the tail pointer marks the **start of a packed
//! B-tree tail section** (`lmur.m` holds `0x70E`, the file is `0x3DB2`
//! long; the tail region carries u16 B-tree node headers with the `0x80`
//! flag, the same style as the `.voc` tail).
//!
//! ## The `.vt` record stream
//!
//! From `0x40` up to the tail pointer, the file is a **contiguous pack of
//! NUL-terminated records**:
//!
//! ```text
//!   record = [u8 count][payload …][NUL]
//! ```
//!
//! - `count` — the first byte of the record. In the DLL's record grammar it
//!   is the field count (fields separated by `0x0E`/`0x09` inside the
//!   payload; some fields are binary u16 data).
//! - `payload` — a mix of **case-folded Latin stems** (the dictionary words
//!   re-encoded into a Latin index alphabet: «не»→`NE`, «по»→`PO`,
//!   «пере»→`PERE`, «спів»→`SPV`, «був»→`BUV`, «неч»→`HCH`, …) and u16
//!   data words (document ids 0..65535). Digits stay ASCII; the stream's
//!   sort order puts `@` (0x40, the first record's lead byte) and the
//!   ASCII digits before the letters — the same order as the `.voc`
//!   dictionary.
//! - The stream ends exactly at `header[0x3C]`; the last 4 bytes of the
//!   file are the little-endian tail pointer itself (`lmur.vt`:
//!   `00 8C 2E CD`).
//!
//! `lmur.vt`: 14160 records in `0x40..0xCCDE5` (verified by a full walk).
//!
//! ## The `.m` u32 table
//!
//! A `.m` file starts the body with a u32 table at `0x40` (`lmur.m`:
//! `0x1000, 0x88, 0x8CB, 0x726, 0xA1, 0x206D, 0x244B, 0x29D7`). The
//! sections are per-word metadata (the `.m` stream holds ~1143 NUL records
//! for the 1002-word `lmu.voc` dictionary) — the 0xE doc-flag structs that
//! `FUN_100177d0` predicates consume (flags 0xD7/0xD8/0xD9 of the
//! in-memory metadata).
//!
//! ## The DLL join chain (documented, not yet executed in Rust)
//!
//! The in-memory context (`DAT_1002f1f0`, 0x58 bytes) the loader
//! (`FUN_10015340`) builds:
//!
//! ```text
//!   +0x00  HFILE handle        +0x04  file-start pointer
//!   +0x30  seek(hdr+0x54)  the u16 predicate table (+0x15cd0 consumer)
//!   +0x34  seek(hdr+0x5C)        +0x38  seek(hdr+0x64)
//!   +0x3C  seek(hdr+0x78)        +0x40  seek(hdr+0x74)
//!   +0x44  seek(hdr+0x6C)  the u16 word table (+0x176c0 join core)
//!   +0x48  seek(hdr+0x70)        +0x4C  seek(hdr+0x4C)
//!   +0x50  seek(hdr+0x50)        +0x56  u16 flags, +0x57 byte flag
//! ```
//!
//! The join core `FUN_100176c0`: `table[idx]` (the u16 word table at
//! `ctx+0x44`) → block-relative record → **kind byte** (`0xFA` special,
//! `0x33` = a nested `FUN_100177d0(u8,u8)` doc-flag predicate chain with
//! skip-bytes `< 0x20`) → `FUN_10015cd0` (u16 predicate table at
//! `ctx+0x30`; `0xFF` = include all documents) → `FUN_100177d0`
//! (the `.m` doc-flag predicates).
//!
//! **Blocked (v0.20):** in the old-format `.vt` files the header fields
//! `0x4C..0x7C` are just stream bytes, so the u16 word/predicate tables
//! must be derived from the tail B-tree (the `FUN_100134d0` tail scan +
//! the `FUN_100019f0` byte-VM over the tail B-tree nodes). What ships in
//! v0.19.0 is the exact **stem join**: word → LmCode → case-folded Latin
//! candidates → `VtRecordStream::find_stem` → the matching records and
//! their u16 word lists (the `.vt` posting evidence), with the
//! proportional `posting_slice` as fallback.
//!
//! ## v0.20.0: the tail section, the `.use` files, the Russian base
//!
//! The `FUN_100134d0` tail section (`VtTailScan`) is **empty in the
//! old-format files** — the tail pointer sits inside the last 4 file
//! bytes, so the `u8 N` is 0. The u16 word/predicate tables the join
//! core needs (`ctx+0x30`/`ctx+0x44`) therefore must be derived from
//! the `.voc` B-tree (the next RE step). The wildcard matcher
//! `tail_match` (`FUN_100135d0`) is ported for the day the tail
//! section carries records. The `.use` usage-flags files
//! (`UseFile`) are the 64-byte header + an all-zero 140-byte body in
//! this base. The Russian streams (`lmru.vt` / `lmru.m`) follow the
//! exact same grammar (14421 records, `NET`/`PER`/`PO` stems
//! confirmed) and are loaded alongside the Ukrainian ones.

/// The 64-byte header common to `.vt` and `.m` files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TridentFileHeader {
    /// `@0x00` — the vendor string (NUL-trimmed), e.g. `Trident Soft. Inc.`.
    pub vendor: String,
    /// `@0x18` — the file name (NUL-trimmed), e.g. `lmUR.vt`.
    pub name: String,
    /// `@0x38..0x3B` — the literal `FREE` format marker.
    pub free: bool,
    /// `@0x3C` — the tail pointer (`.vt`: end of the record stream;
    /// `.m`: start of the packed B-tree tail section).
    pub tail: u32,
}

impl TridentFileHeader {
    /// Parse the 64-byte header. Returns `None` when the buffer is too
    /// short or the `FREE` marker is missing.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 0x40 {
            return None;
        }
        let cstr = |off: usize, len: usize| {
            let end = bytes[off..off + len]
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(len);
            String::from_utf8_lossy(&bytes[off..off + end]).into_owned()
        };
        let free = bytes[0x38..0x3C] == *b"FREE";
        let tail = u32::from_le_bytes(bytes[0x3C..0x40].try_into().unwrap());
        Some(TridentFileHeader {
            // vendor occupies 0x00..0x18 (the 24-byte block before the name)
            vendor: cstr(0x00, 0x18),
            name: cstr(0x18, 0x10),
            free,
            tail,
        })
    }
}

/// One packed `.vt` record: `[u8 count][payload][NUL]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VtRecord {
    /// The offset of the count byte.
    pub off: usize,
    /// The first byte (the field count in the DLL's record grammar).
    pub count: u8,
    /// The payload (everything up to the NUL terminator, NUL excluded).
    pub payload: Vec<u8>,
    /// The maximal `A-Z`/`0-9` runs inside the payload (length >= 2) —
    /// the case-folded Latin stems the join matches against.
    pub latin_runs: Vec<String>,
    /// The u16 words of the payload, read at even offsets from the first
    /// payload byte (the document-id evidence, 0..65535).
    pub u16_words: Vec<u16>,
}

/// The contiguous NUL-terminated record stream of a `.vt` file
/// (`0x40..header[0x3C]`; 14160 records in `lmur.vt`).
#[derive(Debug, Clone)]
pub struct VtRecordStream {
    /// The parsed 64-byte file header.
    pub header: TridentFileHeader,
    /// The records in stream order.
    pub records: Vec<VtRecord>,
    /// Where the walk stopped — must equal `header.tail` on a healthy
    /// file (the last record's NUL sits at `tail - 1` … `tail`).
    pub end: usize,
}

impl VtRecordStream {
    /// Walk the packed record stream from `0x40` to the header tail.
    ///
    /// Returns `None` when the header does not parse or the tail is out of
    /// bounds. A walk that stops short of the tail still yields the
    /// records seen so far (the stream is the evidence, not the contract).
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let header = TridentFileHeader::parse(bytes)?;
        let tail = header.tail as usize;
        if tail < 0x40 || tail > bytes.len() {
            return None;
        }
        let mut records = Vec::new();
        let mut p = 0x40;
        while p < tail {
            let count = bytes[p];
            let mut q = p + 1;
            while q < tail && bytes[q] != 0 {
                q += 1;
            }
            if q >= tail {
                break; // unterminated tail record
            }
            let payload = bytes[p + 1..q].to_vec();
            records.push(VtRecord {
                off: p,
                count,
                latin_runs: latin_runs_of(&payload),
                u16_words: u16_words_of(&payload),
                payload,
            });
            p = q + 1;
        }
        Some(VtRecordStream {
            header,
            records,
            end: p,
        })
    }

    /// The exact-join lookup (v0.19.0): records whose Latin runs contain
    /// `needle` (case-insensitive; the runs are already case-folded
    /// uppercase). Returns `(record index, the matched run)` pairs in
    /// stream order.
    pub fn find_stem(&self, needle: &str) -> Vec<(usize, &str)> {
        let n = needle.to_ascii_uppercase();
        if n.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (i, r) in self.records.iter().enumerate() {
            for run in &r.latin_runs {
                if run.contains(&n) {
                    out.push((i, run.as_str()));
                }
            }
        }
        out
    }
}

/// The `.m` per-word metadata header: the 64-byte file header plus the
/// u32 section table at `0x40`.
#[derive(Debug, Clone)]
pub struct VmHeader {
    /// The parsed 64-byte file header.
    pub header: TridentFileHeader,
    /// The u32 section table at `0x40` (`lmur.m`: 8 entries).
    pub table: Vec<u32>,
}

impl VmHeader {
    /// Parse a `.m` header with `entries` u32 table slots at `0x40`.
    pub fn parse(bytes: &[u8], entries: usize) -> Option<Self> {
        let header = TridentFileHeader::parse(bytes)?;
        if bytes.len() < 0x40 + entries * 4 {
            return None;
        }
        let mut table = Vec::with_capacity(entries);
        for i in 0..entries {
            table.push(u32::from_le_bytes(
                bytes[0x40 + i * 4..0x44 + i * 4].try_into().unwrap(),
            ));
        }
        Some(VmHeader { header, table })
    }
}

/// The maximal `A-Z` / `0-9` runs of a payload (length >= 2).
fn latin_runs_of(payload: &[u8]) -> Vec<String> {
    let is_latin = |b: u8| b.is_ascii_alphanumeric();
    let mut out = Vec::new();
    let mut i = 0;
    while i < payload.len() {
        if is_latin(payload[i]) {
            let a = i;
            while i < payload.len() && is_latin(payload[i]) {
                i += 1;
            }
            if i - a >= 2 {
                out.push(String::from_utf8_lossy(&payload[a..i]).into_owned());
            }
        } else {
            i += 1;
        }
    }
    out
}

/// The u16 words of a payload, aligned to the first payload byte.
fn u16_words_of(payload: &[u8]) -> Vec<u16> {
    payload
        .chunks(2)
        .take_while(|c| c.len() == 2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect()
}

/// The `.use` usage-flags file (`lmu.use` / `lmr.use`, 204 bytes):
/// the same 64-byte header plus a body of usage flags that is all
/// zeros in this base (the flags are not enabled for the 2005-era
/// index).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UseFile {
    /// The parsed 64-byte file header.
    pub header: TridentFileHeader,
    /// The body (`0x40..EOF`; 140 bytes in the real files).
    pub body: Vec<u8>,
}

impl UseFile {
    /// Parse a `.use` file (requires at least the 64-byte header).
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let header = TridentFileHeader::parse(bytes)?;
        Some(UseFile {
            body: bytes[0x40..].to_vec(),
            header,
        })
    }

    /// True when every body byte is zero (the real `lmu.use` / `lmr.use`).
    pub fn body_is_zero(&self) -> bool {
        self.body.iter().all(|&b| b == 0)
    }
}

/// One `FUN_100134d0` tail record: a NUL-terminated key string followed
/// by a `0x0000`-terminated u16 list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VtTailRecord {
    /// The offset of the key's first byte.
    pub off: usize,
    /// The NUL-terminated key (NUL excluded) — an LmCode word or a
    /// wildcard pattern (see [`tail_match`]).
    pub key: Vec<u8>,
    /// The u16 list (the `0x0000` terminator pair excluded) — the
    /// document-id postings for the key.
    pub list: Vec<u16>,
}

/// The `FUN_100134d0` tail section: at the header's tail pointer there is
/// a `u8 N` followed by `N` packed `[key][u16 list]` records.
///
/// **Old-format finding (v0.20.0):** in both real `.vt` files the tail
/// pointer sits inside the last 4 file bytes (the LE tail-pointer u32
/// itself), so `N = 0` — the tail section is **empty**. The u16
/// word/predicate tables the join core needs (`ctx+0x30`/`ctx+0x44`,
/// `FUN_100176c0`/`FUN_10015cd0`) therefore come from elsewhere (the
/// `.voc` B-tree, the next RE step), not from the `.vt` tail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VtTailScan {
    /// The `u8 N` at the tail pointer.
    pub count: u8,
    /// The parsed records (empty when `count = 0`).
    pub records: Vec<VtTailRecord>,
    /// Where the walk stopped (the start of the next section, or the
    /// end of the parsed records).
    pub end: usize,
}

impl VtTailScan {
    /// Parse the tail section at `at` (the header tail pointer).
    ///
    /// A truncated tail (a record running past `EOF`) yields the records
    /// parsed so far — the parse is the evidence, not the contract.
    pub fn parse(bytes: &[u8], at: usize) -> Option<Self> {
        let count = *bytes.get(at)?;
        let mut records = Vec::new();
        let mut p = at + 1;
        for _ in 0..count as usize {
            let key_off = p;
            // the NUL-terminated key
            let mut k = p;
            while k < bytes.len() && bytes[k] != 0 {
                k += 1;
            }
            if k >= bytes.len() {
                break; // unterminated key
            }
            let key = bytes[key_off..k].to_vec();
            // the u16 list, 0x0000-terminated (FUN_1001bd90 semantics:
            // walk pairs up to the zero pair; the next record starts
            // after it)
            let mut list: Vec<u16> = Vec::new();
            let mut l = k + 1;
            while l + 1 < bytes.len() {
                let u = u16::from_le_bytes([bytes[l], bytes[l + 1]]);
                if u == 0 {
                    l += 2;
                    break;
                }
                list.push(u);
                l += 2;
            }
            records.push(VtTailRecord {
                off: key_off,
                key,
                list,
            });
            p = l;
        }
        Some(VtTailScan {
            count,
            records,
            end: p,
        })
    }
}

/// `FUN_100135d0` — the tail-scan wildcard matcher, ported.
///
/// `pattern` is a NUL-terminated wildcard pattern; `subject` is the
/// record key (no NUL appended). Semantics:
///
/// - a run of `*` — the star consumes the subject remainder **except**
///   the room the rest of the pattern needs: the subject is advanced by
///   `subject remainder - pattern remainder`, and the match fails when
///   the subject remainder is shorter than the pattern remainder (the
///   decompiled length check);
/// - `N` — any single byte (the DLL's `FUN_10021b16` fold of the byte
///   is non-zero for every byte — "any");
/// - `R` — the separator class: `-` only when the subject remainder is
///   exactly one byte, otherwise `{; : & ,}` (the decompiled quirk is
///   preserved);
/// - `L` — a letter byte (the DLL's `FUN_10011580` character-property
///   bit 0x40; approximated here as ASCII/Cyrillic letters — the
///   property table itself is a v0.21+ extraction);
/// - any other pattern byte — a case-folded equality: the DLL folds the
///   subject byte (`FUN_10011930(x, -0x80)` — a case-folding map) and
///   compares to the (already case-folded) pattern key byte; this port
///   folds both sides (a superset: identical for upper-case keys).
///
/// Matching is prefix-style: the pattern's NUL wins only when it is
/// reached; a shorter subject than the pattern fails (no implicit
/// end-anchor).
pub fn tail_match(pattern: &[u8], subject: &[u8]) -> bool {
    let plen = pattern.len();
    let mut i = 0usize; // pattern index
    let mut j = 0usize; // subject index
    loop {
        while i < plen && pattern[i] == b'*' {
            i += 1;
            // the star consumes the subject remainder EXCEPT the room
            // the rest of the pattern needs (the decompiled
            // `pbVar6 += L - M` advance; the `L < M` check above is the
            // decompiled length fail)
            let rem_subj = subject.len() - j;
            let rem_pat = plen - i;
            if rem_subj < rem_pat {
                return false; // the decompiled length check
            }
            j += rem_subj - rem_pat;
        }
        if i >= plen {
            return true; // the pattern's NUL
        }
        if j >= subject.len() {
            return false; // the subject ran out (its NUL)
        }
        let c = pattern[i];
        let b = subject[j];
        match c {
            b'N' => {
                // any byte
                i += 1;
                j += 1;
            }
            b'R' => {
                let ok = if b == 0x2D {
                    // '-': only as the entire subject remainder
                    subject.len() - j == 1
                } else {
                    matches!(b, 0x3B | 0x3A | 0x26 | 0x2C)
                };
                if !ok {
                    return false;
                }
                i += 1;
                j += 1;
            }
            b'L' => {
                let is_letter = (b'A'..=b'Z').contains(&b)
                    || (b'a'..=b'z').contains(&b)
                    || b >= 0x80; // the LmCode letter slots
                if !is_letter {
                    return false;
                }
                i += 1;
                j += 1;
            }
            _ => {
                // the case-folded equality. The DLL folds only the
                // subject byte (`FUN_10011930(x, -0x80)`); the pattern
                // (the record key) is stored case-folded already. We
                // fold both sides — the superset that keeps the DLL
                // behavior when the pattern is upper-case and adds the
                // convenience of a lower-case pattern.
                let fold = |x: u8| if x.is_ascii_lowercase() { x - 0x20 } else { x };
                if fold(b) != fold(c) {
                    return false;
                }
                i += 1;
                j += 1;
            }
        }
    }
}

/// The `VocParams` re-export keeps the `.voc` / `.vt` parameter story in
/// one place for callers that only import this module.
pub use crate::trident_voc::VocParams as VocParamsReexport;

#[cfg(test)]
mod tests {
    use super::*;

    fn real(path: &str) -> Option<Vec<u8>> {
        std::fs::read(path).ok()
    }

    #[test]
    fn vt_header_parses_the_real_lmur_vt() {
        let Some(vt) = real(r"C:\Zakon.Local\Baza\SOFT\lmur.vt") else {
            eprintln!("skipped: no base");
            return;
        };
        let h = TridentFileHeader::parse(&vt).expect("header");
        assert_eq!(h.vendor, "Trident Soft. Inc.");
        assert_eq!(h.name, "lmUR.vt");
        assert!(h.free);
        assert_eq!(h.tail, 0xCCDE5, "tail pointer");
        assert_eq!(vt.len(), 0xCCDE9, "file length = tail + 4");
    }

    #[test]
    fn vt_record_stream_walks_exactly_14160_records_to_the_tail() {
        let Some(vt) = real(r"C:\Zakon.Local\Baza\SOFT\lmur.vt") else {
            eprintln!("skipped: no base");
            return;
        };
        let s = VtRecordStream::parse(&vt).expect("stream");
        assert_eq!(s.records.len(), 14160, "record count");
        assert_eq!(s.end, s.header.tail as usize, "walk lands on the tail");
        // the first record: count 2, payload "@RQRB…PEBH@RSP" (the '@'
        // 0x40 lead byte precedes the alphabet in the stream's sort order,
        // digits 0x30–0x39 come right after it)
        assert_eq!(s.records[0].count, 2);
        assert!(s.records[0]
            .payload
            .windows(4)
            .any(|w| w == *b"RQRB"));
        assert_eq!(s.records[0].payload[0], b'@');
    }

    #[test]
    fn vt_record_stream_parses_lmru_vt() {
        let Some(vt) = real(r"C:\Zakon.Local\Baza\SOFT\lmru.vt") else {
            eprintln!("skipped: no base");
            return;
        };
        let h = TridentFileHeader::parse(&vt).expect("header");
        assert_eq!(h.name, "lmRU.vt");
        assert_eq!(h.tail as usize, vt.len() - 4);
        let s = VtRecordStream::parse(&vt).expect("stream");
        assert!(!s.records.is_empty());
        assert_eq!(s.end, s.header.tail as usize);
    }

    #[test]
    fn find_stem_matches_the_confirmed_casefolded_stems() {
        let Some(vt) = real(r"C:\Zakon.Local\Baza\SOFT\lmur.vt") else {
            eprintln!("skipped: no base");
            return;
        };
        let s = VtRecordStream::parse(&vt).expect("stream");
        // «був» = LmCode [0xA1, 0xE3, 0xA2] -> "BUV" (confirmed hits)
        assert!(!s.find_stem("BUV").is_empty(), "BUV");
        // «по» -> "PO", «пере» -> "PERE", «спів» -> "SPV"
        assert!(!s.find_stem("PO").is_empty(), "PO");
        assert!(!s.find_stem("PERE").is_empty(), "PERE");
        assert!(!s.find_stem("SPV").is_empty(), "SPV");
        // an unknown stem finds nothing
        assert!(s.find_stem("ZZZZZ").is_empty());
    }

    #[test]
    fn m_header_parses_the_real_lmur_m() {
        let Some(m) = real(r"C:\Zakon.Local\Baza\SOFT\lmur.m") else {
            eprintln!("skipped: no base");
            return;
        };
        let h = VmHeader::parse(&m, 8).expect("header");
        assert_eq!(h.header.name, "lmUR.m");
        assert_eq!(h.header.tail, 0x70E, "tail section start");
        assert_eq!(
            h.table,
            vec![0x1000, 0x88, 0x0BCB, 0x726, 0xA1, 0x206D, 0x244B, 0x29D7]
        );
    }

    #[test]
    fn latin_runs_and_u16_words_extract_cleanly() {
        // [count]['N','E',0x0E,'P','O'][NUL]
        let payload = [b'N', b'E', 0x0E, b'P', b'O'];
        assert_eq!(
            latin_runs_of(&payload),
            vec!["NE".to_string(), "PO".to_string()]
        );
        // alignment: chunks start at the first payload byte — [N,E] then
        // [0x0E,P]; the trailing single 'O' is dropped
        assert_eq!(u16_words_of(&payload), vec![0x454E, 0x500E]);
    }

    #[test]
    fn voc_params_are_reachable_from_the_vt_module() {
        // the re-export compiles and is the same type
        let p: VocParamsReexport = VocParamsReexport::default();
        assert_eq!(p.raw, [0u16; 16]);
    }

    #[test]
    fn short_buffers_yield_none() {
        assert!(TridentFileHeader::parse(&[0u8; 32]).is_none());
        assert!(VtRecordStream::parse(&[0u8; 64]).is_none());
        assert!(VmHeader::parse(&[0u8; 64], 1).is_none());
    }

    #[test]
    fn use_file_parses_the_real_lmu_use() {
        let Some(u) = real(r"C:\Zakon.Local\Baza\SOFT\lmu.use") else {
            eprintln!("skipped: no base");
            return;
        };
        assert_eq!(u.len(), 204);
        let f = UseFile::parse(&u).expect("use file");
        assert_eq!(f.header.name, "lmU.use");
        assert!(f.header.free);
        assert_eq!(f.body.len(), 140);
        assert!(
            f.body_is_zero(),
            "the 2005-era base does not enable the usage flags"
        );
        let Some(r) = real(r"C:\Zakon.Local\Baza\SOFT\lmr.use") else {
            eprintln!("skipped: no base");
            return;
        };
        let fr = UseFile::parse(&r).expect("use file (rus)");
        assert!(fr.body_is_zero());
    }

    #[test]
    fn tail_scan_reports_empty_on_the_old_format_files() {
        for name in ["lmur.vt", "lmru.vt"] {
            let Some(vt) = real(&format!(r"C:\Zakon.Local\Baza\SOFT\{name}"))
            else {
                eprintln!("skipped: no base");
                return;
            };
            let stream = VtRecordStream::parse(&vt).expect("stream");
            // the old-format tail pointer sits inside the last 4 file
            // bytes (the LE tail-pointer u32 itself) — the `u8 N` there is
            // 0: the tail section is empty
            let ts = VtTailScan::parse(&vt, stream.end).expect("tail scan");
            assert_eq!(
                ts.count, 0,
                "{name}: the old-format tail must carry 0 records"
            );
            assert!(ts.records.is_empty());
        }
    }

    #[test]
    fn tail_scan_parses_a_synthetic_tail() {
        // [u8 N=1]["NE"[NUL]][0x1234][0x0000]
        let mut b = vec![0u8; 8 + 2 + 4 + 2];
        b[0] = 1;
        b[1] = b'N';
        b[2] = b'E';
        b[3] = 0;
        b[4] = 0x34;
        b[5] = 0x12;
        b[6] = 0x00;
        b[7] = 0x00;
        let ts = VtTailScan::parse(&b, 0).expect("tail");
        assert_eq!(ts.count, 1);
        assert_eq!(ts.records[0].key, *b"NE");
        assert_eq!(ts.records[0].list, vec![0x1234]);
    }

    #[test]
    fn tail_match_implements_the_wildcard_semantics() {
        // exact (case-folded)
        assert!(tail_match(b"ne", b"NE"));
        assert!(tail_match(b"NE", b"ne"));
        assert!(!tail_match(b"NE", b"NN"));
        // prefix-style: a longer subject still matches (a pattern with
        // no N/R/L class letters — pattern `R` is the separator CLASS,
        // not a literal, exactly as in the DLL)
        assert!(tail_match(b"PO", b"POS"));
        assert!(!tail_match(b"PO", b"QO"));
        // `*`: the remainder-room check (the decompiled length rule)
        assert!(tail_match(b"*E", b"PERE")); // * consumes PER
        assert!(tail_match(b"*", b"ANYTHING"));
        assert!(tail_match(b"*ZZ", b"NEZZ")); // exact-fit remainder
        assert!(!tail_match(b"*ZZ", b"NE")); // subject remainder too short
        // `N` = any single byte (prefix-style: a shorter pattern wins
        // over the subject remainder)
        assert!(tail_match(b"NN", b"PE"));
        assert!(tail_match(b"NN", b"PER"));
        assert!(!tail_match(b"NNN", b"PE")); // the subject runs out
        // `R` = the separator class {; : & ,} (the '-' quirk: only a
        // 1-byte remainder)
        assert!(tail_match(b"R", b";"));
        assert!(tail_match(b"R", b":"));
        assert!(tail_match(b"R", b"&"));
        assert!(tail_match(b"R", b","));
        assert!(tail_match(b"R", b"-")); // the quirk: 1-byte remainder
        assert!(!tail_match(b"R", b"-;")); // '-' with a longer remainder
        assert!(!tail_match(b"R", b"5"));
        // the literal '-' in the pattern + the class 'R' after it
        assert!(tail_match(b"X-R", b"X-;"));
        assert!(!tail_match(b"X-R", b"X-Y"));
        // `L` = a letter byte (the approximated property)
        assert!(tail_match(b"L", b"N"));
        assert!(tail_match(b"L", &[0xAD])); // the LmCode letter slot
        assert!(!tail_match(b"L", b"5"));
        // wildcards + plain: "N*R" — any 1-char prefix, a star, then the
        // separator class
        assert!(tail_match(b"N*R", b"PO;"));
        assert!(!tail_match(b"N*R", b"PO5"));
    }

    #[test]
    fn rus_streams_parse_the_real_lmru_files() {
        let Some(vt) = real(r"C:\Zakon.Local\Baza\SOFT\lmru.vt") else {
            eprintln!("skipped: no base");
            return;
        };
        let s = VtRecordStream::parse(&vt).expect("rus stream");
        assert_eq!(s.header.name, "lmRU.vt");
        assert_eq!(s.records.len(), 14421, "rus record count");
        assert_eq!(s.end, s.header.tail as usize, "walk lands on the tail");
        // the Russian stems share the Latin index alphabet (NET/PER/PO
        // confirmed present in the byte dumps)
        assert!(!s.find_stem("NET").is_empty());
        assert!(!s.find_stem("PER").is_empty());
        let Some(m) = real(r"C:\Zakon.Local\Baza\SOFT\lmru.m") else {
            eprintln!("skipped: no base");
            return;
        };
        let hm = VmHeader::parse(&m, 8).expect("rus .m header");
        assert_eq!(hm.header.name, "lmRU.m");
        assert_eq!(
            hm.table,
            vec![0x0C20, 0x88, 0xBD8, 0x56B, 0x99, 0x1ACF, 0x2000, 0x25C1]
        );
    }
}
