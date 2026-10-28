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
//!
//! ## v0.21.0: the `.m` full header — the u16 tables are in the `.m`
//!
//! The `.m` u32 table has **16 slots, not 8** (the v0.19.0 parse was
//! the first half). The empirical dumps pin the roles: **slot 5 =
//! the u16 predicate table** (`ctx+0x44`, `FUN_100176c0`/
//! `FUN_10015cd0`; 0x206D ukr / 0x1ACF rus) and **slot 11 = the u16
//! word table** (`ctx+0x30`; 0x37CB ukr / 0x3001 rus). The u16
//! tables therefore live in the `.m` files, not the `.vt` tail (and
//! not the `.voc` B-tree as v0.20.0 suspected). The `.m` tail
//! N-section is NON-empty — `VtTailScan` now parses it as part of
//! `VmHeader` (N=3 ukr: "R", "N*", "*"; N=4 rus: adds "*N") — the
//! `tail_match` wildcard keys. Two derived header fields are pinned
//! by the cross-language invariant: `u32@0x84 = pred_off - 7` and
//! `u32@0x80 = pred_off - 19`. The `trident dump` CLI verb ships the
//! hex-dump helper for the next RE steps; the `.use` files are the
//! engine's **write-mode** update caches (`lmC.use` opened 'W', the
//! 204-byte `FUN_10011e10` form) and `.uvt` opens 'W' only — both
//! reserved/never-written in this read-only base.
//!
//! ## v0.22.0: the u16 word/predicate tables, structural decode
//!
//! The two tables the join core consumes are structurally decoded
//! (`VmWordTable` / `VmPredicateTable`):
//!
//! - the **word table** (slot 11) is `[u16 count][count u16
//!   records @base+2]` — the count is the dictionary word count
//!   (254 ukr / 256 rus). The first ~half of the records are small
//!   ascending "offset-like" values (126 ukr / 128 rus, step 2),
//!   the second half are i16-negative values carrying the `0xFFFA`
//!   kind markers (61 ukr / 64 rus) and a final boundary record
//!   (0x8000 ukr / 0x7F00 rus);
//! - the **predicate table** (slot 5) is `[u16 offsets ×
//!   160/168][0xCECA sentinel][packed u8-op/u16-arg records]` —
//!   the offset run is non-strictly ascending and ends at the same
//!   `0xCECA` sentinel in both languages; the record region after
//!   the sentinel is the packed chain `FUN_10015cd0` walks.
//!
//! `TridentIndex` carries all four tables (ukr/rus) and the CLI
//! `trident` verb prints their fingerprints (sizes + kind
//! distributions). The semantic walk of the i16/predicate chains
//! (word → u16 word-idx → doc-ids) is the next step (the v0.23.0
//! doc-number join).

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

/// The `.m` per-word metadata header (v0.21.0 full decode): the
/// 64-byte file header, the **16-entry** u32 section table at
/// `0x40..0x80`, the two derived u32 fields at `0x80`/`0x84`, and
/// the parsed `FUN_100134d0` tail N-section (the wildcard join
/// substrate of `tail_match`).
///
/// **v0.21.0 empirical findings (both languages):**
/// - the table has 16 slots, not 8 — the earlier 8-entry parse was
///   the first half;
/// - **slot 5 = the u16 predicate table** offset (`ctx+0x44`,
///   `FUN_100176c0`/`FUN_10015cd0`): 0x206D ukr, 0x1ACF rus;
/// - **slot 11 = the u16 word table** offset (`ctx+0x30`): 0x37CB
///   ukr, 0x3001 rus;
/// - the u16 word/predicate tables therefore live **in the `.m`
///   files, not the `.vt` tail** (the old-format `.vt` tails are
///   empty, v0.20.0);
/// - `u32@0x84` is the verified invariant `pred_off - 7` in both
///   files (0x2066+7=0x206D; 0x1AC8+7=0x1ACF), and
///   `u32@0x80 = pred_off - 19` (the two fields differ by 12);
/// - the tail N-section is NOT empty for `.m` (unlike `.vt`):
///   N=3 for `lmur.m` ("R", "N*", "*"), N=4 for `lmru.m`
///   (adds "*N") — the `tail_match` wildcard keys that complete the
///   word → u16 word-index join.
#[derive(Debug, Clone)]
pub struct VmHeader {
    /// The parsed 64-byte file header.
    pub header: TridentFileHeader,
    /// The 16-entry u32 section table at `0x40..0x80`. Verified slot
    /// roles: 5 = the u16 predicate table, 11 = the u16 word table;
    /// the other slots start the 0xE-structure metadata sections
    /// (the packed stream from `0x8C`, `FUN_100177d0`).
    pub table: Vec<u32>,
    /// The u32 at `0x80` — observed as `pred_off - 19` in both
    /// real files (the end of the preceding section; semantics
    /// pending the 0xE-structure decode, v0.22+).
    pub u80: u32,
    /// The u32 at `0x84` — the verified invariant `pred_off - 7`.
    pub u84: u32,
    /// The parsed `FUN_100134d0` tail N-section (`None` when the
    /// tail pointer is out of range for the buffer).
    pub tail: Option<VtTailScan>,
}

impl VmHeader {
    /// Parse the full `.m` header (the 16-entry table + the `0x80` /
    /// `0x84` fields + the tail N-section). Requires at least `0x88`
    /// bytes.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let header = TridentFileHeader::parse(bytes)?;
        if bytes.len() < 0x88 {
            return None;
        }
        let table: Vec<u32> = (0..16)
            .map(|i| {
                u32::from_le_bytes(bytes[0x40 + i * 4..0x44 + i * 4].try_into().unwrap())
            })
            .collect();
        let u80 = u32::from_le_bytes(bytes[0x80..0x84].try_into().unwrap());
        let u84 = u32::from_le_bytes(bytes[0x84..0x88].try_into().unwrap());
        let tail = VtTailScan::parse(bytes, header.tail as usize);
        Some(VmHeader {
            header,
            table,
            u80,
            u84,
            tail,
        })
    }

    /// Slot 5 — the u16 predicate table offset (0x206D ukr / 0x1ACF
    /// rus; the `ctx+0x44` table `FUN_100176c0` walks).
    pub fn pred_off(&self) -> Option<u32> {
        self.table.get(5).copied()
    }

    /// Slot 11 — the u16 word table offset (0x37CB ukr / 0x3001 rus;
    /// the `ctx+0x30` table). The section layout is `[u16 count]` +
    /// `count` u16 records (count 0xFE=254 ukr / 0x100=256 rus —
    /// the dictionary word count; v0.22.0 decode).
    pub fn word_off(&self) -> Option<u32> {
        self.table.get(11).copied()
    }
}

/// The `.m` u16 word table (slot 11, v0.22.0 structural decode):
/// `[u16 count][count u16 records @base+2]`.
///
/// Empirical findings (both real files):
/// - the count is the dictionary word count (254 ukr / 256 rus);
/// - the first ~half of the records (126 ukr / 128 rus) are small
///   ascending "offset-like" values (0x0100+2i ukr, 0x0102+2i rus);
/// - the second half are i16-negative values (high byte 0xFF) with
///   the `0xFFFA` kind markers (61 ukr / 64 rus) and a final
///   boundary record (0x8000 ukr / 0x7F00 rus).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmWordTable {
    /// The section start (slot 11): 0x37CB ukr / 0x3001 rus.
    pub base: u32,
    /// The u16 record count (@base): 254 ukr / 256 rus.
    pub count: u16,
    /// The u16 records (@base+2, `count` of them).
    pub records: Vec<u16>,
}

impl VmWordTable {
    /// Parse the word-table section (the full section must be in
    /// range; a truncated section yields `None`).
    pub fn parse(bytes: &[u8], base: u32) -> Option<Self> {
        let b = base as usize;
        if b + 2 > bytes.len() {
            return None;
        }
        let count = u16::from_le_bytes([bytes[b], bytes[b + 1]]);
        let end = b + 2 + count as usize * 2;
        if end > bytes.len() {
            return None;
        }
        let records = (0..count as usize)
            .map(|i| {
                u16::from_le_bytes([bytes[b + 2 + 2 * i], bytes[b + 3 + 2 * i]])
            })
            .collect();
        Some(VmWordTable {
            base,
            count,
            records,
        })
    }

