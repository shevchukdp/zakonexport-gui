//! Exact `.voc` section-table + opcode-VM decode (the lmsup32.dll engine's
//! data structures), built byte-for-byte from the decompiled loader
//! (`FUN_10004230` section loader, `FUN_100044f0`/`FUN_100044c0`/
//! `FUN_100044a0` opcode-VM length functions) and confirmed against the real
//! `lmu.voc`.
//!
//! ## The section table
//!
//! A `.voc` file's header is a 200-byte image of the in-memory handle struct.
//! At file offset `0x3C` there is a table of **17 little-endian u32 section
//! pointers** (absolute file offsets, `0` = absent). The loader reads the
//! struct fields `+0x40…+0x7C` (which mirror these file bytes 1:1).
//!
//! The last three entries (`+0x74`, `+0x78`, `+0x7C`) are **dual-encoded**:
//! the low u16 is a *record count* and the high u16 is a *section selector*;
//! the u32 is a valid offset only when the section actually exists.
//!
//! ```text
//!   +0x3C  root u16 index table
//!   +0x40  listA  (packed per-word records, NUL-terminated)
//!   +0x44  listB  (packed per-word records, NUL-terminated)
//!   +0x48  per-word 'f' records (opcode-VM programs)
//!   +0x4C  morphological suffix / endings table (LmCode words)
//!   +0x50 … +0x70  more tables (words, digraphs, affixes)
//!   +0x74  (count, section) — low u16 = listA count
//!   +0x78  (count, section)
//!   +0x7C  (count, section) — low u16 = digraph count
//! ```
//!
//! ## The opcode VM
//!
//! The per-word `'f'` records are little-endian **u16 instruction programs**
//! executed by `FUN_100045e0`:
//!
//! - `0x1001` — enter a nested subtree (recurse, then skip via
//!   `FUN_100044f0` and add the subtree's word count);
//! - `0x1002` — terminator (top-level returns the program length);
//! - `0x1003` — word leaf (word counter + 1);
//! - `0x1004 [u16 ptr]` — child node (via a resolver; `child[0]` = the child
//!   word count);
//! - any other u16 — inline data collected into an output buffer.
//!
//! The root index table (`+0x3C`) is **not** a `0x1001`/`0x1002` program:
//! in `lmu.voc` it is a packed **B-tree root** — a u16 stream of child-node
//! ids and per-subtree word counts. The search chain (decompiled,
//! `lmsup32.dll`) is `FUN_10002ff0` (seek to the root at `handle+0x3C`) →
//! `FUN_10002b70` (case-fold descent; the node's `0x80` flag = child list,
//! count `&0x7f`) → `FUN_10002620` / `FUN_10001920` (resolve a child id to a
//! node record via the 4-byte **node pointer pool**, record + 2 = the key)
//! → `FUN_100018b0` / `FUN_100019f0` (walk the key payload: stop at NUL /
//! `0xFF` / `0x42`, `0x49` = skip a marker byte). The exact leaf → `.vt`
//! posting offsets remain the next RE step (the B-tree leaves carry the
//! postings references, not yet joined to the `.vt` stream).
//!
//! `vm_program_byte_len` is `FUN_100044f0`: it walks the u16 program (a byte
//! pointer advanced by 2 per instruction), tracking a `0x1001`/`0x1002`
//! depth counter, and returns the **byte** offset of the first instruction
//! after the top-level `0x1002`. `vm_record_len` is `FUN_100044a0` =
//! `L + 3 * *(byte at L) + 1`, where `L = vm_record_scan_len` (the program length
//! plus the trailing byte scan up to a `0xFF` marker, per `FUN_100044c0`).
//!
//! ## The record walker
//!
//! `FUN_10004230(handle, section_off, mode)` walks a section's records and
//! returns a compact NUL-terminated array of record **file offsets** (the
//! pointer array the engine later consumes). Each section starts with a u16
//! record count; per record the DLL computes a mode-specific byte length,
//! then advances:
//!
//! - if the byte right after the record is NUL (the record's double-NUL
//!   terminator), the next record starts at `start + len + 1`;
//! - otherwise the walker **resynchronizes to the next block boundary**:
//!   `next = (start / block + 1) * block` (`block` = `FUN_10021940()` = the
//!   file-I/O layer's buffer size, `DAT_1002ad98`, **confirmed 4096** — the
//!   `.data` image initial value; the packed B-tree regions resynchronize
//!   onto 4096-aligned slots).
//!
//! The block jumps are the packed B-tree resync: the listA/listB node
//! regions are densely packed, and a node that does not end in a double
//! NUL means the next node starts on the next block boundary.
//!
//! Modes (the `param_3` byte of `FUN_10004230`), with their record-length
//! functions:
//!
//! - `'n'` (`0x6e`, `record_len_n`) — a NUL-terminated byte run (LmCode
//!   words). The DLL's scanner advances a `short*` **one byte at a time**
//!   (the `(short *)((int)p + 1)` idiom) and stops at the first `0x00`;
//!   length = index + 1.
//! - `'f'` (`0x66`, `vm_record_len` / `FUN_100044a0`) — the opcode-VM
//!   program + `0xFF` marker + `3*count` byte trailer.
//! - `0xff` (`record_len_ffff` / `FUN_10004210`) — a u16 stream terminated
//!   by `0xFFFF`; the length includes the terminator.
//! - `'e'` (`0x65`, `record_len_e` / `FUN_100041d0`) — a byte VM starting at
//!   record offset 2: `J` takes 1 extra operand byte, `F`/`G`/`C`/`E` take
//!   2, other bytes none; the walk stops at `0xFF` (included in the length).
//!
//! Section → mode map (from the `FUN_100032d0` loader's calls): `+0x48` →
//! `'f'`, `+0x58` → `0xff`, `+0x4C`/`+0x50`/`+0x54`/`+0x60`/`+0x68`/`+0x6C`/
//! `+0x70` (and empirically `+0x64`, read by a separate consumer) → `'n'`,
//! `+0x5C` → a raw seek (no record walk), `+0x3C`/`+0x40`/`+0x44` → the
//! B-tree index consumed by the search code (outside the loader's walks).
//!
//! Verified against the real `lmu.voc` (block = 512): `f` @ `0x75253`
//! (142 records) lands exactly on `0x764c5`; `0xff` @ `0x764c5` (12) on
//! `0x765b1`; the affixes @ `0x765b2` (7) on `0x765e1`; the 33-word table
//! @ `0x765e1` on `0x7667a`; the suffixes @ `0x76714` (722) on `0x77b78` —
//! every one of them the *next* section's table offset, exact.

