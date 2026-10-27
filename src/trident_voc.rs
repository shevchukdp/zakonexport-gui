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
//! The per-word `'f'` records and the root index table are little-endian
//! **u16 instruction programs** executed by `FUN_100045e0`:
//!
//! - `0x1001` — enter a nested subtree (recurse, then skip via
//!   `FUN_100044f0` and add the subtree's word count);
//! - `0x1002` — terminator (top-level returns the program length);
//! - `0x1003` — word leaf (word counter + 1);
//! - `0x1004 [u16 ptr]` — child node (via a resolver; `child[0]` = the child
//!   word count);
//! - any other u16 — inline data collected into an output buffer.
//!
//! `vm_program_byte_len` is `FUN_100044f0`: it walks the u16 program (a byte
//! pointer advanced by 2 per instruction), tracking a `0x1001`/`0x1002`
//! depth counter, and returns the **byte** offset of the first instruction
//! after the top-level `0x1002`. `vm_record_len` is `FUN_100044a0` =
//! `L + 3 * *(byte at L) + 1`, where `L = vm_record_scan_len` (the program length
//! plus the trailing byte scan up to a `0xFF` marker, per `FUN_100044c0`).

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
        Some(Morphology {
            table,
            affixes,
            suffixes,
            sec_64_words,
            digraphs,
            list_a_count,
            list_b_count,
            per_word_f_count,
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
