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
//! `+0x70` → `'n'`.
//!
//! **The two loader-consumer gaps (roadmap v0.33.0), triaged from
//! `FUN_100032d0` (clean-room, file-referenced):**
//! - `+0x5C` — a **raw seek** (the loader's `FUN_10021800(handle, +0x5C)`,
//!   the file-I/O layer's seek), stored to `DAT_1002db76` (`+0x5C`'s
//!   struct field). No `FUN_10004230` record walk — it is a direct
//!   file-position / count read, not a packed record section.
//! - `+0x64` — the **33-word table**, walked by the *same* loader
//!   (`FUN_10004230(handle, +0x64, 'n')`, the `FUN_100032d0` line with the
//!   `+ 100` decimal offset, under the `param_1 & 8` affix/word load flag),
//!   stored to `DAT_1002db7e`. Not a separate consumer — the earlier
//!   "separate consumer" note is the loader's `& 8` mode branch.
//! `+0x3C`/`+0x40`/`+0x44` → the B-tree index consumed by the search code
//! (outside the loader's walks).
//!
//! **The per-word `'f'` program count invariant (roadmap v0.33.0 DoD, UA+EN):**
//! in `lmu.voc`, the `+0x48` section declares 142 records and the
//! `block = 4096` walk resynchronizes onto all 142 (each ends in a
//! double-NUL in-file, unlike the dense listA/listB). Running all 142
//! through [`run_f_program`]: **every one terminates at the top-level
//! `0x1002`** (142/142), with **176 `0x1003` word-leaf hits** in total.
//! EN: of the 142 declared per-word `'f'` programs, all 142 are walked and
//! all 142 terminate at the top-level `0x1002`; the 176 `0x1003` word-leaf
//! hits are the resolvable part — the `0x1004` child-subtree word counts
//! need the `DAT_1002ddec` node pool (a DLL-image global, deferred), so
//! [`Morphology::f_programs`] records the structural skeleton.
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

/// The u16 tuning-parameter block at header offset `0x80` of a `.voc`
/// file (immediately after the u32 section table `0x40..0x7F`).
///
/// `lmu.voc` decodes to `2,2,1,0x600,2,2,0x42,0x2000,0x20,0x108,0x104,0x10,
/// 2,4,0x80,0x400`; `lmr.voc` decodes to `2,1,0x220,0x402,0x800,0x10,8,4`
/// (the rest zero). The individual roles are only partially identified from
/// the DLL (`lmsup32.dll`) walkers — the values that line up with observed
/// constants get a named accessor, the rest stay in `raw`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VocParams {
    /// The 16 raw u16 values in file order (`0x80 + 2*i`).
    pub raw: [u16; 16],
}

impl VocParams {
    /// Parse the parameter block. Requires a file of at least `0x98` bytes.
    pub fn parse(voc: &[u8]) -> Option<Self> {
        if voc.len() < 0x98 {
            return None;
        }
        let mut raw = [0u16; 16];
        for i in 0..16 {
            raw[i] = u16::from_le_bytes([
                voc[0x80 + 2 * i],
                voc[0x80 + 2 * i + 1],
            ]);
        }
        Some(VocParams { raw })
    }

    /// `[6]` — the listA record stop-marker u16. `lmu.voc` holds `0x42`
    /// (`'B'`), which is exactly the terminator u16 the listA record
    /// walker (`FUN_10004230`, mode `n`) stops on.
    pub fn stop_marker(&self) -> u16 {
        self.raw[6]
    }

    /// `[7]` — a block/page-style constant (`lmu.voc`: `0x2000` = 8192).
    pub fn page_const(&self) -> u16 {
        self.raw[7]
    }

    /// `[8]` — `lmu.voc` holds `0x20` (32), which matches the record count
    /// of the `+0x54` section (the known `sec_54` count).
    pub fn sec_54_count(&self) -> u16 {
        self.raw[8]
    }