use crate::trident::{lm_byte_to_char, LM_CHAR_TABLE};

/// One decoded affix / morphological record: a class byte + an LmCode word.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MorphWord {
    /// the leading class / flag byte (0–7; 0 = unclassified)
    pub class: u8,
    /// the LmCode word bytes (after the class byte)
    pub word: Vec<u8>,
}

impl MorphWord {
    /// Decode the LmCode word to a display string (via [`lm_word_display`]).
    pub fn text(&self) -> String {
        lm_word_display(&self.word)
    }
}

/// Decode LmCode bytes to a human-readable word for display: letter slots
/// through [`LM_CHAR_TABLE`], printable ASCII `0x20..=0x7e` as-is (which
/// renders the hyphens of compound endings like «ам-» and «і-», and the
/// `!` class-marker byte), and every other byte as a `·xx·` marker.
pub fn lm_word_display(bytes: &[u8]) -> String {
    let mut s = String::new();
    for &b in bytes {
        if b >= 0x80 {
            match lm_byte_to_char(b) {
                Some(ch) => s.push(ch),
                None => s.push_str(&format!("·{b:02x}·")),
            }
        } else if (0x20..=0x7e).contains(&b) {
            s.push(b as char);
        } else {
            s.push_str(&format!("·{b:02x}·"));
        }
    }
    s
}

/// The 17-entry `.voc` section table (absolute file offsets; `0` = absent).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VocSectionTable {
    /// +0x3C — root u16 index table
    pub root: u32,
    /// +0x40 — listA (packed per-word records)
    pub list_a: u32,
    /// +0x44 — listB (packed per-word records)
    pub list_b: u32,
    /// +0x48 — per-word `'f'` records (opcode-VM programs)
    pub per_word_f: u32,
    /// +0x4C — morphological suffix / endings table (LmCode words)
    pub suffixes: u32,
    /// +0x50
    pub sec_50: u32,
    /// +0x54
    pub sec_54: u32,
    /// +0x58
    pub sec_58: u32,
    /// +0x5C
    pub sec_5c: u32,
    /// +0x60 — affix table (7 class-tagged LmCode words)
    pub affixes: u32,
    /// +0x64
    pub sec_64: u32,
    /// +0x68
    pub sec_68: u32,
    /// +0x6C
    pub sec_6c: u32,
    /// +0x70
    pub sec_70: u32,
    /// +0x74 — dual (count, section); low u16 = listA record count
    pub dual_74: u32,
    /// +0x78 — dual (count, section)
    pub dual_78: u32,
    /// +0x7C — dual (count, section); low u16 = digraph count
    pub dual_7c: u32,
}

impl VocSectionTable {
    /// Parse the 17-entry section table from a `.voc` file (offset `0x3C`).
    /// Returns `None` when the buffer is too short.
    pub fn parse(voc: &[u8]) -> Option<Self> {
        if voc.len() < 0x80 {
            return None;
        }
        let r = |off: usize| u32::from_le_bytes(voc[off..off + 4].try_into().unwrap());
        Some(VocSectionTable {
            root: r(0x3c),
            list_a: r(0x40),
            list_b: r(0x44),
            per_word_f: r(0x48),
            suffixes: r(0x4c),
            sec_50: r(0x50),
            sec_54: r(0x54),
            sec_58: r(0x58),
            sec_5c: r(0x5c),
            affixes: r(0x60),
            sec_64: r(0x64),
            sec_68: r(0x68),
            sec_6c: r(0x6c),
            sec_70: r(0x70),
            dual_74: r(0x74),
            dual_78: r(0x78),
            dual_7c: r(0x7c),
        })
    }