    /// The count of the small "offset-like" records (< 0x8000): 126
    /// ukr / 128 rus (the first half of the table).
    pub fn offset_like_count(&self) -> u32 {
        self.records.iter().filter(|v| **v < 0x8000).count() as u32
    }

    /// The last offset-like record (0x01FA ukr / the rus analogue).
    pub fn last_offset(&self) -> Option<u16> {
        self.records
            .iter()
            .copied()
            .take_while(|v| *v < 0x8000)
            .last()
    }

    /// The `0xFFFA` kind-marker count (61 ukr / 64 rus).
    pub fn kind_fa_count(&self) -> u32 {
        self.records.iter().filter(|&&v| v == 0xFFFA).count() as u32
    }

    /// The final boundary record (0x8000 ukr / 0x7F00 rus).
    pub fn boundary(&self) -> Option<u16> {
        self.records.last().copied()
    }
}

/// The `.m` u16 predicate table (slot 5, v0.22.0 structural decode):
/// `[u16 offsets × N][0xCECA sentinel][packed u8-op/u16-arg records]`.
///
/// Empirical findings (both real files): the offset run is
/// non-strictly ascending and ends at the `0xCECA` sentinel (160
/// offsets + sentinel ukr, 168 + sentinel rus); the record region
/// after the sentinel is the packed predicate chain
/// `FUN_10015cd0` walks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmPredicateTable {
    /// The section start (slot 5): 0x206D ukr / 0x1ACF rus.
    pub base: u32,
    /// The ascending u16 offsets (the sentinel included as the last
    /// entry: 161 ukr / 169 rus).
    pub offsets: Vec<u16>,
    /// The last offset of the run — the `0xCECA` sentinel in both
    /// real files.
    pub sentinel: u16,
    /// The first 64 bytes of the record region (after the sentinel).
    pub record_sample: Vec<u8>,
}

impl VmPredicateTable {
    /// Parse the predicate table: walk the non-strictly-ascending u16
    /// run from `base` (it ends at the 0xCECA sentinel, or the first
    /// descent); take 64 record bytes after it.
    pub fn parse(bytes: &[u8], base: u32) -> Option<Self> {
        let b = base as usize;
        if b + 8 > bytes.len() {
            return None;
        }
        let mut offsets = Vec::new();
        let mut p = b;
        let mut last = 0u16;
        while p + 2 <= bytes.len() {
            let u = u16::from_le_bytes([bytes[p], bytes[p + 1]]);
            if !offsets.is_empty() && u < last {
                break; // the run ended (the sentinel was the last)
            }
            offsets.push(u);
            last = u;
            p += 2;
        }
        if offsets.len() < 4 {
            return None;
        }
        let sentinel = *offsets.last().unwrap();
        let record_sample = bytes[p..(p + 64).min(bytes.len())].to_vec();
        Some(VmPredicateTable {
            base,
            offsets,
            sentinel,
            record_sample,
        })
    }

    /// The offset count excluding the sentinel (160 ukr / 168 rus).
    pub fn offset_count(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }
}

// ---------------------------------------------------------------------------
// v0.26.0: the `.m` section walker (roadmap v0.27.0)
// ---------------------------------------------------------------------------

/// The structural kind of one `.m` 16-slot table section (v0.26.0),
/// classified empirically from the two real files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VmSectionKind {
    /// `[u16 count][count u16 records]` and the section ends **exactly**
    /// on the next table slot in file order (the u16 word table:
    /// 0x37CB→0x39C9 ukr, 0x3001→0x3203 rus).
    U16Records,
    /// A count field in the head + NUL-terminated (or 2-byte pair)
    /// records; the best-effort NUL walk lands *inside* or *past* the
    /// next slot (the 0x206D / 0x1ACF predicate regions, the 0x339D
    /// pair tables).
    NulRecords,
    /// The per-word 16-byte row block (slot 0: 0x1000 ukr / 0xC20 rus —
    /// 64 rows / 51 rows; the `0xE` per-word rows of the predicate
    /// machinery, structurally decoded here).
    WordRows,
    /// The u16-offset / `(u8 len, u8 flag)` key directory (slot 7:
    /// 0x29D7 ukr / 0x25C1 rus — the packed B-tree key index).
    LenFlagIndex,
    /// A small fixed config block (span ≤ 64 B: the 0x88 registry,
    /// 0xBCB / 0xBF3 ukr, 0xC09 / 0xC12 rus).
    Small,
    /// An opaque packed region (the B-tree 0x1405 / 0xF5D, the
    /// row regions 0x244B / 0x3000 / 0x36EF, the sparse 0xBFC).
    Packed,
    /// The head did not classify.
    Unknown,
}

/// One walkable `.m` section (roadmap v0.27.0 DoD: "every walkable
/// section lands on the next table slot" — holds for the `U16Records`
/// tables; the other kinds interleave with the packed B-tree region and
/// land at the region end or the file tail, documented per kind).
#[derive(Debug, Clone)]
pub struct VmSectionWalk {
    /// The table slot index (the slot whose value is `off`).
    pub slot: u8,
    /// The section start (the slot value).
    pub off: u32,
    /// The first 4 bytes as a u32 (the head fingerprint).
    pub head_u32: u32,
    pub kind: VmSectionKind,
    /// The record count: the u16 count (U16Records / NulRecords), the
    /// row count (WordRows), the `(len, flag)` pair count (LenFlagIndex),
    /// 0 for Small / Packed / Unknown.
    pub records: u32,
    /// The offset after the last record (the DLL walker's `landed`).
    pub landed: u32,
    /// `landed == the next table slot in file order` (the tile
    /// definition; only `U16Records` sections tile in the real files).
    pub lands_on_next: bool,
}

/// The per-word 16-byte rows of slot 0 (v0.26.0): 0x1000..0x1405 ukr
/// (64 rows + 5 B leftover), 0xC20..0xF5D rus (51 rows + 13 B). Every
/// 16-byte row carries data (ukr: a u8 field at +0x0C that is often
/// zero but the surrounding bytes are set; rus: packed LmCode words +
/// u16 fields). The on-disk encoding of the DLL's 224+ B `0xE` rows
/// (the +0xD7 / +0xD8 / +0xD9 flag bytes of `FUN_100177d0`) is a
/// v0.29+ B-tree item; this is the structural row decode.
#[derive(Debug, Clone)]
pub struct VmWordRows {
    /// The section start (slot 0).
    pub base: u32,
    /// The 16-byte rows (the span down to the next table slot, 16-byte
    /// aligned).
    pub rows: Vec<[u8; 16]>,
    /// The trailing bytes past the last full 16-byte row (5 ukr / 13 rus).
    pub leftover: u32,
    /// The rows carrying any non-zero byte.
    pub nonzero_rows: u32,
    /// The +0x0C row-field histogram over all rows (the ukr field values
    /// are LmCode letter slots 0xA3..0xEF + small counters; sorted by
    /// field value).
    pub field_0c_hist: Vec<(u8, u32)>,
}

impl VmWordRows {
    /// Decode the 16-byte row block in `[base, end)` (the next table
    /// slot in file order, or the file end).
    pub fn parse(bytes: &[u8], base: u32, end: u32) -> Option<Self> {
        let b = base as usize;
        let e = (end as usize).min(bytes.len());
        if b + 16 > e {
            return None;
        }
        let span = e - b;
        let mut hist: std::collections::HashMap<u8, u32> =
            std::collections::HashMap::new();
        let mut rows: Vec<[u8; 16]> = Vec::new();
        let mut nonzero = 0u32;
        for r in (b..e).step_by(16).take_while(|&p| p + 16 <= e) {
            let mut row = [0u8; 16];
            row.copy_from_slice(&bytes[r..r + 16]);
            if row.iter().any(|&x| x != 0) {
                nonzero += 1;
            }
            let v = row[12];
            *hist.entry(v).or_insert(0) += 1;
            rows.push(row);
        }
        let mut field_0c_hist: Vec<(u8, u32)> = hist.into_iter().collect();
        field_0c_hist.sort_by_key(|(v, _)| *v);
        Some(VmWordRows {
            base,
            rows,
            leftover: (span % 16) as u32,
            nonzero_rows: nonzero,
            field_0c_hist,
        })
    }
}