    /// `[9]` — `lmu.voc` holds `0x108` (264), exactly the size of
    /// `lmr.gw` (the morphology weight file).
    pub fn gw_size(&self) -> u16 {
        self.raw[9]
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
// v0.32.0 (roadmap v0.33.0): the per-word `'f'` opcode-VM execution
// ---------------------------------------------------------------------------

/// The result of executing one per-word `'f'` program (roadmap v0.33.0):
/// the program byte length, the word count, and the collected inline u16 data.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FResult {
    /// the program byte length (up to and including the top-level `0x1002`;
    /// the even-truncated length when there is no top-level terminator).
    pub byte_len: usize,
    /// the word count: the `0x1003` word-leaf hits + the `0x1004` child
    /// subtree word counts (via the resolver).
    pub word_count: u32,
    /// the inline u16 data collected (the non-`0x1001`/`0x1002`/`0x1003`
    /// ops: the `0x1004` child pointers + the raw data u16s).
    pub data: Vec<u16>,
    /// whether the scan terminated at a top-level `0x1002` (vs running to
    /// the record end / an even-truncation).
    pub terminated: bool,
}

/// `FUN_100044f0` (with the word-count out-param) — execute a per-word `'f'`
/// opcode-VM program (the `0x1001..0x1004` u16 instruction stream):
/// - `0x1001` — enter a nested subtree (depth + 1);
/// - `0x1002` — a subtree terminator (depth − 1); a top-level one (depth 0)
///   terminates the program;
/// - `0x1003` — a word leaf (`word_count += 1`);
/// - `0x1004 [u16 ptr]` — a child node: the resolver (`child_word_count`)
///   yields the child's word count (the node's first u16, via `DAT_1002ddec`);
///   a `None` (pool-not-dumped) default contributes `0`. The child pointer is
///   recorded in [`FResult::data`];
/// - any other u16 — inline data, recorded in [`FResult::data`].
///
/// `child_word_count` is the `0x1004` child resolver: for the structural
/// skeleton (the `DAT_1002ddec` pool not dumped) pass a closure that always
/// returns `None`; the `word_count` then counts only the `0x1003` leaf hits.
/// Bounds-safe; never panics.
pub fn run_f_program(rec: &[u8], child_word_count: impl Fn(u16) -> Option<u32>) -> FResult {
    let mut depth: i32 = 0;
    let mut word_count: u32 = 0;
    let mut data: Vec<u16> = Vec::new();
    let mut i = 0usize;
    let mut terminated = false;
    while i + 2 <= rec.len() {
        let op = u16::from_le_bytes([rec[i], rec[i + 1]]);
        match op {
            0x1001 => {
                depth += 1;
                i += 2;
            }
            0x1002 => {
                if depth == 0 {
                    i += 2;
                    terminated = true;
                    break;
                }
                depth -= 1;
                i += 2;
            }
            0x1003 => {
                word_count += 1;
                i += 2;
            }
            0x1004 => {
                i += 2;
                if i + 2 <= rec.len() {
                    let ptr = u16::from_le_bytes([rec[i], rec[i + 1]]);
                    data.push(ptr);
                    word_count += child_word_count(ptr).unwrap_or(0);
                    i += 2;
                }
            }
            _ => {
                data.push(op);
                i += 2;
            }
        }
    }
    FResult {
        byte_len: i,
        word_count,
        data,
        terminated,
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
    /// the executed per-word `'f'` program report (v0.32.0: [`run_f_program`]
    /// over every walked `+0x48` record; `None` when the section is absent).
    /// The structural skeleton — the `0x1004` child word counts need the
    /// `DAT_1002ddec` node pool (a DLL-image global, not dumped).
    pub f_programs: Option<Vec<FResult>>,
    /// the `FUN_10004230` record walks of every walkable section
    /// (`(slot, walk)`, block = [`WALK_BLOCK`])
    pub walks: Vec<(u32, SectionWalk)>,
    /// the u16 tuning-parameter block @0x80 (`None` when the file is
    /// shorter than `0x98` bytes)
    pub params: Option<VocParams>,
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
        let params = VocParams::parse(voc);
        // the executed per-word `'f'` program report (roadmap v0.33.0): walk
        // the `+0x48` records and run each through the opcode-VM. The
        // structural skeleton — the `0x1004` child word counts need the
        // `DAT_1002ddec` pool (a DLL-image global, not dumped), so the
        // resolver is the always-`None` default.
        let f_programs = if table.per_word_f != 0 {
            let w = walk_section_records(voc, table.per_word_f, REC_MODE_F, WALK_BLOCK);
            let mut progs = Vec::new();
            for &start in &w.starts {
                let s = start as usize;
                let rec = voc.get(s..).unwrap_or(&[]);
                let blen = vm_program_byte_len(rec);
                let prog = rec.get(..blen).unwrap_or(rec);
                progs.push(run_f_program(prog, |_c: u16| None));
            }
            Some(progs)
        } else {
            None
        };
        Some(Morphology {
            table,
            affixes,
            suffixes,
            sec_64_words,
            digraphs,
            list_a_count,
            list_b_count,
            per_word_f_count,
            f_programs,
            walks,
            params,
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
// v0.30.0 (roadmap v0.31.0): the `.voc` root B-tree descent
// ---------------------------------------------------------------------------

/// One parsed `.voc` root B-tree node (roadmap v0.31.0) — the `FUN_10002b70`
/// case-fold descent node:
/// - byte 0: `0x80` = a child list is present; `0x40` = the `FUN_100028e0`
///   leaf key-walk path.
/// - byte 1: `0x80` = the child count is present; the low 7 bits = the child
///   count.
/// - bytes 2..: the u16 child list (the `DAT_1002db56` pool indices: `<3000`
///   = the `FUN_10002620` pool resolve; `3000..0xC1C` = the virtual
///   children, the `+0x48` offset under flags `0x1000`/`0x4000`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VocBTreeNode {
    /// The file offset of this node.
    pub off: u32,
    /// byte 0 & 0x80 — a child list is present.
    pub has_children: bool,
    /// byte 0 & 0x40 — the `FUN_100028e0` leaf key-walk path.
    pub is_leaf40: bool,
    /// byte 1 & 0x80 — the child count is present.
    pub has_count: bool,
    /// byte 1 & 0x7f — the child count.
    pub count: u8,
    /// The u16 child list (the pool indices; empty when the count is 0).
    pub children: Vec<u16>,
}

impl VocBTreeNode {
    /// Parse one node at the file offset `off`. Bounds-checked; `None` when
    /// the offset is out of range.
    pub fn parse(voc: &[u8], off: u32) -> Option<Self> {
        let o = off as usize;
        if o + 2 > voc.len() {
            return None;
        }
        let b0 = voc[o];
        let b1 = voc[o + 1];
        let has_children = b0 & 0x80 != 0;
        let is_leaf40 = b0 & 0x40 != 0;
        let has_count = b1 & 0x80 != 0;
        let count = (b1 & 0x7f) as u8;
        let mut children = Vec::new();
        if has_children && count > 0 {
            for i in 0..count as usize {
                let p = o + 2 + i * 2;
                if p + 1 < voc.len() {
                    children.push(u16::from_le_bytes([voc[p], voc[p + 1]]));
                } else {
                    break;
                }
            }
        }
        Some(VocBTreeNode {
            off,
            has_children,
            is_leaf40,
            has_count,
            count,
            children,
        })
    }

    /// The node's raw byte length (the 2-byte header + the u16 child list).
    pub fn raw_len(&self) -> usize {
        2 + self.children.len() * 2
    }

    /// The node's raw bytes (the header + the child list) — the structural
    /// decode (the case-fold emit is the DLL's `DAT_1002db1c`, a DLL-image
    /// global not dumped here).
    pub fn raw(&self, voc: &[u8]) -> Vec<u8> {
        let o = self.off as usize;
        let end = (o + self.raw_len()).min(voc.len());
        voc.get(o..end).map(|s| s.to_vec()).unwrap_or_default()
    }
}

/// The `.voc` root B-tree descent (roadmap v0.31.0) — the search chain open
/// since v0.15.0. `root` is the `+0x3C` root node file offset
/// (`VocSectionTable::root`); `pool` is the `DAT_1002db56` node-pool (the
/// u32 array the u16 children index into). The pool is a DLL-image global
/// (thread-offset) and is caller-supplied; an empty pool limits the descent
/// to the root's child list (the pool-location pin is roadmap-deferred).
#[derive(Debug, Clone, Default)]
pub struct VocBTree {
    /// The root node file offset (`+0x3C`).
    pub root: u32,
    /// The `DAT_1002db56` node-pool (the u32 node offsets).
    pub pool: Vec<u32>,
}

/// The leaf the B-tree descent lands on: the landing node's file offset
/// (`node_off`), the `.vt` record-stream offset it points to (`vt_off`), and
/// the landing node's raw bytes (structural).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VocLeaf {
    /// The landing node's file offset.
    pub node_off: u32,
    /// The `.vt` record-stream offset the leaf word points to.
    pub vt_off: u32,
    /// The landing node's raw bytes (structural; the case-fold emit is the
    /// DLL's `DAT_1002db1c`, a DLL-image global).
    pub key: Vec<u8>,
}

impl VocBTree {
    /// Build a tree from the section-table root + an optional pool.
    pub fn new(root: u32, pool: Vec<u32>) -> Self {
        VocBTree { root, pool }
    }

    /// Build from a `.voc` file (empty pool — the DLL pool is not dumped).
    pub fn parse(voc: &[u8]) -> Option<Self> {
        let table = VocSectionTable::parse(voc)?;
        Some(VocBTree::new(table.root, Vec::new()))
    }

    /// The root node.
    pub fn root_node(&self, voc: &[u8]) -> Option<VocBTreeNode> {
        VocBTreeNode::parse(voc, self.root)
    }

    /// A pool child's node offset (`pool[c]`), or `None` when the pool does
    /// not cover the index.
    pub fn child_offset(&self, child: u16) -> Option<u32> {
        self.pool.get(child as usize).copied()
    }

    /// The best-effort descent for `word` (the roadmap v0.31.0 DoD stretch):
    /// walk the root's `<3000` pool children; the deepest reachable leaf (a
    /// node with no child list) yields its key + `.vt` offset. With an empty
    /// pool the descent stops at the root (the DLL pool is not dumped), so the
    /// returned leaf is the root-stopped structural result. The case-fold
    /// compare of `word` needs the `DAT_1002e642` table (deferred). Bounds-
    /// checked; never panics.
    pub fn find(&self, voc: &[u8], word: &[u8]) -> Option<VocLeaf> {
        let _ = word; // the case-fold compare needs DAT_1002e642 (roadmap-deferred)
        let root = self.root_node(voc)?;
        for &c in &root.children {
            if c >= 3000 {
                continue; // the virtual children need the 0x1000/0x4000 flags
            }
            let Some(off) = self.child_offset(c) else {
                continue; // the pool does not cover this child
            };
            let Some(node) = VocBTreeNode::parse(voc, off) else {
                continue;
            };
            if !node.has_children || node.count == 0 {
                // a leaf node: the structural .vt landing is its pool offset
                return Some(VocLeaf {
                    node_off: off,
                    vt_off: off,
                    key: node.raw(voc),
                });
            }
        }
        // the descent stopped at the root (empty pool or all virtual children)
        Some(VocLeaf {
            node_off: self.root,
            vt_off: self.root,
            key: root.raw(voc),
        })
    }
}

// ---------------------------------------------------------------------------
// v0.31.0 (roadmap v0.32.0): the 4096-resync dense-node payload semantics
// ---------------------------------------------------------------------------

/// One decoded dense B-tree node — a 4096 resync slot (roadmap v0.32.0).
///
/// The v0.18.x walker resynchronizes the packed listA/listB regions onto the
/// 4096-aligned node slots (`0x69000..0x77000` in `lmu.voc`, step `0x1000`).
/// Each slot is a B-tree node (the `FUN_10002b70` header):
/// - byte 0: `0x80` = a u16 child list is present; `0x40` = the `FUN_100028e0`
///   leaf key-walk path (the low bits carry the leaf's key-run metadata).
/// - byte 1: `0x80` = the child / key count is present; low 7 bits = the
///   child / key count.
///
/// A child-list node's payload is `count` u16 child ids (pool-resolve or the
/// virtual `+0x48` offset); a leaf / key node's payload is the packed LmCode
/// key runs (case-folded bytes `<= 0x3F`, terminated by `0x00FF`-style
/// markers — the same byte-VM key encoding the `.vt` stream uses).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DenseNode {
    /// the slot's file offset (a 4096-aligned resync landing).
    pub off: u32,
    /// byte 0 (the child-list / `0x40`-leaf flags).
    pub hdr: u8,
    /// byte 1 (the count-present flag + the low-7 count).
    pub count_byte: u8,
    /// byte 0 & `0x80` — a u16 child list is present.
    pub has_child: bool,
    /// byte 0 & `0x40` — the `FUN_100028e0` leaf key-walk path.
    pub is_leaf40: bool,
    /// byte 1 & `0x80` — the child / key count is present.
    pub count_present: bool,
    /// byte 1 & `0x7f` — the child / key count.
    pub count: u8,
    /// the node's raw payload (the 2-byte header + the child list or key run).
    pub raw: Vec<u8>,
}

impl DenseNode {
    /// The node's raw extent: the 2-byte header + `count * 2` bytes for a
    /// child list, or the packed key run (bounded, `2 + count`) for a leaf.
    pub fn extent(&self) -> usize {
        if self.has_child {
            2 + (self.count as usize) * 2
        } else {
            2 + self.count as usize
        }
    }
}

/// The 722/2157 reconciliation facts (the roadmap v0.32.0 DoD stretch).
///
/// The section headers declare `listA` (722) and `listB` (2157) packed
/// records, but the pointer-array resync walk (block = [`WALK_BLOCK`] = 4096)
/// walks only `list_a_walked` / `list_b_walked` starts before it runs off the
/// file end — the remainder (the "dense" shortfall) are the node keys packed
/// inside the 4096-block data, consumed directly rather than via the pointer
/// array. The delta `listB − listA` is the roadmap-referenced `722/2157`
/// reconciliation (UA+EN below).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DenseReconcile {
    /// the listA section-header record count (722 in `lmu.voc`).
    pub list_a: u16,
    /// the listB section-header record count (2157 in `lmu.voc`).
    pub list_b: u16,
    /// the listA pointer-array walkable starts (388 in `lmu.voc`).
    pub list_a_walked: usize,
    /// the listB pointer-array walkable starts (382 in `lmu.voc`).
    pub list_b_walked: usize,
}

impl DenseReconcile {
    /// The listB − listA delta (1435 in `lmu.voc`).
    pub fn delta(&self) -> i32 {
        self.list_b as i32 - self.list_a as i32
    }