    /// The low u16 of a dual-encoded field (the record count).
    pub fn dual_count(&self, field: u32) -> u16 {
        u16::try_from(field & 0xFFFF).unwrap_or(0)
    }

    /// The high u16 of a dual-encoded field (the section selector).
    pub fn dual_selector(&self, field: u32) -> u16 {
        u16::try_from((field >> 16) & 0xFFFF).unwrap_or(0)
    }

    /// Read a table slot by its header offset (e.g. `0x48` → the per-word
    /// `'f'` section) as a file offset (`0` = the section is absent).
    pub fn slot_offset(&self, slot: u32) -> u32 {
        match slot {
            0x3c => self.root,
            0x40 => self.list_a,
            0x44 => self.list_b,
            0x48 => self.per_word_f,
            0x4c => self.suffixes,
            0x50 => self.sec_50,
            0x54 => self.sec_54,
            0x58 => self.sec_58,
            0x5c => self.sec_5c,
            0x60 => self.affixes,
            0x64 => self.sec_64,
            0x68 => self.sec_68,
            0x6c => self.sec_6c,
            0x70 => self.sec_70,
            0x74 => self.dual_74,
            0x78 => self.dual_78,
            0x7c => self.dual_7c,
            _ => 0,
        }
    }
}

// ---------------------------------------------------------------------------
// The opcode VM
// ---------------------------------------------------------------------------

/// `FUN_100044f0`: the **byte** length of a top-level opcode-VM program.
///
/// The VM walks a u16 instruction stream (byte pointer, +2 per
/// instruction). Depth counters `0x1001`/`0x1002` nest sub-blocks; the
/// function returns the byte offset of the first instruction after the
/// top-level `0x1002`. When no top-level terminator exists, the whole
/// (even-truncated) input length is returned — the observed 3-byte
/// overflow in record lengths is exactly that truncation.
pub fn vm_program_byte_len(prog: &[u8]) -> usize {
    let mut depth: i32 = 0;
    let mut i = 0usize;
    while i + 2 <= prog.len() {
        let op = u16::from_le_bytes([prog[i], prog[i + 1]]);
        match op {
            0x1002 => {
                if depth == 0 {
                    return i + 2;
                }
                depth -= 1;
            }
            0x1001 => depth += 1,
            _ => {}
        }
        i += 2;
    }
    // no top-level terminator found: the whole (even-truncated) input
    prog.len() - prog.len() % 2
}

/// `FUN_100044c0`: the program byte length plus the trailing byte scan up to
/// (and including) the first `0xFF` marker byte after the program.
fn vm_record_scan_len(rec: &[u8]) -> usize {
    let l0 = vm_program_byte_len(rec);
    if l0 >= rec.len() {
        return l0;
    }
    // scan from byte `l0`; the first `0xFF` byte terminates the scan
    let mut i = l0;
    while i < rec.len() {
        if rec[i] == 0xFF {
            return i + 1;
        }
        i += 1;
    }
    // no 0xFF marker within the record: the scan runs to the end
    rec.len()
}

/// `FUN_100044a0`: the total **byte** length of a `'f'`-flag record:
/// `L + 3 * count + 1`, where `L = vm_record_scan_len(rec)` and `count` is
/// the byte at offset `L` (the trailing count block). Returns the record
/// length in bytes, or `None` when the record is truncated.
pub fn vm_record_len(rec: &[u8]) -> Option<usize> {
    let l = vm_record_scan_len(rec);
    if l >= rec.len() {
        return Some(rec.len());
    }
    let count = rec[l] as usize;
    let total = l + 1 + 3 * count;
    if total > rec.len() {
        None
    } else {
        Some(total)
    }
}

// ---------------------------------------------------------------------------
// The record walker (`FUN_10004230`)
// ---------------------------------------------------------------------------

/// `'n'` walk mode (the `0x6e` byte of `FUN_10004230`).
pub const REC_MODE_N: u8 = b'n';
/// `'e'` walk mode (the `0x65` byte of `FUN_10004230`).
pub const REC_MODE_E: u8 = b'e';
/// `'f'` walk mode (the `0x66` byte of `FUN_10004230`).
pub const REC_MODE_F: u8 = b'f';
/// `0xff` walk mode (the u16 stream terminated by `0xFFFF`).
pub const REC_MODE_FFFF: u8 = 0xFF;

/// The block-resync size for [`walk_section_records`]: the DLL takes it
/// from `FUN_10021940()` (the file-I/O layer's buffer size,
/// `DAT_1002ad98`). The value is **confirmed 4096**: `DAT_1002ad98` has
/// no write in the DLL's `.text` (byte-scanned `C7 05` / `89 0D` / `A3`
/// stores, none), so it carries its `.data` image initial value
/// `0x00001000` (4096) — the I/O layer's page-sized read buffer. The
/// packed B-tree regions resynchronize exactly onto 4096-aligned slots
/// (the listA walk's jump landings are `0x69000, 0x6A000, … 0x77000`,
/// step 0x1000). The verified clean-section landings are identical for
/// every block size in 128..=4096, so the walk is robust to the choice
/// either way.
pub const WALK_BLOCK: u32 = 4096;