/// The 0x88 field-name registry block (slot 1; v0.26.0 empirical
/// correction: **not** a u16-counted NUL-terminated string section —
/// it is a small u32 config block). Layout: `[u32 marker][u32
/// offset][u32 offset][u32 offset][u32 offset][u32 offset]` (the marker
/// is 0x00FFxxxx — 0x00FFE703 ukr / 0x00FFEF02 rus; its low byte is
/// the sub-entry count: 3 ukr / 2 rus; the u32 offsets point into the
/// packed B-tree / row regions: 0x1900 / 0x1A00 / 0x1400 / 0x2400 /
/// 0x2D00 ukr, 0x1100 / 0x1200 / 0x0D00 / … rus). The string decoding (the "field names") is blocked on
/// the B-tree predicate walk (v0.29+).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmFieldRegistry {
    /// The block start (0x88 in both files).
    pub base: u32,
    /// The marker u32 (0x00FFE703 ukr / 0x00FFEF02 rus — the 0xFF word
    /// sits in bits 16..23; the low byte is the sub-entry count).
    pub u88: u32,
    /// The u32 offset stream (@0x8C..@0x9C).
    pub offsets: [u32; 5],
}

impl VmFieldRegistry {
    /// Parse the 0x88 block (requires at least `base + 0x22` bytes).
    pub fn parse(bytes: &[u8], base: u32) -> Option<Self> {
        let b = base as usize;
        if b + 0x22 > bytes.len() {
            return None;
        }
        let u88 = u32::from_le_bytes(bytes[b..b + 4].try_into().unwrap());
        if (u88 & 0x00FF_0000) != 0x00FF_0000 {
            return None; // the 0xFF word (bits 16..23) is the block marker
        }
        let mut offsets = [0u32; 5];
        for i in 0..5 {
            offsets[i] =
                u32::from_le_bytes(bytes[b + 4 + i * 4..b + 8 + i * 4].try_into().unwrap());
        }
        Some(VmFieldRegistry { base, u88, offsets })
    }

    /// The marker's low byte (the sub-entry count: 3 ukr / 2 rus).
    pub fn sub_count(&self) -> u8 {
        (self.u88 & 0xFF) as u8
    }
}

/// The full `.m` section walker (v0.26.0): classifies every table slot
/// in file order and decodes the two role sections (slot 0 = the
/// per-word rows, slot 1 = the 0x88 registry).
#[derive(Debug, Clone)]
pub struct VmSections {
    /// The per-slot walks (deduplicated offsets, file order).
    pub walks: Vec<VmSectionWalk>,
    /// The slot-0 per-word rows (0x1000 ukr / 0xC20 rus).
    pub word_rows: Option<VmWordRows>,
    /// The slot-1 0x88 registry block.
    pub registry: Option<VmFieldRegistry>,
}

impl VmSections {
    /// Classify + walk every table slot of a parsed `.m` header.
    pub fn parse(header: &VmHeader, bytes: &[u8]) -> Self {
        let mut offs: Vec<u32> = header.table.iter().copied().collect();
        offs.retain(|&o| o > 0 && o < bytes.len() as u32);
        offs.sort_unstable();
        offs.dedup();

        // the file-order sections (span = down to the next slot or EOF)
        let mut walks: Vec<VmSectionWalk> = Vec::new();
        for (si, &off) in offs.iter().enumerate() {
            let slot = header
                .table
                .iter()
                .enumerate()
                .find(|(_, v)| **v == off)
                .map(|(i, _)| i as u8)
                .unwrap_or(0xFF);
            let head_u32 = if off as usize + 4 <= bytes.len() {
                u32::from_le_bytes(
                    bytes[off as usize..off as usize + 4].try_into().unwrap(),
                )
            } else {
                0
            };
            let count = u16::from_le_bytes([
                bytes[off as usize],
                bytes.get(off as usize + 1).copied().unwrap_or(0),
            ]) as u32;
            let next = offs.get(si + 1).copied();
            let end = next.unwrap_or(bytes.len() as u32);
            let span = end - off;

            // 1. the exact-tile u16 record tables (the word tables):
            //    [u16 count][count u16 records] ending exactly on the
            //    next slot.
            let u16_end = off + 2 + count * 2;
            if count >= 1
                && next.is_some()
                && u16_end == next.unwrap()
            {
                walks.push(VmSectionWalk {
                    slot,
                    off,
                    head_u32,
                    kind: VmSectionKind::U16Records,
                    records: count,
                    landed: u16_end,
                    lands_on_next: true,
                });
                continue;
            }
            // 2. the role slots: slot 0 = the per-word rows, slot 1 =
            //    the 0x88 registry (the `Small`/registry decode below),
            //    slot 7 = the len-flag key index.
            if slot == 0 && span >= 16 {
                walks.push(VmSectionWalk {
                    slot,
                    off,
                    head_u32,
                    kind: VmSectionKind::WordRows,
                    records: span / 16,
                    landed: end,
                    lands_on_next: false,
                });
                continue;
            }
            if slot == 7 {
                let pairs = walk_lenflag_pairs(bytes, off, end);
                walks.push(VmSectionWalk {
                    slot,
                    off,
                    head_u32,
                    kind: VmSectionKind::LenFlagIndex,
                    records: pairs,
                    landed: end,
                    lands_on_next: false,
                });
                continue;
            }
            // 3. the small config blocks (span ≤ 64 B).
            if span <= 64 {
                walks.push(VmSectionWalk {
                    slot,
                    off,
                    head_u32,
                    kind: VmSectionKind::Small,
                    records: 0,
                    landed: end,
                    lands_on_next: false,
                });
                continue;
            }
            // 4. a count field + a NUL walk (the predicate / pair
            //    regions): the walk consumes `count` NUL-terminated
            //    records and lands where it lands.
            if count >= 1 && count <= 4096 {
                let landed = nul_walk_land(bytes, off, count, 4096);
                walks.push(VmSectionWalk {
                    slot,
                    off,
                    head_u32,
                    kind: VmSectionKind::NulRecords,
                    records: count,
                    landed,
                    lands_on_next: next.is_some() && landed == next.unwrap(),
                });
                continue;
            }
            // 5. everything else: the opaque packed regions.
            walks.push(VmSectionWalk {
                slot,
                off,
                head_u32,
                kind: VmSectionKind::Packed,
                records: 0,
                landed: end,
                lands_on_next: false,
            });
        }

        // the role decodes
        let word_rows = header
            .table
            .get(0)
            .copied()
            .filter(|&o| o > 0 && o < bytes.len() as u32)
            .and_then(|o| {
                let si = offs.iter().position(|&x| x == o)?;
                let end = offs.get(si + 1).copied().unwrap_or(bytes.len() as u32);
                VmWordRows::parse(bytes, o, end)
            });
        let registry = header
            .table
            .get(1)
            .copied()
            .filter(|&o| o > 0 && o < bytes.len() as u32)
            .and_then(|o| VmFieldRegistry::parse(bytes, o));

        VmSections {
            walks,
            word_rows,
            registry,
        }
    }

    /// The walk of one slot (`None` when the slot value is not a
    /// section in this file).
    pub fn walk(&self, slot: u8) -> Option<&VmSectionWalk> {
        self.walks.iter().find(|w| w.slot == slot)
    }

    /// How many sections tile the file exactly (the DoD assertion
    /// target; empirically the `U16Records` word tables only).
    pub fn tiling_count(&self) -> u32 {
        self.walks.iter().filter(|w| w.lands_on_next).count() as u32
    }
}

/// Count the `(u8 len, u8 flag)` pairs of a key directory: the section
/// may start with NUL padding or an ascending u16 offset run; the
/// pairs follow (each 2 bytes) until a NUL or the section end.
fn walk_lenflag_pairs(bytes: &[u8], off: u32, end: u32) -> u32 {
    let mut p = off as usize;
    let e = end as usize;
    // skip NUL padding
    while p < e && bytes[p] == 0x00 {
        p += 1;
    }
    // skip an ascending u16 offset run (the rus 0x25C1 shape)
    while p + 2 < e {
        let u = u16::from_le_bytes([bytes[p], bytes[p + 1]]);
        if u == 0x0000 {
            p += 2;
            break;
        }
        // an offset run value stays below the section span
        if (u as u32) > (end - off) {
            break;
        }
        p += 2;
        // pairs start with small values (<= 0x80)
        if *bytes.get(p).unwrap_or(&0x01) > 0x80 {
            break;
        }
    }
    let mut n = 0u32;
    while p + 2 <= e {
        if bytes[p + 1] > 0x02 || bytes[p] == 0x00 {
            break;
        }
        n += 1;
        p += 2;
    }
    n
}