    /// The listA dense shortfall (`listA − walked`): the node keys packed in
    /// the 4096-block data, not the pointer array (334 in `lmu.voc`).
    pub fn list_a_dense(&self) -> usize {
        self.list_a as usize - self.list_a_walked
    }

    /// The listB dense shortfall (`listB − walked`) (1775 in `lmu.voc`).
    pub fn list_b_dense(&self) -> usize {
        self.list_b as usize - self.list_b_walked
    }
}

/// The 4096-resync dense-node region (roadmap v0.32.0): the per-slot decodes
/// (`slots`) + the listA/listB reconciliation counts. `slots` is the
/// data-driven set of 4096-aligned node slots the listA resync walk lands on
/// (`lmu.voc`: the 15 slots `0x69000..0x77000`).
///
/// **The in-memory word-struct layout** (the `FUN_10004370` oracle, documented
/// clean-room / file-referenced): the engine's per-section word array is a
/// NULL-terminated array of **word-struct pointers** (`param_3`), one per
/// record. Each struct's byte length is mode-specific (`param_4` = the mode
/// tag): `'n'` = `strlen` (a NUL-terminated LmCode run), `'e'` =
/// `FUN_100041d0`, `'f'` = `FUN_100044a0`, `-1` = `FUN_10004210`. The
/// validator advances each pointer by its length and asserts the byte right
/// past it is NUL (`*(ptr + len) == 0`); a non-NUL byte is a free/overrun
/// (`FUN_10021940`). The dense nodes decode the *on-disk* packed form of
/// exactly these per-word structs (the pointer array is the in-memory
/// consumer, the 4096-block data is the on-disk source).
#[derive(Debug, Clone, Default)]
pub struct VocDenseNodes {
    /// the per-slot node decodes (the 4096-aligned resync landings).
    pub slots: Vec<DenseNode>,
    /// the listA section-header record count (722 in `lmu.voc`).
    pub list_a_count: u16,
    /// the listB section-header record count (2157 in `lmu.voc`).
    pub list_b_count: u16,
    /// the listA pointer-array walkable starts (388 in `lmu.voc`).
    pub list_a_walked: usize,
    /// the listB pointer-array walkable starts (382 in `lmu.voc`).
    pub list_b_walked: usize,
}

impl VocDenseNodes {
    /// Decode the dense-node region from a `.voc` file: the 4096-aligned
    /// resync slots the listA/listB walks land on + the section-header record
    /// counts + the pointer-array walkable starts. `None` when the section
    /// table does not parse. The slot range is data-driven: the 4096-aligned
    /// offsets from `ceil(listA / 4096) * 4096` up to the earlier of the
    /// two walks' landings (or the file end).
    pub fn parse(voc: &[u8]) -> Option<Self> {
        let table = VocSectionTable::parse(voc)?;
        let list_a_count = u16_from(voc, table.list_a);
        let list_b_count = u16_from(voc, table.list_b);
        let la = walk_section_records(voc, table.list_a, REC_MODE_N, WALK_BLOCK);
        let lb = walk_section_records(voc, table.list_b, REC_MODE_N, WALK_BLOCK);
        // the 4096-aligned slot range: from the first block boundary at/after
        // listA up to the earlier walk landing (or the file end)
        let block = WALK_BLOCK as usize;
        let first = (table.list_a as usize / block).saturating_add(1) * block;
        let end = la.landed.min(lb.landed).min(voc.len() as u32) as usize;
        let mut slots = Vec::new();
        let mut off = first as u32;
        while (off as usize) < end {
            let o = off as usize;
            if o + 2 <= voc.len() {
                let b0 = voc[o];
                let b1 = voc[o + 1];
                let has_child = b0 & 0x80 != 0;
                let count = (b1 & 0x7f) as u8;
                let extent = if has_child {
                    2 + (count as usize) * 2
                } else {
                    2 + count as usize
                };
                let e = (o + extent).min(voc.len());
                let raw = voc.get(o..e).map(|s| s.to_vec()).unwrap_or_default();
                slots.push(DenseNode {
                    off,
                    hdr: b0,
                    count_byte: b1,
                    has_child,
                    is_leaf40: b0 & 0x40 != 0,
                    count_present: b1 & 0x80 != 0,
                    count,
                    raw,
                });
            }
            off = off.saturating_add(WALK_BLOCK);
        }
        Some(VocDenseNodes {
            slots,
            list_a_count,
            list_b_count,
            list_a_walked: la.starts.len(),
            list_b_walked: lb.starts.len(),
        })
    }