/// `FUN_10004230` `'n'` mode: a NUL-terminated byte run (LmCode words). The
/// DLL's scanner advances one byte at a time and stops at the first `0x00`;
/// the length is that index + 1 (the NUL included).
pub fn record_len_n(rec: &[u8]) -> usize {
    match rec.iter().position(|&b| b == 0x00) {
        Some(i) => i + 1,
        None => rec.len(),
    }
}

/// `FUN_10004210` (`0xff` mode): a u16 stream terminated by `0xFFFF`; the
/// length includes the terminator u16.
pub fn record_len_ffff(rec: &[u8]) -> usize {
    let mut i = 0usize;
    while i + 2 <= rec.len() {
        if u16::from_le_bytes([rec[i], rec[i + 1]]) == 0xFFFF {
            return i + 2;
        }
        i += 2;
    }
    rec.len()
}

/// `FUN_100041d0` (`'e'` mode): a byte VM over the record starting at offset
/// 2 (the first 2 bytes are the record's header). `J` takes 1 extra operand
/// byte; `F`/`G`/`C`/`E` take 2; any other byte takes none. The walk stops
/// at `0xFF`, which is included in the returned length.
pub fn record_len_e(rec: &[u8]) -> usize {
    let mut i = 2usize;
    while i < rec.len() && rec[i] != 0xFF {
        let extra = match rec[i] {
            b'J' => 1,
            b'F' | b'G' | b'C' | b'E' => 2,
            _ => 0,
        };
        i += 1 + extra;
    }
    if i < rec.len() {
        i + 1
    } else {
        rec.len()
    }
}

/// The record length for a walk mode (the `FUN_10004230` match arms). An
/// unknown mode returns the whole record (the walk then resynchronizes via
/// the block boundary).
pub fn record_len(rec: &[u8], mode: u8) -> usize {
    match mode {
        REC_MODE_N => record_len_n(rec),
        REC_MODE_E => record_len_e(rec),
        REC_MODE_F => vm_record_len(rec).unwrap_or(rec.len()),
        REC_MODE_FFFF => record_len_ffff(rec),
        _ => rec.len(),
    }
}

/// The result of walking one section's records: the record start offsets
/// (the DLL walker's pointer array) plus the landing offset after the last
/// record.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SectionWalk {
    /// the u16 record count read from the section header
    pub count: u16,
    /// each record's start file offset (what `FUN_10004230` returns)
    pub starts: Vec<u32>,
    /// the block-resync jumps taken (records that did not end in a double
    /// NUL — the packed B-tree regions)
    pub block_jumps: u32,
    /// the offset after the last record; may lie past the file end when the
    /// final record resynchronizes off the end
    pub landed: u32,
}

/// `FUN_10004230` — walk the `count` records of a section and return their
/// start offsets + the landing offset.
///
/// Advance rule (mirroring the decompiled loader): `len` is the mode's
/// record length (bounded by the file end); when the byte at `start + len`
/// is NUL the next record starts at `start + len + 1` (the double-NUL
/// terminator), otherwise the walker jumps to the next `block` boundary:
/// `(start / block + 1) * block`.
pub fn walk_section_records(voc: &[u8], off: u32, mode: u8, block: u32) -> SectionWalk {
    let o = off as usize;
    if o + 2 > voc.len() || block == 0 {
        return SectionWalk::default();
    }
    let count = u16_from(voc, off);
    let block = block as usize;
    let mut p = o + 2;
    let mut starts = Vec::with_capacity(count as usize);
    let mut jumps = 0u32;
    for _ in 0..count as u32 {
        if p >= voc.len() {
            break;
        }
        starts.push(p as u32);
        let rec = &voc[p..];
        let len = record_len(rec, mode).min(rec.len());
        if p + len < voc.len() && voc[p + len] == 0x00 {
            p += len + 1;
        } else {
            jumps += 1;
            p = ((p / block) + 1) * block;
        }
    }
    SectionWalk {
        count,
        starts,
        block_jumps: jumps,
        landed: p as u32,
    }
}

/// Section slot (header offset) → record-walk mode, mirroring the
/// `FUN_100032d0` loader: `+0x48` → `'f'`, `+0x58` → `0xff`,
/// `+0x4C`/`+0x50`/`+0x54`/`+0x60`/`+0x68`/`+0x6C`/`+0x70` → `'n'`. The
/// `+0x64` 33-word table is read by a separate consumer, but a `'n'` walk
/// of it lands exactly on the next section (`0x7667a`) — included. `+0x5C`
/// is a raw seek, `+0x3C`/`+0x40`/`+0x44` are the B-tree index.
pub fn section_mode(slot: u32) -> Option<u8> {
    match slot {
        0x48 => Some(REC_MODE_F),
        0x58 => Some(REC_MODE_FFFF),
        0x4c | 0x50 | 0x54 | 0x60 | 0x64 | 0x68 | 0x6c | 0x70 => Some(REC_MODE_N),
        _ => None,
    }
}