/// The `FUN_10004230` NUL-terminated walk landing (the `trident_voc`
/// advance rule with WALK_BLOCK = 4096), consuming `count` records.
fn nul_walk_land(bytes: &[u8], off: u32, count: u32, block: u32) -> u32 {
    let mut p = off as usize + 2;
    for _ in 0..count {
        if p >= bytes.len() {
            break;
        }
        let mut len = 1usize;
        while p + len < bytes.len() && bytes[p + len] != 0x00 {
            len += 1;
        }
        if p + len < bytes.len() && bytes[p + len] == 0x00 {
            p += len + 1;
        } else {
            p = ((p / block as usize) + 1) * block as usize;
        }
    }
    p as u32
}

// ---------------------------------------------------------------------------
// v0.27.0: the `FUN_100177d0` predicate catalog (roadmap v0.28.0)
// ---------------------------------------------------------------------------

/// One predicate opcode of `FUN_100177d0` (roadmap v0.28.0), named by its
/// raw (hex) opcode value. The 22 variants are the complete decompiled
/// switch of the DLL's doc-flag predicate table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PredOp {
    /// 0x00 — unresolved `FUN_10011600(+0xDD)&0x80 == 0` accessor.
    Op0,
    /// 0x01 — unresolved `FUN_10011580(idx>>8, +0xDD)&0x40 != 0` accessor.
    Op1,
    /// 0x02 — always true; OR `0x0C` into the low flag byte.
    Op2,
    /// 0x05 — the next row's `+0xD9` counter is exactly `cur + 1`.
    Op5,
    /// 0x06 — the header `H[+8] == 1` (structural approximation via
    /// `PredRow::header8`).
    Op6,
    /// 0x07 — `!(+0xD7 & 8)`.
    Op7,
    /// 0x08 — always true; OR `0x02` into the low flag byte.
    Op8,
    /// 0x0B — `!(+0xD7 & 0x10)`.
    Op0B,
    /// 0x0C — always true; OR `0x28` into the low flag byte.
    Op0C,
    /// 0x0D — always true; OR `0x40` into the low flag byte.
    Op0D,
    /// 0x10 — always true; OR `0x02` into the high flag byte.
    Op10,
    /// 0x14 — `+0xD8 & 1`.
    Op14,
    /// 0x16 — always true; OR `0x04` into the high flag byte.
    Op16,
    /// 0x17 — `+0x51 != 0` (a prev-chain pointer exists).
    Op17,
    /// 0x18 — the prev-chain walk (`+0x51`) reaches the current row.
    Op18,
    /// 0x19 — `+0xD7 & 2`.
    Op19,
    /// 0x1C — the next-chain walk reaches the prev row (mirrors `Op18`).
    Op1C,
    /// 0x1D — `!(+0xD7 & 2)`.
    Op1D,
    /// 0x1E — `+0xA4 → *[+0xA4] → +0x4C != 0` (approximation via
    /// `PredRow::a4_idx` + `a4_4c`).
    Op1E,
    /// 0x23 — `prev[+0xD9] < cur[+0xD9]`.
    Op23,
    /// 0x24 — `+200 == 0`.
    Op24,
    /// 0x1B — always true; OR `0x08` into the high flag byte.
    Op1B,
}

/// The null marker for the in-memory row pool (a `PRED_NULL` pointer field
/// is an absent / terminated link).
pub const PRED_NULL: u32 = u32::MAX;

/// The in-memory 224+ B predicate row (the fields the 22 opcodes read).
/// The on-disk 16 B `v0.26.0` word rows are a *different* encoding; the
/// full row is allocated by the DLL. Pointer fields are pool indices.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PredRow {
    /// `+0x51` — the prev-chain pointer (pool index; `Op17`/`Op18`/`Op23`).
    pub prev_idx: u32,
    /// The next-row pointer (pool index; `Op5`).
    pub next_idx: u32,
    /// `+0xA4` — the pointer-to-pointer (pool index; `Op1E`).
    pub a4_idx: u32,
    /// `+0xD7` — the low flag byte.
    pub d7: u8,
    /// `+0xD8` — the second flag byte.
    pub d8: u8,
    /// `+0xD9` — the per-row counter (`Op5`/`Op23`).
    pub d9: u8,
    /// `+0xDD` — the unresolved-accessor operand (`Op0`/`Op1`).
    pub dd: u8,
    /// `+200` — the field `Op24` tests (`== 0`).
    pub plus200: u32,
    /// `H[+8]` — the header byte `Op6` tests (`== 1`; structural proxy).
    pub header8: u8,
    /// `*(*(R+0xA4)) + 0x4C` — the final `+0x4C` byte for `Op1E`.
    pub a4_4c: u8,
}

impl PredRow {
    /// A fully-populated synthetic row (every pointer field nulled).
    pub fn new() -> Self {
        PredRow {
            prev_idx: PRED_NULL,
            next_idx: PRED_NULL,
            a4_idx: PRED_NULL,
            d7: 0,
            d8: 0,
            d9: 0,
            dd: 0,
            plus200: 0,
            header8: 0,
            a4_4c: 0,
        }
    }
}

/// A catalog row (opcode → semantics, UA + EN, evaluable flag).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PredCatalogEntry {
    pub op: PredOp,
    /// The raw on-disk opcode value.
    pub raw: u32,
    pub en: &'static str,
    pub ua: &'static str,
    /// Whether [`pred_evaluate`] can decide this predicate (the two
    /// `FUN_10011580`/`FUN_10011600` accessors are `false`).
    pub evaluable: bool,
}