    /// The listA/listB reconciliation facts (the `722/2157` delta + the dense
    /// shortfalls).
    pub fn reconcile(&self) -> DenseReconcile {
        DenseReconcile {
            list_a: self.list_a_count,
            list_b: self.list_b_count,
            list_a_walked: self.list_a_walked,
            list_b_walked: self.list_b_walked,
        }
    }

    /// Structural validation (the roadmap v0.32.0 DoD, part-1): every slot's
    /// extent stays in file bounds, a child-list node fits one 4096 block, and
    /// the dense shortfalls are non-negative (the header count >= the walked
    /// count). Returns `(ok, mismatches)` — the number of structurally-valid
    /// slots and a human-readable issue list (empty when clean). The full
    /// LmCode key cross-check (every node key is a valid word vs the
    /// dictionary + `.vt`) needs the `DAT_1002e642` case-fold table — the
    /// same DLL-image global blocker as the `VmRecord` rate — and is the
    /// deferred stretch, not this structural pass.
    pub fn validate(&self, voc: &[u8]) -> (usize, Vec<String>) {
        let mut ok = 0usize;
        let mut issues = Vec::new();
        for node in &self.slots {
            let ext = node.extent();
            if node.off as usize + ext > voc.len() {
                issues.push(format!(
                    "{:#x}: extent {} overruns the file (len {})",
                    node.off,
                    ext,
                    voc.len()
                ));
            } else if node.has_child && (node.count as usize) * 2 > WALK_BLOCK as usize {
                issues.push(format!(
                    "{:#x}: child list (count {}) exceeds one 4096 block",
                    node.off, node.count
                ));
            } else {
                ok += 1;
            }
        }
        let rec = self.reconcile();
        if self.list_a_walked > self.list_a_count as usize {
            issues.push(format!(
                "listA: walked {} > header count {}",
                self.list_a_walked, self.list_a_count
            ));
        }
        if self.list_b_walked > self.list_b_count as usize {
            issues.push(format!(
                "listB: walked {} > header count {}",
                self.list_b_walked, self.list_b_count
            ));
        }
        if rec.delta() <= 0 {
            issues.push(format!(
                "delta listB-listA = {} (expected positive; the packed listB is larger)",
                rec.delta()
            ));
        }
        (ok, issues)
    }
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
    fn params_block_parses_the_real_lmu_voc() {
        let Some(voc) = std::fs::read(r"C:\Zakon.Local\Baza\SOFT\lmu.voc").ok() else {
            eprintln!("skipped: no base");
            return;
        };
        let p = VocParams::parse(&voc).expect("params");
        // the confirmed @0x80 values (see the byte dump)
        assert_eq!(
            p.raw,
            [
                0x0002, 0x0002, 0x0001, 0x0600, 0x0002, 0x0002, 0x0042, 0x2000, 0x0020,
                0x0108, 0x0104, 0x0010, 0x0002, 0x0004, 0x0080, 0x0400
            ]
        );
        assert_eq!(p.stop_marker(), 0x42, "listA stop marker 'B'");
        assert_eq!(p.page_const(), 0x2000);
        assert_eq!(p.sec_54_count(), 0x20, "sec_54 count");
        assert_eq!(p.gw_size(), 0x108, "lmr.gw size");
        // Morphology carries the block
        let m = Morphology::parse(&voc).expect("morph");
        assert!(m.params.is_some());
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

    // ------------------------------------------------------------------
    // v0.30.0 (roadmap v0.31.0): the `.voc` root B-tree descent
    // ------------------------------------------------------------------

    #[test]
    fn voc_btree_node_parses_the_real_lmu_root() {
        let Some(voc) = std::fs::read(r"C:\Zakon.Local\Baza\SOFT\lmu.voc").ok() else {
            eprintln!("skipped: no base");
            return;
        };
        let tree = VocBTree::parse(&voc).expect("the tree from the section table");
        // the root node decodes structurally (count = 26, all pool children)
        let root = tree.root_node(&voc).expect("the root node");
        assert!(root.has_children, "the root carries a child list");
        // 0xC0: both the 0x80 child-list and the 0x40 (FUN_100028e0) flags
        assert!(root.is_leaf40, "the root takes the 0x40-flag (FUN_100028e0) path");
        assert_eq!(root.count, 26, "the recorded root child count");
        assert_eq!(root.children.len(), 26, "the full child list is parsed");
        // every child is a pool-resolve index (<3000), not a virtual child
        assert!(
            root.children.iter().all(|&c| c < 3000),
            "all root children take the pool-resolve path"
        );
        // the find() descent with the empty pool stops at the root (no panic);
        // the word is the «конституція» DoD word (transliterated; the case-fold
        // compare needs the DAT_1002e642 table, roadmap-deferred)
        let leaf = tree.find(&voc, b"konstytutsia").expect("a structural leaf");
        assert_eq!(leaf.node_off, tree.root, "the empty-pool descent stops at the root");
        assert_eq!(leaf.vt_off, tree.root);
        println!(
            "lmu.voc B-tree root @0x{:x}: count={} all-pool; empty-pool find stops at root (recorded)",
            tree.root,
            root.count
        );
    }

    #[test]
    fn voc_btree_synthetic_pool_descends_to_a_leaf() {
        // a synthetic node image: the root has one child (index 5); pool[5]
        // points at a leaf node (no child list)
        let mut img = vec![0u8; 128];
        // leaf node @64: byte 0 = 0x00 (no child list), byte 1 = 0x00
        // (no count) — a leaf
        img[64] = 0x00;
        img[65] = 0x00;
        // root node @0: byte 0 = 0x80 (child list), byte 1 = 0x81 (count 1)
        img[0] = 0x80;
        img[1] = 0x81;
        img[2..4].copy_from_slice(&5u16.to_le_bytes()); // child index 5
        let mut pool = vec![0u32; 64];
        pool[5] = 64; // pool[5] = the leaf node offset
        // the section table is not present in the synthetic image, so build
        // the tree directly with the root offset
        let tree = VocBTree::new(0, pool);
        let leaf = tree.find(&img, b"x").expect("the leaf");
        assert_eq!(leaf.node_off, 64, "the descent reached the pool leaf");
        assert_eq!(leaf.vt_off, 64, "the leaf's .vt landing is its pool offset");
        assert_eq!(leaf.key.len(), 2, "the leaf's raw bytes (the 2-byte header)");
    }

    // ------------------------------------------------------------------
    // v0.31.0 (roadmap v0.32.0): the 4096-resync dense-node payload semantics
    // ------------------------------------------------------------------

    #[test]
    fn voc_dense_nodes_parse_the_real_lmu_slots() {
        let Some(voc) = std::fs::read(r"C:\Zakon.Local\Baza\SOFT\lmu.voc").ok() else {
            eprintln!("skipped: no base");
            return;
        };
        let dn = VocDenseNodes::parse(&voc).expect("the dense-node region");
        // the 15 confirmed 4096-aligned resync slots (0x69000..0x77000)
        assert_eq!(dn.slots.len(), 15, "the 15 resync slots");
        assert_eq!(dn.slots[0].off, 0x69000, "the first slot");
        assert_eq!(dn.slots.last().unwrap().off, 0x77000, "the last slot");
        // the section-header record counts (the 722/2157 pair)
        assert_eq!(dn.list_a_count, 722, "listA header count");
        assert_eq!(dn.list_b_count, 2157, "listB header count");
        // the pointer-array walkable starts (the resync walk, block 4096)
        assert_eq!(dn.list_a_walked, 388, "listA walked starts");
        assert_eq!(dn.list_b_walked, 382, "listB walked starts");
        // exactly one child-list node (0x77000); the rest are leaf/key nodes
        assert_eq!(
            dn.slots.iter().filter(|n| n.has_child).count(),
            1,
            "only 0x77000 carries a child list"
        );
        let rec = dn.reconcile();
        assert_eq!(rec.delta(), 1435, "listB - listA delta (2157 - 722)");
        assert_eq!(rec.list_a_dense(), 334, "listA dense shortfall (722 - 388)");
        assert_eq!(rec.list_b_dense(), 1775, "listB dense shortfall (2157 - 382)");
        println!(
            "lmu.voc dense nodes: {} slots · 722/2157 delta {} · dense shortfalls {}/{} (recorded)",
            dn.slots.len(),
            rec.delta(),
            rec.list_a_dense(),
            rec.list_b_dense()
        );
    }

    #[test]
    fn voc_dense_nodes_validate_is_clean_on_the_real_lmu() {
        let Some(voc) = std::fs::read(r"C:\Zakon.Local\Baza\SOFT\lmu.voc").ok() else {
            eprintln!("skipped: no base");
            return;
        };
        let dn = VocDenseNodes::parse(&voc).expect("the dense-node region");
        let (ok, issues) = dn.validate(&voc);
        assert_eq!(ok, 15, "every slot's extent stays in bounds and fits a block");
        assert!(
            issues.is_empty(),
            "the structural validation is clean (got {issues:?})"
        );
    }

    #[test]
    fn dense_node_extent_is_child_times_two() {
        // a child-list node: extent = 2 + count*2 (the u16 child list)
        let child = DenseNode {
            off: 0x77000,
            hdr: 0xAE,
            count_byte: 0xF9,
            has_child: true,
            is_leaf40: false,
            count_present: true,
            count: 121,
            raw: vec![0u8; 244],
        };
        assert_eq!(child.extent(), 2 + 121 * 2, "the child-list extent");
        // a leaf / key node: extent = 2 + count (the packed key run, bounded)
        let leaf = DenseNode {
            off: 0x72000,
            hdr: 0x5E,
            count_byte: 0x7F,
            has_child: false,
            is_leaf40: true,
            count_present: false,
            count: 127,
            raw: vec![0u8; 129],
        };
        assert_eq!(leaf.extent(), 2 + 127, "the leaf key-run extent");
    }

    #[test]
    fn dense_reconcile_delta_and_dense_shortfalls() {
        // the recorded lmu.voc reconciliation (the 722/2157 delta + shortfalls)
        let rec = DenseReconcile {
            list_a: 722,
            list_b: 2157,
            list_a_walked: 388,
            list_b_walked: 382,
        };
        assert_eq!(rec.delta(), 1435, "listB - listA (2157 - 722)");
        assert_eq!(rec.list_a_dense(), 334, "listA header - walked (722 - 388)");
        assert_eq!(rec.list_b_dense(), 1775, "listB header - walked (2157 - 382)");
    }

    // ------------------------------------------------------------------
    // v0.32.0 (roadmap v0.33.0): the per-word `'f'` opcode-VM execution
    // ------------------------------------------------------------------

    #[test]
    fn run_f_program_top_level_terminator() {
        // a nested program: 0x1001 (enter), 0x1003 (word leaf), 0x1002
        // (close the subtree), 0x1002 (top-level terminate)
        let prog: [u8; 8] = [
            0x01, 0x10, 0x03, 0x10, 0x02, 0x10, 0x02, 0x10,
        ];
        let r = run_f_program(&prog, |_c: u16| None);
        assert_eq!(r.byte_len, 8, "the full program up to the top-level 0x1002");
        assert_eq!(r.word_count, 1, "the single 0x1003 word leaf");
        assert!(r.terminated, "it terminated at the top-level 0x1002");
        assert!(r.data.is_empty(), "no inline data u16s");
    }

    #[test]
    fn run_f_program_child_resolver_adds_the_child_word_count() {
        // a program: 0x1004 [child ptr 7] (the resolver yields 5 words), then
        // a top-level 0x1002
        let prog: [u8; 6] = [0x04, 0x10, 0x07, 0x00, 0x02, 0x10];
        let r = run_f_program(&prog, |c: u16| {
            assert_eq!(c, 7, "the resolver sees the child pointer");
            Some(5)
        });
        assert_eq!(r.word_count, 5, "the child subtree's word count");
        assert_eq!(r.data, vec![7], "the child pointer is recorded in the data");
        assert!(r.terminated, "it terminated at the top-level 0x1002");
    }

    #[test]
    fn run_f_program_collects_inline_data_u16() {
        // a data u16 (0x0024) + a 0x1003 word leaf + a top-level 0x1002
        let prog: [u8; 6] = [0x24, 0x00, 0x03, 0x10, 0x02, 0x10];
        let r = run_f_program(&prog, |_c: u16| None);
        assert_eq!(r.data, vec![0x24], "the inline data u16 is collected");
        assert_eq!(r.word_count, 1, "the 0x1003 word leaf");
        assert!(r.terminated);
    }

    #[test]
    fn morphology_f_programs_run_the_real_lmu_programs() {
        let Some(voc) = std::fs::read(r"C:\Zakon.Local\Baza\SOFT\lmu.voc").ok() else {
            eprintln!("skipped: no base");
            return;
        };
        let m = Morphology::parse(&voc).expect("morph");
        // the `+0x48` per-word 'f' report: 142 declared records, but the
        // record walk (block 4096) resynchronizes onto the dense nodes, so
        // the walked starts are fewer than the declared count
        let progs = m.f_programs.as_ref().expect("the 'f' program report");
        assert!(
            !progs.is_empty(),
            "at least one 'f' program is executed"
        );
        let terminated = progs.iter().filter(|p| p.terminated).count();
        let sum_words: u32 = progs.iter().map(|p| p.word_count).sum();
        println!(
            "lmu.voc 'f' programs: {} declared / {} walked · terminated {} · sum word-count (leaf hits) {} (recorded)",
            m.per_word_f_count,
            progs.len(),
            terminated,
            sum_words
        );
        // the structural invariant: every walked program terminates at the
        // top-level 0x1002 (the count needs the DAT_1002ddec pool, deferred)
        assert_eq!(
            terminated,
            progs.len(),
            "every walked 'f' program terminates at the top-level 0x1002"
        );
    }
}