/// Walk every walkable section of `table` (see [`section_mode`]) with the
/// DLL's advance rule. Returns `(slot, walk)` pairs in slot order.
pub fn walk_sections(voc: &[u8], table: &VocSectionTable, block: u32) -> Vec<(u32, SectionWalk)> {
    const SLOTS: [u32; 10] = [
        0x48, 0x4c, 0x50, 0x54, 0x58, 0x60, 0x64, 0x68, 0x6c, 0x70,
    ];
    let mut out = Vec::new();
    for &slot in &SLOTS {
        let Some(mode) = section_mode(slot) else {
            continue;
        };
        let off = table.slot_offset(slot);
        if off == 0 {
            continue;
        }
        out.push((slot, walk_section_records(voc, off, mode, block)));
    }
    out
}

// ---------------------------------------------------------------------------
// Section readers
// ---------------------------------------------------------------------------

/// Skip the NUL terminator(s) of a record whose payload ended at `p`: the
/// record's word bytes are NUL-terminated, and the observed layout has a
/// **second** NUL byte immediately after (the `…00 00` double terminator).
/// Advances `p` past the first NUL and, when present, the second.
fn skip_record_terminator(voc: &[u8], p: &mut usize) {
    if *p < voc.len() {
        *p += 1; // first NUL
    }
    if *p < voc.len() && voc[*p] == 0x00 {
        *p += 1; // second NUL (double terminator)
    }
}

/// Read a count-driven word section: `[u16 count][word][00 00] × count` where
/// each `word` is a NUL-terminated LmCode byte run (the suffix/endings table
/// at `+0x4C`, the 33-word table at `+0x64`, and the digraph table at
/// `+0x70`). Returns `(count, words)` — each word without its terminators.
pub fn read_word_section(voc: &[u8], off: u32) -> (u16, Vec<Vec<u8>>) {
    let o = off as usize;
    if o + 2 > voc.len() {
        return (0, Vec::new());
    }
    let count = u16::from_le_bytes([voc[o], voc[o + 1]]);
    let mut p = o + 2;
    let mut out: Vec<Vec<u8>> = Vec::with_capacity(count as usize);
    for _ in 0..count as usize {
        if p >= voc.len() {
            break;
        }
        let start = p;
        while p < voc.len() && voc[p] != 0x00 {
            p += 1;
        }
        out.push(voc[start..p].to_vec());
        skip_record_terminator(voc, &mut p);
    }
    (count, out)
}

/// Read a count-driven **class-tagged** morphological record section (the
/// affix table at `+0x60`): each record is
/// `[u8 class][LmCode word][00 00]`. Returns `(count, MorphWord[])`.
pub fn read_morph_records(voc: &[u8], off: u32) -> (u16, Vec<MorphWord>) {
    let o = off as usize;
    if o + 2 > voc.len() {
        return (0, Vec::new());
    }
    let count = u16::from_le_bytes([voc[o], voc[o + 1]]);
    let mut p = o + 2;
    let mut out: Vec<MorphWord> = Vec::with_capacity(count as usize);
    for _ in 0..count as usize {
        if p >= voc.len() {
            break;
        }
        let class = voc[p];
        p += 1;
        let start = p;
        while p < voc.len() && voc[p] != 0x00 {
            p += 1;
        }
        let word = voc[start..p].to_vec();
        skip_record_terminator(voc, &mut p);
        out.push(MorphWord { class, word });
    }
    (count, out)
}

// re-export for callers that want the decode helper alongside the tables
#[allow(dead_code)]
fn _lm_table() -> &'static [(u8, char)] {
    LM_CHAR_TABLE
}

/// The decoded morphology tables of one `.voc` file (the decodable subset).
#[derive(Debug, Clone, Default)]
pub struct Morphology {
    /// the 17-entry section table
    pub table: VocSectionTable,
    /// the class-tagged affix words (section `+0x60`, 7 in `lmu.voc`)
    pub affixes: Vec<MorphWord>,
    /// the morphological suffix / endings (section `+0x4C`)
    pub suffixes: Vec<Vec<u8>>,
    /// the 33-word table (section `+0x64`)
    pub sec_64_words: Vec<Vec<u8>>,
    /// the digitraph / two-letter table (section `+0x70`)
    pub digraphs: Vec<String>,
    /// the number of packed records in listA / listB (their u16 headers)
    pub list_a_count: u16,
    pub list_b_count: u16,
    /// the per-word `'f'` record count (section `+0x48`)
    pub per_word_f_count: u16,
    /// the `FUN_10004230` record walks of every walkable section
    /// (`(slot, walk)`, block = [`WALK_BLOCK`])
    pub walks: Vec<(u32, SectionWalk)>,
}