/// The complete 22-opcode catalog (sorted by raw opcode).
pub const PRED_CATALOG: &[PredCatalogEntry] = &[
    PredCatalogEntry { op: PredOp::Op0,  raw: 0x00, en: "unresolved: FUN_10011600(+0xDD)&0x80==0 → set low 0x80", ua: "нерозв'язано: FUN_10011600(+0xDD)&0x80==0 → low |= 0x80", evaluable: false },
    PredCatalogEntry { op: PredOp::Op1,  raw: 0x01, en: "unresolved: FUN_10011580(idx>>8,+0xDD)&0x40!=0 → set high 0x01", ua: "нерозв'язано: FUN_10011580(idx>>8,+0xDD)&0x40!=0 → high |= 0x01", evaluable: false },
    PredCatalogEntry { op: PredOp::Op2,  raw: 0x02, en: "always true; low |= 0x0C", ua: "завжди true; low |= 0x0C", evaluable: true },
    PredCatalogEntry { op: PredOp::Op5,  raw: 0x05, en: "next row +0xD9 == cur +0xD9 + 1 (not last row)", ua: "next-рядок +0xD9 == cur +0xD9 + 1 (не останній)", evaluable: true },
    PredCatalogEntry { op: PredOp::Op6,  raw: 0x06, en: "header H[+8] == 1 (structural: row.header8)", ua: "заголовок H[+8] == 1 (структурно: row.header8)", evaluable: true },
    PredCatalogEntry { op: PredOp::Op7,  raw: 0x07, en: "!(+0xD7 & 8)", ua: "!(+0xD7 & 8)", evaluable: true },
    PredCatalogEntry { op: PredOp::Op8,  raw: 0x08, en: "always true; low |= 0x02", ua: "завжди true; low |= 0x02", evaluable: true },
    PredCatalogEntry { op: PredOp::Op0B, raw: 0x0B, en: "!(+0xD7 & 0x10)", ua: "!(+0xD7 & 0x10)", evaluable: true },
    PredCatalogEntry { op: PredOp::Op0C, raw: 0x0C, en: "always true; low |= 0x28", ua: "завжди true; low |= 0x28", evaluable: true },
    PredCatalogEntry { op: PredOp::Op0D, raw: 0x0D, en: "always true; low |= 0x40", ua: "завжди true; low |= 0x40", evaluable: true },
    PredCatalogEntry { op: PredOp::Op10, raw: 0x10, en: "always true; high |= 0x02", ua: "завжди true; high |= 0x02", evaluable: true },
    PredCatalogEntry { op: PredOp::Op14, raw: 0x14, en: "+0xD8 & 1", ua: "+0xD8 & 1", evaluable: true },
    PredCatalogEntry { op: PredOp::Op16, raw: 0x16, en: "always true; high |= 0x04", ua: "завжди true; high |= 0x04", evaluable: true },
    PredCatalogEntry { op: PredOp::Op17, raw: 0x17, en: "+0x51 != 0 (a prev-chain link exists)", ua: "+0x51 != 0 (є prev-ланка)", evaluable: true },
    PredCatalogEntry { op: PredOp::Op18, raw: 0x18, en: "prev-chain walk (+0x51) reaches the current row", ua: "prev-ланцюг (+0x51) доходить до поточного рядка", evaluable: true },
    PredCatalogEntry { op: PredOp::Op19, raw: 0x19, en: "+0xD7 & 2", ua: "+0xD7 & 2", evaluable: true },
    PredCatalogEntry { op: PredOp::Op1C, raw: 0x1C, en: "next-chain walk reaches the prev row (mirrors Op18)", ua: "next-ланцюг доходить до prev-рядка (дзеркало Op18)", evaluable: true },
    PredCatalogEntry { op: PredOp::Op1D, raw: 0x1D, en: "!(+0xD7 & 2)", ua: "!(+0xD7 & 2)", evaluable: true },
    PredCatalogEntry { op: PredOp::Op1E, raw: 0x1E, en: "+0xA4 → *[+0xA4] → +0x4C != 0 (structural: a4_idx + a4_4c)", ua: "+0xA4 → *[+0xA4] → +0x4C != 0 (структурно: a4_idx + a4_4c)", evaluable: true },
    PredCatalogEntry { op: PredOp::Op23, raw: 0x23, en: "prev[+0xD9] < cur[+0xD9]", ua: "prev[+0xD9] < cur[+0xD9]", evaluable: true },
    PredCatalogEntry { op: PredOp::Op24, raw: 0x24, en: "+200 == 0", ua: "+200 == 0", evaluable: true },
    PredCatalogEntry { op: PredOp::Op1B, raw: 0x1B, en: "always true; high |= 0x08", ua: "завжди true; high |= 0x08", evaluable: true },
];

/// Decode a raw on-disk opcode into a [`PredOp`] (None for unknown).
pub fn pred_from_opcode(raw: u32) -> Option<PredOp> {
    PRED_CATALOG
        .iter()
        .find(|e| e.raw == raw)
        .map(|e| e.op)
}

/// The catalog size (the DoD "catalog size" the `trident` verb reports).
pub const fn pred_catalog_size() -> usize {
    PRED_CATALOG.len()
}

/// How many of the 22 are evaluable by [`pred_evaluate`] (20 — the two
/// unresolved `FUN_10011580`/`FUN_10011600` accessors are not).
pub const fn pred_evaluable_count() -> usize {
    20
}

/// The flag bits a predicate ORs into the `DAT_1002f41c` flag area when it
/// returns true: `(low byte, high byte)`. The low byte is at stride 7, the
/// high byte at stride `0xE + 1` (both relative to `DAT_1002f41c`).
pub fn pred_flag_bits(op: PredOp) -> (u8, u8) {
    use PredOp::*;
    match op {
        Op2 => (0x0C, 0),
        Op8 => (0x02, 0),
        Op0C => (0x28, 0),
        Op0D => (0x40, 0),
        Op0 => (0x80, 0), // conditional (only when the accessor returns 1)
        Op10 => (0, 0x02),
        Op16 => (0, 0x04),
        Op1B => (0, 0x08),
        Op1 => (0, 0x01), // conditional
        _ => (0, 0),
    }
}