impl Morphology {
    /// Parse the decodable morphology tables from a `.voc` file.
    pub fn parse(voc: &[u8]) -> Option<Self> {
        let table = VocSectionTable::parse(voc)?;
        let mut affixes = Vec::new();
        if table.affixes != 0 {
            let (_c, w) = read_morph_records(voc, table.affixes);
            affixes = w;
        }
        let mut suffixes = Vec::new();
        if table.suffixes != 0 {
            let (_c, w) = read_word_section(voc, table.suffixes);
            suffixes = w;
        }
        let mut sec_64_words = Vec::new();
        if table.sec_64 != 0 {
            let (_c, w) = read_word_section(voc, table.sec_64);
            sec_64_words = w;
        }
        // digraphs: a section of NUL-terminated ASCII two-letter strings
        let mut digraphs = Vec::new();
        if table.sec_70 != 0 {
            let (_c, recs) = read_word_section(voc, table.sec_70);
            digraphs = recs
                .iter()
                .map(|r| {
                    String::from_utf8_lossy(r)
                        .into_owned()
                        .chars()
                        .filter(|c| c.is_ascii_alphanumeric())
                        .collect()
                })
                .filter(|s: &String| !s.is_empty())
                .collect();
        }
        // listA / listB record counts (their u16 headers)
        let list_a_count = if table.list_a != 0 {
            u16_from(voc, table.list_a)
        } else {
            0
        };
        let list_b_count = if table.list_b != 0 {
            u16_from(voc, table.list_b)
        } else {
            0
        };
        let per_word_f_count = if table.per_word_f != 0 {
            u16_from(voc, table.per_word_f)
        } else {
            0
        };
        let walks = walk_sections(voc, &table, WALK_BLOCK);
        Some(Morphology {
            table,
            affixes,
            suffixes,
            sec_64_words,
            digraphs,
            list_a_count,
            list_b_count,
            per_word_f_count,
            walks,
        })
    }

    /// The number of distinct LmCode characters the tables use (a sanity
    /// signal that the decode is over the real letter slots).
    pub fn lm_codepoint_count(&self) -> usize {
        let mut seen = std::collections::BTreeSet::new();
        for w in &self.affixes {
            for b in &w.word {
                seen.insert(*b);
            }
        }
        for w in &self.suffixes {
            for b in w {
                seen.insert(*b);
            }
        }
        let mut letters = 0usize;
        for &b in &seen {
            if LM_CHAR_TABLE.iter().any(|(code, _)| *code == b) {
                letters += 1;
            }
        }
        letters
    }
}

fn u16_from(voc: &[u8], off: u32) -> u16 {
    let o = off as usize;
    if o + 2 > voc.len() {
        return 0;
    }
    u16::from_le_bytes([voc[o], voc[o + 1]])
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vm_program_byte_len_top_level() {
        // 0x1003 (leaf) then 0x1002 (terminator): 4 bytes
        let prog: [u8; 4] = [0x03, 0x10, 0x02, 0x10];
        assert_eq!(vm_program_byte_len(&prog), 4);
        // nested: 0x1001 (enter), 0x1003, 0x1002 (close), 0x1002 (top end)
        let prog: [u8; 8] = [
            0x01, 0x10, 0x03, 0x10, 0x02, 0x10, 0x02, 0x10,
        ];
        assert_eq!(vm_program_byte_len(&prog), 8);
        // data u16s are skipped: 0x0024 then 0x1002
        let prog: [u8; 4] = [0x24, 0x00, 0x02, 0x10];
        assert_eq!(vm_program_byte_len(&prog), 4);
    }

    #[test]
    fn vm_record_len_adds_the_3c_tail() {
        // program (0x1002, 2 bytes) + 0xFF marker (1 byte) + count=2 →
        // L=3 (0xFF at index 2, +1), total = 3 + 1 + 3*2 = 8
        let rec: [u8; 8] = [0x02, 0x10, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00];
        // count byte is at index L=3 → rec[3]=0 → total = 3 + 1 + 0 = 4
        assert_eq!(vm_record_len(&rec), Some(4));
        // a record whose scan finds no 0xFF: whole tail is the record
        let rec2: [u8; 4] = [0x02, 0x10, 0x01, 0x02];
        assert_eq!(vm_record_len(&rec2), Some(4));
    }

    #[test]
    fn section_table_parses_real_lmu_voc() {
        let Some(voc) = std::fs::read(r"C:\Zakon.Local\Baza\SOFT\lmu.voc").ok() else {
            eprintln!("skipped: no base");
            return;
        };
        let t = VocSectionTable::parse(&voc).expect("table");
        // the confirmed offsets (see the byte dumps)
        assert_eq!(t.list_a, 0x00068368);
        assert_eq!(t.list_b, 0x0006ca96);
        assert_eq!(t.per_word_f, 0x00075253);
        assert_eq!(t.suffixes, 0x00076714);
        assert_eq!(t.affixes, 0x000765b2);
        // dual fields: low u16 counts (0x02D2 = 722, 0x0020 = 32)
        assert_eq!(t.dual_count(t.dual_74), 722, "listA count");
        assert_eq!(t.dual_count(t.dual_78), 516, "dual_78 count");
        assert_eq!(t.dual_count(t.dual_7c), 32, "digraph count");
    }

    #[test]
    fn affix_table_decodes_the_seven_known_words() {
        let Some(voc) = std::fs::read(r"C:\Zakon.Local\Baza\SOFT\lmu.voc").ok() else {
            eprintln!("skipped: no base");
            return;
        };
        let m = Morphology::parse(&voc).expect("morph");
        assert_eq!(m.affixes.len(), 7);
        // the known affix words (class, text)
        let texts: Vec<String> = m.affixes.iter().map(|w| w.text()).collect();
        // най, по-, по, не, спів, анти, псевдо
        assert!(texts.iter().any(|t| t == "най"), "най missing: {texts:?}");
        assert!(
            texts.iter().any(|t| t == "не"),
            "не missing: {texts:?}"
        );
        assert!(
            texts.iter().any(|t| t == "псевдо"),
            "псевдо missing: {texts:?}"
        );
        // the classes observed in the file
        let classes: Vec<u8> = m.affixes.iter().map(|w| w.class).collect();
        assert_eq!(classes, vec![1, 3, 4, 2, 6, 5, 5]);
    }

    #[test]
    fn suffix_table_is_non_empty_lmcode() {
        let Some(voc) = std::fs::read(r"C:\Zakon.Local\Baza\SOFT\lmu.voc").ok() else {
            eprintln!("skipped: no base");
            return;
        };
        let m = Morphology::parse(&voc).expect("morph");
        // the suffix/affix table has 722 records (its u16 header)
        assert_eq!(m.suffixes.len(), 722, "suffixes len {}", m.suffixes.len());
        // the records are LmCode letter runs mixed with ASCII / control
        // marker bytes (affixes like «ам-», «і-», plus class codes); the
        // non-empty majority must be LmCode letters
        let nonempty = m.suffixes.iter().filter(|w| !w.is_empty()).count();
        assert!(nonempty > 500, "non-empty suffixes {}", nonempty);
        // spot-check: the table holds hyphenated affix endings («ам-», «і-»)
        assert!(
            m.suffixes.iter().any(|w| w.iter().any(|&b| b == 0x2d)),
            "a hyphenated affix ending (0x2d) must be present"
        );
    }

    #[test]
    fn list_counts_match_the_dual_fields() {
        let Some(voc) = std::fs::read(r"C:\Zakon.Local\Baza\SOFT\lmu.voc").ok() else {
            eprintln!("skipped: no base");
            return;
        };
        let m = Morphology::parse(&voc).expect("morph");
        assert_eq!(m.list_a_count, 722, "listA records");
        assert_eq!(m.list_b_count, 2157, "listB records");
        assert_eq!(m.per_word_f_count, 142, "per-word 'f' records");
    }

    #[test]
    fn record_len_n_stops_at_the_first_nul() {
        // LmCode «ам-» + NUL: the first NUL is at index 3 → length 4
        assert_eq!(record_len_n(&[0xA0, 0xAC, 0x2d, 0x00, 0x00]), 4);
        // no NUL in sight: the whole input
        assert_eq!(record_len_n(&[0xA0, 0xAC, 0x2d]), 3);
        // an empty input stays empty
        assert_eq!(record_len_n(&[]), 0);
    }

    #[test]
    fn record_len_ffff_includes_the_terminator_u16() {
        // two data u16s then the 0xFFFF terminator: length 6
        let rec = [0x11, 0x00, 0x22, 0x00, 0xFF, 0xFF];
        assert_eq!(record_len_ffff(&rec), 6);
        // an odd tail without the terminator: the whole input
        assert_eq!(record_len_ffff(&[0x11, 0x00, 0x22]), 3);
    }

    #[test]
    fn record_len_e_walks_the_byte_vm() {
        // record: 2-byte header + J (1 operand) + data + 0xFF at index 5
        let rec: [u8; 6] = [0x00, 0x00, b'J', 0x07, 0x00, 0xFF];
        // i=2: J → i += 2 → 4; rec[4] != 0xFF → i += 1 → 5; rec[5] == 0xFF → 5 + 1
        assert_eq!(record_len_e(&rec), 6);
        // F takes 2 operand bytes: i=2 → F → i=5; data; 0xFF at 6 → 7
        let rec2: [u8; 7] = [0x00, 0x00, b'F', 0x01, 0x02, 0x00, 0xFF];
        assert_eq!(record_len_e(&rec2), 7);
        // an under-2-byte header falls back to the whole input
        assert_eq!(record_len_e(&[0x00]), 1);
    }

    #[test]
    fn record_len_dispatches_by_mode() {
        let rec: [u8; 6] = [0x11, 0x00, 0x22, 0x00, 0xFF, 0xFF];
        assert_eq!(record_len(&rec, REC_MODE_FFFF), 6);
        assert_eq!(record_len(&[0xA0, 0xAC, 0x00], REC_MODE_N), 3);
        assert_eq!(record_len(&[0x02, 0x10, 0x00, 0x00], REC_MODE_F), 4);
        // an unknown mode: the whole record (resync via the block boundary)
        assert_eq!(record_len(&[0x00, 0x01, 0x02], 0x42), 3);
    }

    #[test]
    fn walk_records_land_on_the_verified_section_boundaries() {
        let Some(voc) = std::fs::read(r"C:\Zakon.Local\Baza\SOFT\lmu.voc").ok() else {
            eprintln!("skipped: no base");
            return;
        };
        // the verified exact landings (the next section's table offset)
        let cases: [(u32, u8, u16, u32); 6] = [
            (0x75253, REC_MODE_F, 142, 0x764c5),
            (0x764c5, REC_MODE_FFFF, 12, 0x765b1),
            (0x765b2, REC_MODE_N, 7, 0x765e1),
            (0x765e1, REC_MODE_N, 33, 0x7667a),
            (0x76691, REC_MODE_N, 32, 0x76714),
            (0x76714, REC_MODE_N, 722, 0x77b78),
        ];
        for (off, mode, count, landed) in cases {
            let w = walk_section_records(&voc, off, mode, WALK_BLOCK);
            assert_eq!(w.count, count, "count @ {off:#x}");
            assert_eq!(w.landed, landed, "landing @ {off:#x} mode {mode}");
            assert_eq!(w.starts.len(), count as usize, "starts @ {off:#x}");
            assert_eq!(w.starts[0], off + 2, "first start @ {off:#x}");
        }
        // the packed B-tree lists at the confirmed block size (4096): the
        // walk resynchronizes onto 4096-aligned node slots (0x69000,
        // 0x6A000, … 0x77000) and runs just past the file end. The
        // header's record count (722 / 2157) is larger than the walked
        // starts — the dense nodes inside a block are consumed directly
        // from the block data, not via the pointer array (the payload
        // semantics remain the next RE step).
        let la = walk_section_records(&voc, 0x068368, REC_MODE_N, WALK_BLOCK);
        assert_eq!(la.count, 722);
        assert_eq!(la.starts.len(), 388, "listA resync walk at block 4096");
        assert_eq!(la.landed, 0x78000, "listA resynchronizes off the end");
        // the resync landings sit exactly on the 4096-aligned node slots
        // (0x69000, 0x6A000, … 0x77000 — 15 slots, step 0x1000)
        let aligned = la
            .starts
            .iter()
            .filter(|s| **s >= 0x69000 && **s % 0x1000 == 0)
            .count();
        assert_eq!(aligned, 15, "the 4096-aligned node slots");
        let lb = walk_section_records(&voc, 0x06ca96, REC_MODE_N, WALK_BLOCK);
        assert_eq!(lb.count, 2157);
        assert_eq!(lb.starts.len(), 382, "listB resync walk at block 4096");
        assert_eq!(lb.landed, 0x78000, "listB resynchronizes off the end");
    }

    #[test]
    fn walk_landings_are_block_size_independent() {
        let Some(voc) = std::fs::read(r"C:\Zakon.Local\Baza\SOFT\lmu.voc").ok() else {
            eprintln!("skipped: no base");
            return;
        };
        // the verified landings are identical for every block size in
        // 128..=4096 (the single resync jump resynchronizes onto the next
        // section's start regardless of the boundary size)
        for (off, mode, landed) in [
            (0x75253u32, REC_MODE_F, 0x764c5u32),
            (0x76714u32, REC_MODE_N, 0x77b78u32),
        ] {
            for block in [128u32, 256, 512, 1024, 2048, 4096] {
                let w = walk_section_records(&voc, off, mode, block);
                assert_eq!(
                    w.landed,
                    landed,
                    "landing @ {off:#x} mode {mode} block {block}"
                );
            }
        }
    }

    #[test]
    fn morphology_walks_cover_the_walkable_slots() {
        let Some(voc) = std::fs::read(r"C:\Zakon.Local\Baza\SOFT\lmu.voc").ok() else {
            eprintln!("skipped: no base");
            return;
        };
        let m = Morphology::parse(&voc).expect("morph");
        // every non-absent walkable slot is walked (10 slots in lmu.voc)
        assert_eq!(m.walks.len(), 10, "walks: {:?}", m.walks.len());
        let slots: Vec<u32> = m.walks.iter().map(|(s, _)| *s).collect();
        assert_eq!(
            slots,
            vec![0x48, 0x4c, 0x50, 0x54, 0x58, 0x60, 0x64, 0x68, 0x6c, 0x70]
        );
        // the digraph section is empty (count 0): zero records, no jumps
        let digraph = m
            .walks
            .iter()
            .find(|(s, _)| *s == 0x70)
            .expect("the digraph walk");
        assert_eq!(digraph.1.count, 0);
        assert!(digraph.1.starts.is_empty());
    }

    #[test]
    fn section_mode_maps_the_loader_slots() {
        assert_eq!(section_mode(0x48), Some(REC_MODE_F));
        assert_eq!(section_mode(0x58), Some(REC_MODE_FFFF));
        assert_eq!(section_mode(0x4c), Some(REC_MODE_N));
        assert_eq!(section_mode(0x70), Some(REC_MODE_N));
        // the raw-seek and B-tree slots are not walked by the loader
        assert_eq!(section_mode(0x5c), None);
        assert_eq!(section_mode(0x40), None);
    }

    #[test]
    fn lm_word_display_renders_hyphens_and_letters() {
        // LmCode «ам-» (м=0xAC, а=0xA0, hyphen=0x2D) renders with a hyphen
        assert_eq!(lm_word_display(&[0xA0, 0xAC, 0x2d]), "ам-");
        // a pure-letter word is untouched by the display layer
        assert_eq!(lm_word_display(&[0x8F, 0xAF]), "Пп");
        // printable ASCII marker bytes (0x21 = '!') render as-is
        assert_eq!(lm_word_display(&[0xA8, 0x21]), "и!");
        // non-letter, non-printable bytes keep the `·xx·` marker
        assert_eq!(lm_word_display(&[0x01, 0xAC]), "·01·м");
    }
}