/// Evaluate the evaluable subset of `FUN_100177d0` against a row pool.
///
/// `pool` is the in-memory row array; `idx` is the current row. The two
/// unresolved accessors ([`PredOp::Op0`]/[`PredOp::Op1`]) return `false`
/// (they need the `.g`/template decode). The row-walk arms (`Op18`/`Op1C`)
/// are structural approximations: they walk the `prev_idx`/`next_idx`
/// pool links.
pub fn pred_evaluate(op: PredOp, pool: &[PredRow], idx: usize) -> bool {
    if idx >= pool.len() {
        return false;
    }
    let row = &pool[idx];
    use PredOp::*;
    match op {
        // always-true flag setters
        Op2 | Op8 | Op0C | Op0D | Op10 | Op16 | Op1B => true,
        // field checks
        Op5 => {
            row.next_idx != PRED_NULL
                && idx + 1 < pool.len()
                && pool[row.next_idx as usize]
                    .d9
                    .wrapping_sub(row.d9)
                    == 1
        }
        Op6 => row.header8 == 1,
        Op7 => row.d7 & 8 == 0,
        Op0B => row.d7 & 0x10 == 0,
        Op14 => row.d8 & 1 != 0,
        Op17 => row.prev_idx != PRED_NULL,
        Op19 => row.d7 & 2 != 0,
        Op1D => row.d7 & 2 == 0,
        Op23 => idx > 0 && row.prev_idx != PRED_NULL && pool[row.prev_idx as usize].d9 < row.d9,
        Op24 => row.plus200 == 0,
        // row walks (structural approximations)
        Op18 => {
            if idx == 0 {
                return true; // param_2 == 0: the guard is skipped
            }
            let mut cur = row.prev_idx;
            if cur == PRED_NULL {
                return false;
            }
            loop {
                if cur == idx as u32 {
                    return true;
                }
                cur = pool[cur as usize].prev_idx;
                if cur == PRED_NULL {
                    return false;
                }
            }
        }
        Op1C => {
            if idx == 0 {
                return true;
            }
            let target = row.prev_idx;
            let mut cur = row.next_idx;
            if cur == PRED_NULL {
                return false;
            }
            loop {
                if cur == target {
                    return true;
                }
                cur = pool[cur as usize].next_idx;
                if cur == PRED_NULL {
                    return false;
                }
            }
        }
        Op1E => row.a4_idx != PRED_NULL && row.a4_4c != 0,
        // unresolved accessors (FUN_10011580 / FUN_10011600)
        Op0 | Op1 => false,
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
        let h = VmHeader::parse(&m).expect("header");
        assert_eq!(h.header.name, "lmUR.m");
        assert_eq!(h.header.tail, 0x70E, "tail section start");
        assert_eq!(
            h.table,
            vec![
                0x1000, 0x88, 0x0BCB, 0x726, 0xA1, 0x206D, 0x244B, 0x29D7, 0x3000,
                0x339D, 0x36EF, 0x37CB, 0x39C9, 0xBF3, 0xBFC, 0x1405
            ],
            "the full 16-slot u32 table (v0.21.0)"
        );
        // the pinned slot roles (v0.21.0)
        assert_eq!(h.pred_off(), Some(0x206D), "slot 5 = u16 predicate table");
        assert_eq!(h.word_off(), Some(0x37CB), "slot 11 = u16 word table");
        // the cross-language header invariants
        assert_eq!(h.u84, 0x206D - 7, "u32@0x84 = pred_off - 7");
        assert_eq!(h.u80, 0x206D - 19, "u32@0x80 = pred_off - 19");
        // the .m tail N-section IS non-empty (unlike the .vt tail)
        let ts = h.tail.as_ref().expect("the .m tail N-section");
        assert_eq!(ts.count, 3);
        assert_eq!(ts.records.len(), 3);
        assert_eq!(ts.records[0].key, *b"R");
        assert_eq!(ts.records[0].list, vec![0x1]);
        assert_eq!(ts.records[1].key, *b"N*");
        assert_eq!(ts.records[1].list, vec![0x1C, 0x3E]);
        assert_eq!(ts.records[2].key, *b"*");
        assert_eq!(ts.records[2].list, vec![0x3E, 0x14]);
    }

    #[test]
    fn word_table_section_is_the_count_plus_offsets_shape() {
        // v0.21.0: the u16 word table (slot 11) is a `[u16 count]` +
        // `count` relative u16 offset array that exactly fills the
        // section up to the slot-12 pointer.
        let Some(m) = real(r"C:\Zakon.Local\Baza\SOFT\lmur.m") else {
            eprintln!("skipped: no base");
            return;
        };
        let h = VmHeader::parse(&m).expect("header");
        let word = h.word_off().expect("word table");
        let next = h.table[12];
        let count = u16::from_le_bytes([m[word as usize], m[word as usize + 1]]);
        assert_eq!(count, 0xFE, "ukr word-table count (254)");
        let extent = next as usize - word as usize;
        assert_eq!(extent, (1 + count as usize) * 2, "the section fills exactly");
        // the offsets are ascending (a relative record pool)
        let mut off = word as usize + 2;
        let mut prev = 0u16;
        for _ in 0..16 {
            let o = u16::from_le_bytes([m[off], m[off + 1]]);
            assert!(o > prev || o < 0x0100, "ascending offsets");
            prev = o;
            off += 2;
        }
        assert_eq!(
            u16::from_le_bytes([m[off - 2], m[off - 1]]),
            0x011E,
            "first 16 offsets step by 2 from 0x0100 (16th = 0x011E)"
        );
    }

    #[test]
    fn pred_table_is_a_plain_ascending_u16_array() {
        let Some(m) = real(r"C:\Zakon.Local\Baza\SOFT\lmur.m") else {
            eprintln!("skipped: no base");
            return;
        };
        let h = VmHeader::parse(&m).expect("header");
        let pred = h.pred_off().expect("predicate table");
        let first: Vec<u16> = (0..4)
            .map(|i| {
                u16::from_le_bytes([
                    m[pred as usize + i * 2],
                    m[pred as usize + i * 2 + 1],
                ])
            })
            .collect();
        assert_eq!(first, vec![0x013E, 0x0154, 0x0164, 0x0170]);
    }

    #[test]
    fn word_table_parses_the_real_lmur_m() {
        let Some(m) = real(r"C:\Zakon.Local\Baza\SOFT\lmur.m") else {
            eprintln!("skipped: no base");
            return;
        };
        let w = VmWordTable::parse(&m, 0x37CB).expect("ukr word table");
        assert_eq!(w.count, 0xFE, "the ukr dictionary word count");
        assert_eq!(w.records.len(), 254);
        // the first half: ascending offset-like records (step 2)
        assert_eq!(
            &w.records[..8],
            &[0x0100, 0x0102, 0x0104, 0x0106, 0x0108, 0x010A, 0x010C, 0x010E]
        );
        assert_eq!(w.offset_like_count(), 126, "the offset-like half");
        assert_eq!(w.last_offset(), Some(0x01FA));
        // the i16 half: the 0xFFFA kind markers + the boundary record
        assert_eq!(w.kind_fa_count(), 61);
        assert_eq!(w.boundary(), Some(0x8000));
    }

    #[test]
    fn word_table_parses_the_real_lmru_m() {
        let Some(m) = real(r"C:\Zakon.Local\Baza\SOFT\lmru.m") else {
            eprintln!("skipped: no base");
            return;
        };
        let w = VmWordTable::parse(&m, 0x3001).expect("rus word table");
        assert_eq!(w.count, 0x100, "the rus dictionary word count");
        assert_eq!(
            &w.records[..8],
            &[0x0102, 0x0104, 0x0106, 0x0108, 0x010A, 0x010C, 0x010E, 0x0110]
        );
        assert_eq!(w.offset_like_count(), 128, "the rus split is even");
        assert_eq!(w.kind_fa_count(), 64);
        assert_eq!(w.boundary(), Some(0x7F00));
    }

    #[test]
    fn pred_table_parses_the_real_lmur_m() {
        let Some(m) = real(r"C:\Zakon.Local\Baza\SOFT\lmur.m") else {
            eprintln!("skipped: no base");
            return;
        };
        let p = VmPredicateTable::parse(&m, 0x206D).expect("ukr predicate");
        assert_eq!(p.offsets.len(), 161, "160 offsets + the sentinel");
        assert_eq!(
            &p.offsets[..8],
            &[0x013E, 0x0154, 0x0164, 0x0170, 0x0173, 0x0177, 0x017B, 0x017F]
        );
        assert_eq!(p.sentinel, 0xCECA, "the run-ending sentinel");
        assert_eq!(p.offset_count(), 160);
        assert_eq!(p.record_sample[0], 0x16, "the first packed record byte");
    }

    #[test]
    fn pred_table_parses_the_real_lmru_m() {
        let Some(m) = real(r"C:\Zakon.Local\Baza\SOFT\lmru.m") else {
            eprintln!("skipped: no base");
            return;
        };
        let p = VmPredicateTable::parse(&m, 0x1ACF).expect("rus predicate");
        assert_eq!(p.offsets.len(), 169, "168 offsets + the sentinel");
        assert_eq!(
            &p.offsets[..8],
            &[0x014E, 0x0164, 0x016D, 0x0173, 0x0189, 0x019B, 0x01A7, 0x01AA]
        );
        assert_eq!(p.sentinel, 0xCECA, "the same sentinel in rus");
        assert_eq!(p.offset_count(), 168);
    }

    #[test]
    fn u16_tables_yield_none_on_truncated_buffers() {
        // the word count overruns the buffer (count=10, 2 records fit)
        let mut b = vec![10u8, 0u8];
        b.extend_from_slice(&[0u8; 4]);
        assert!(VmWordTable::parse(&b, 0).is_none());
        assert!(VmWordTable::parse(&[0u8; 1], 0).is_none());
        // the predicate walk needs at least 8 bytes
        assert!(VmPredicateTable::parse(&[0u8; 4], 0).is_none());
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
        assert!(VmHeader::parse(&[0u8; 64]).is_none());
        assert!(VmHeader::parse(&[0u8; 0x80]).is_none());
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
        let hm = VmHeader::parse(&m).expect("rus .m header");
        assert_eq!(hm.header.name, "lmRU.m");
        assert_eq!(
            hm.table,
            vec![
                0x0C20, 0x88, 0xBD8, 0x56B, 0x99, 0x1ACF, 0x2000, 0x25C1, 0x28AD,
                0x2C0F, 0x2F17, 0x3001, 0x3203, 0xC09, 0xC12, 0xF5D
            ],
            "the full 16-slot u32 table (v0.21.0)"
        );
        // the same pinned slot roles as the Ukrainian file
        assert_eq!(hm.pred_off(), Some(0x1ACF), "slot 5 = u16 predicate table");
        assert_eq!(hm.word_off(), Some(0x3001), "slot 11 = u16 word table");
        // the cross-language header invariant holds for rus too
        assert_eq!(hm.u84, 0x1ACF - 7, "u32@0x84 = pred_off - 7");
        assert_eq!(hm.u80, 0x1ACF - 19, "u32@0x80 = pred_off - 19");
        // the rus tail N-section has 4 records (adds "*N")
        let ts = hm.tail.as_ref().expect("the rus .m tail N-section");
        assert_eq!(ts.count, 4);
        assert_eq!(ts.records[0].key, *b"R");
        assert_eq!(ts.records[0].list, vec![0x8]);
        assert_eq!(ts.records[1].key, *b"N*");
        assert_eq!(ts.records[1].list, vec![0x24, 0x0E, 0x26, 0x1D, 0x3E]);
        assert_eq!(ts.records[2].key, *b"*N");
        assert_eq!(ts.records[2].list, vec![0x24, 0x0E, 0x26, 0x1D, 0x3E]);
        assert_eq!(ts.records[3].key, *b"*");
        assert_eq!(ts.records[3].list, vec![0x3E, 0x1D]);
    }

    // ------------------------------------------------------------------
    // v0.26.0: the `.m` section walker (roadmap v0.27.0)
    // ------------------------------------------------------------------

    #[test]
    fn m_sections_walk_classify_the_real_lmur_m() {
        let Some(m) = real(r"C:\Zakon.Local\Baza\SOFT\lmur.m") else {
            eprintln!("skipped: no base");
            return;
        };
        let hm = VmHeader::parse(&m).expect("ukr .m header");
        let secs = VmSections::parse(&hm, &m);
        assert_eq!(secs.walks.len(), 16, "every table slot is walked");
        // the DoD tiling assert: exactly the u16 word tables tile the
        // file (the packed B-tree regions interleave and land at the
        // region end / file tail)
        assert_eq!(secs.tiling_count(), 1, "one exactly-tiling section");
        // slot 11 = the u16 word table: [u16 count=254][254 u16] ending
        // exactly on the next slot (0x37CB → 0x39C9)
        let w11 = secs.walk(11).expect("slot 11 walked");
        assert_eq!(w11.kind, VmSectionKind::U16Records);
        assert!(w11.lands_on_next, "the word table tiles the file");
        assert_eq!(w11.off, 0x37CB);
        assert_eq!(w11.records, 254);
        assert_eq!(w11.landed, 0x39C9);
        // slot 0 = the per-word 16-byte rows (64 rows + 5 B leftover)
        let w0 = secs.walk(0).expect("slot 0 walked");
        assert_eq!(w0.kind, VmSectionKind::WordRows);
        assert_eq!(w0.records, 64);
        let rows = secs.word_rows.as_ref().expect("the word rows decoded");
        assert_eq!(rows.base, 0x1000);
        assert_eq!(rows.rows.len(), 64, "64×16B rows in the 1029 B span");
        assert_eq!(rows.leftover, 5);
        assert_eq!(rows.nonzero_rows, 64, "every 16B row carries data");
        // slot 7 = the (len, flag) key directory
        let w7 = secs.walk(7).expect("slot 7 walked");
        assert_eq!(w7.kind, VmSectionKind::LenFlagIndex);
        assert!(w7.records > 0, "the key pairs are counted");
        // the 0x88 registry (v0.26.0 empirical: a u32 config block, not a
        // NUL-terminated string section)
        let reg = secs.registry.as_ref().expect("the 0x88 registry decoded");
        assert_eq!(reg.base, 0x88);
        assert_eq!(reg.u88, 0x00FFE703, "the 0x00FFxx marker u32");
        assert_eq!(reg.sub_count(), 3, "the marker's low byte (3 ukr)");
        assert_eq!(reg.offsets[0], 0x1900, "the first u32 region offset");
        // the kind histogram sanity (16 = the sum over all kinds)
        let sum: u32 = secs
            .walks
            .iter()
            .map(|w| match w.kind {
                VmSectionKind::U16Records
                | VmSectionKind::NulRecords
                | VmSectionKind::WordRows
                | VmSectionKind::LenFlagIndex
                | VmSectionKind::Small
                | VmSectionKind::Packed
                | VmSectionKind::Unknown => 1,
            })
            .sum();
        assert_eq!(sum, 16);
    }

    #[test]
    fn m_sections_walk_classify_the_real_lmru_m() {
        let Some(m) = real(r"C:\Zakon.Local\Baza\SOFT\lmru.m") else {
            eprintln!("skipped: no base");
            return;
        };
        let hm = VmHeader::parse(&m).expect("rus .m header");
        let secs = VmSections::parse(&hm, &m);
        assert_eq!(secs.walks.len(), 16, "every table slot is walked");
        assert_eq!(secs.tiling_count(), 1, "one exactly-tiling section");
        // slot 11 = the rus u16 word table (256 records, 0x3001 → 0x3203)
        let w11 = secs.walk(11).expect("slot 11 walked");
        assert_eq!(w11.kind, VmSectionKind::U16Records);
        assert!(w11.lands_on_next, "the rus word table tiles the file");
        assert_eq!(w11.off, 0x3001);
        assert_eq!(w11.records, 256);
        assert_eq!(w11.landed, 0x3203);
        // slot 0 = the rus per-word rows (51 rows + 13 B leftover)
        let rows = secs.word_rows.as_ref().expect("the rus word rows decoded");
        assert_eq!(rows.base, 0xC20);
        assert_eq!(rows.rows.len(), 51, "51×16B rows in the 829 B span");
        assert_eq!(rows.leftover, 13);
        // slot 7 = the rus key directory (the u16 offset run + the pairs)
        let w7 = secs.walk(7).expect("slot 7 walked");
        assert_eq!(w7.kind, VmSectionKind::LenFlagIndex);
        // the rus 0x88 registry
        let reg = secs.registry.as_ref().expect("the 0x88 registry decoded");
        assert_eq!(reg.u88, 0x00FFEF02, "the rus marker u32");
        assert_eq!(reg.sub_count(), 2, "the marker's low byte (2 rus)");
        assert_eq!(reg.offsets[0], 0x1100, "the first u32 region offset");
    }

    #[test]
    fn vm_word_rows_parse_a_synthetic_block() {
        // 3 full 16-byte rows + 4 leftover bytes
        let mut buf = vec![0u8; 64 + 52 + 4];
        buf[64 + 0] = 0x2A; // row 0, a non-zero byte
        buf[64 + 16 + 12] = 0xA3; // row 1, the +0x0C field
        buf[64 + 32 + 12] = 0xA4; // row 2, the +0x0C field
        let r = VmWordRows::parse(&buf, 64, 64 + 52).expect("rows");
        assert_eq!(r.rows.len(), 3);
        assert_eq!(r.leftover, 4);
        assert_eq!(r.nonzero_rows, 3);
        let hist: std::collections::HashMap<u8, u32> =
            r.field_0c_hist.iter().cloned().collect();
        assert_eq!(hist.get(&0x00), Some(&1));
        assert_eq!(hist.get(&0xA3), Some(&1));
        assert_eq!(hist.get(&0xA4), Some(&1));
        // a span shorter than one row yields None
        assert!(VmWordRows::parse(&buf, 64, 70).is_none());
    }

    #[test]
    fn vm_field_registry_requires_the_marker() {
        // a valid 0x88 block: the 0x00FFxx marker + five u32 offsets
        let mut buf = vec![0u8; 64 + 0x88 + 0x22];
        buf[0x88..0x8C].copy_from_slice(&0x00FFE703u32.to_le_bytes());
        buf[0x8C..0x90].copy_from_slice(&0x1900u32.to_le_bytes());
        let r = VmFieldRegistry::parse(&buf, 0x88).expect("registry");
        assert_eq!(r.u88, 0x00FFE703);
        assert_eq!(r.sub_count(), 3);
        assert_eq!(r.offsets[0], 0x1900);
        // a missing 0xFF high word is rejected
        let mut bad = buf.clone();
        bad[0x88..0x8C].copy_from_slice(&0x12345678u32.to_le_bytes());
        assert!(VmFieldRegistry::parse(&bad, 0x88).is_none());
    }

    #[test]
    fn vm_sections_classify_a_synthetic_u16_tiling_table() {
        // a minimal synthetic `.m`: the 64-byte header (FREE marker) + a
        // 16-slot table where slot 0 points at a u16 record table that
        // ends exactly on the next slot (the tiling case).
        let mut buf = vec![0u8; 64 + 0x200];
        buf[0x38..0x3C].copy_from_slice(b"FREE");
        // table @0x40: slot 0 = 0x100 (a u16 table), slot 1 = 0x108
        buf[0x40..0x44].copy_from_slice(&0x100u32.to_le_bytes());
        buf[0x44..0x48].copy_from_slice(&0x108u32.to_le_bytes());
        // the u16 table @0x100: [u16 count=3][3 u16 records] = 8 B,
        // ending exactly at 0x108 (the next slot)
        buf[0x100..0x102].copy_from_slice(&3u16.to_le_bytes());
        for i in 0..3 {
            buf[0x102 + i * 2..0x104 + i * 2]
                .copy_from_slice(&(i as u16).to_le_bytes());
        }
        let hm = VmHeader::parse(&buf).expect("synthetic header");
        let secs = VmSections::parse(&hm, &buf);
        assert_eq!(secs.tiling_count(), 1, "the u16 table tiles");
        let w0 = secs.walk(0).expect("slot 0 walked");
        assert_eq!(w0.kind, VmSectionKind::U16Records);
        assert!(w0.lands_on_next);
        assert_eq!(w0.records, 3);
        assert_eq!(w0.landed, 0x108);
    }

    // ------------------------------------------------------------------
    // v0.27.0: the `FUN_100177d0` predicate catalog (roadmap v0.28.0)
    // ------------------------------------------------------------------

    #[test]
    fn pred_catalog_covers_all_22_opcodes() {
        assert_eq!(pred_catalog_size(), 22);
        assert_eq!(pred_evaluable_count(), 20);
        // every raw opcode decodes to its variant and round-trips
        for e in PRED_CATALOG {
            assert_eq!(pred_from_opcode(e.raw), Some(e.op));
        }
        // the two unresolved accessors are the only non-evaluable arms
        let uneval: Vec<&PredCatalogEntry> =
            PRED_CATALOG.iter().filter(|e| !e.evaluable).collect();
        assert_eq!(uneval.len(), 2);
        assert_eq!(uneval[0].op, PredOp::Op0);
        assert_eq!(uneval[1].op, PredOp::Op1);
        // opcodes not in the catalog are unknown
        assert!(pred_from_opcode(3).is_none());
        assert!(pred_from_opcode(0x25).is_none());
    }

    #[test]
    fn pred_flag_bits_match_the_decompiled_arms() {
        assert_eq!(pred_flag_bits(PredOp::Op2), (0x0C, 0));
        assert_eq!(pred_flag_bits(PredOp::Op8), (0x02, 0));
        assert_eq!(pred_flag_bits(PredOp::Op0C), (0x28, 0));
        assert_eq!(pred_flag_bits(PredOp::Op0D), (0x40, 0));
        assert_eq!(pred_flag_bits(PredOp::Op0), (0x80, 0));
        assert_eq!(pred_flag_bits(PredOp::Op10), (0, 0x02));
        assert_eq!(pred_flag_bits(PredOp::Op16), (0, 0x04));
        assert_eq!(pred_flag_bits(PredOp::Op1B), (0, 0x08));
        assert_eq!(pred_flag_bits(PredOp::Op1), (0, 0x01));
        // field-check opcodes carry no side-effect bits
        assert_eq!(pred_flag_bits(PredOp::Op19), (0, 0));
        assert_eq!(pred_flag_bits(PredOp::Op5), (0, 0));
    }

    #[test]
    fn pred_evaluate_flag_setters_are_always_true() {
        let pool = vec![PredRow::new()];
        for op in [
            PredOp::Op2,
            PredOp::Op8,
            PredOp::Op0C,
            PredOp::Op0D,
            PredOp::Op10,
            PredOp::Op16,
            PredOp::Op1B,
        ] {
            assert!(pred_evaluate(op, &pool, 0));
        }
    }

    #[test]
    fn pred_evaluate_field_checks() {
        // a row with only d7 bit1 set: 0x02
        let mut a = PredRow::new();
        a.d7 = 0x02;
        let mut b = PredRow::new(); // all zeros
        // Op19 (+0xD7&2) true on `a`, Op1D (!+0xD7&2) true on `b`
        assert!(pred_evaluate(PredOp::Op19, &pool1(&a), 0));
        assert!(!pred_evaluate(PredOp::Op19, &pool1(&b), 0));
        assert!(pred_evaluate(PredOp::Op1D, &pool1(&b), 0));
        assert!(!pred_evaluate(PredOp::Op1D, &pool1(&a), 0));
        // Op7 / Op0B: d7 bit3 / bit4 clear → true on `b`
        assert!(pred_evaluate(PredOp::Op7, &pool1(&b), 0));
        assert!(pred_evaluate(PredOp::Op0B, &pool1(&b), 0));
        // Op14: d8 bit0
        let mut c = PredRow::new();
        c.d8 = 0x01;
        assert!(pred_evaluate(PredOp::Op14, &pool1(&c), 0));
        assert!(!pred_evaluate(PredOp::Op14, &pool1(&b), 0));
        // Op17: a prev link exists
        let mut d = PredRow::new();
        d.prev_idx = 0;
        assert!(pred_evaluate(PredOp::Op17, &pool1(&d), 0));
        assert!(!pred_evaluate(PredOp::Op17, &pool1(&b), 0));
        // Op24: +200 == 0 (default)
        assert!(pred_evaluate(PredOp::Op24, &pool1(&b), 0));
        let mut e = PredRow::new();
        e.plus200 = 7;
        assert!(!pred_evaluate(PredOp::Op24, &pool1(&e), 0));
        // Op6: header8 == 1
        let mut f = PredRow::new();
        f.header8 = 1;
        assert!(pred_evaluate(PredOp::Op6, &pool1(&f), 0));
        assert!(!pred_evaluate(PredOp::Op6, &pool1(&b), 0));
    }

    // a single-row pool around one row
    fn pool1(r: &PredRow) -> Vec<PredRow> {
        vec![*r]
    }

    #[test]
    fn pred_evaluate_counter_ops() {
        // Op5: the next row's d9 is exactly cur+1 (and not the last row)
        let mut cur = PredRow::new();
        cur.d9 = 5;
        cur.next_idx = 1;
        let mut nxt = PredRow::new();
        nxt.d9 = 6;
        let pool = vec![cur, nxt];
        assert!(pred_evaluate(PredOp::Op5, &pool, 0));
        // a Δ of 2 is not 1
        let mut pool2 = pool.clone();
        pool2[1].d9 = 7;
        assert!(!pred_evaluate(PredOp::Op5, &pool2, 0));
        // Op23: prev[+0xD9] < cur[+0xD9]
        let mut prev = PredRow::new();
        prev.d9 = 3;
        let mut c2 = PredRow::new();
        c2.d9 = 4;
        c2.prev_idx = 0;
        let pool3 = vec![prev, c2];
        assert!(pred_evaluate(PredOp::Op23, &pool3, 1));
        // equal counters are not `<`
        let mut pool4 = pool3.clone();
        pool4[0].d9 = 4;
        assert!(!pred_evaluate(PredOp::Op23, &pool4, 1));
        // idx 0 has no prev row → Op23 is false
        assert!(!pred_evaluate(PredOp::Op23, &pool3, 0));
    }

    #[test]
    fn pred_evaluate_pointer_walks() {
        // Op1E: +0xA4 → *[+0xA4] → +0x4C != 0
        let mut r = PredRow::new();
        r.a4_idx = 5;
        r.a4_4c = 0;
        assert!(!pred_evaluate(PredOp::Op1E, &pool1(&r), 0));
        r.a4_4c = 1;
        assert!(pred_evaluate(PredOp::Op1E, &pool1(&r), 0));
        r.a4_idx = PRED_NULL;
        assert!(!pred_evaluate(PredOp::Op1E, &pool1(&r), 0));
        // Op18: the prev chain (a 2-row mutual loop) reaches the current row
        let mut a = PredRow::new();
        a.prev_idx = 1;
        let mut b = PredRow::new();
        b.prev_idx = 0;
        let pool = vec![a, b];
        assert!(pred_evaluate(PredOp::Op18, &pool, 1));
        // Op18 at idx 0 skips the guard → true
        assert!(pred_evaluate(PredOp::Op18, &pool, 0));
        // a broken chain (a head with a NULL link) does not reach
        let mut c = PredRow::new();
        c.prev_idx = 1;
        let mut d = PredRow::new();
        d.prev_idx = 0; // and row0.prev_idx is NULL → terminates
        let pool2 = vec![c, d]; // c.prev_idx=1, d.prev_idx=0, c has prev 1...
        // from idx1: cur=d, prev_idx=0; walk: 0==1?no, c.prev_idx=1; 1==1 → true
        assert!(pred_evaluate(PredOp::Op18, &pool2, 1));
        // Op1C: the next chain reaches the prev row
        let mut x = PredRow::new();
        x.next_idx = 1;
        let mut y = PredRow::new();
        y.prev_idx = 0;
        y.next_idx = 0;
        let pool3 = vec![x, y];
        assert!(pred_evaluate(PredOp::Op1C, &pool3, 1));
    }

    #[test]
    fn pred_evaluate_unresolved_accessors_return_false() {
        let pool = vec![PredRow::new()];
        assert!(!pred_evaluate(PredOp::Op0, &pool, 0));
        assert!(!pred_evaluate(PredOp::Op1, &pool, 0));
    }
}
