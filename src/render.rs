//! Document render engine: docloc -> text\N.mb container -> chunk -> mode A/B
//! record split -> token expansion via the words.db/words.mb dictionary.
//!
//! Grammar proven against the app (see C:\Zakon.Local\REVERSE.md):
//! - docloc.db: 5-byte records at offset 12, [mode u8][file BE16][chunk BE16]
//!   (word order proven by chunk-vs-container-count consistency: 612900/612900)
//! - text\N.mb: count u16BE@7; size(c)=u24BE@6+3c; chunk c data @9+3*count
//! - mode 1 chunk (splitter B, FUN_00654150):
//!     u24BE@0 = items-section end == records-section offset
//!     @recs: u16BE record count; per record: flags byte, len-code = flags>>6
//!     (0->u8, 1->u16BE, 2->u24BE; 3 unmapped), len field, data.
//!     text record <=> (flags & 7) == 1  (record +0xe subtype 0)
//! - mode 0 chunk (splitter A, FUN_00654880):
//!     item count varint (b&0x7f; b>=0x80 -> ((b<<8)|b1)&0x7fff);
//!     items: param (2B if first byte >=0x80 else 3B), flags byte with bits
//!     0-5 escaped at 0x3f, bit7 => u24 id +3B, bit6 => attrs(varint head),
//!     records: count varint (b&0x3f + code*0x40 logic), per record: flags
//!     byte, type=(flags>>3)&7 escaped at 7 (+byte, cursor+1),
//!     sub=flags&7 escaped at 7, optional varint when type not in {0,8},
//!     len: code0=>0, code1=>u8, code2=>u16BE, code3=>u24BE.
//!     text record <=> type == 0 (flags & 0x38 == 0).
//! - token expander (FUN_00662e48): 0x80 literal, 0x81 n spaces, 0x82 8B
//!   style switch, 0x83 commit (\n), bytes >= 0x84 single-byte dict id
//!   (b-0x80), varint: continuations < 0x80, terminator >= 0x80 (bit7 masked,
//!   control bytes qualify when following an open varint), id = 7-bit
//!   groups MSB-first (pack7). Word = words.db row (12 + id*3, BE24 pool
//!   offset) -> words.mb[7 + ofs .. NUL). Boundary space when previous
//!   element's first-char class == 0 and next first char's class == 0
//!   (class table DAT_00667624, ripped to rosetta\cls256.bin).

use anyhow::{bail, Context, Result};
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex};

// ============================================================
// class table (app DAT_00667624; equals rosetta\cls256.bin)
// ============================================================

pub const CLASS_TABLE: [u8; 256] = [
    1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, //
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, //
    1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, //
    1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, //
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, //
    1, 1, 1, 1, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
];

fn first_class(w: &[u8]) -> u8 {
    match w.first() {
        Some(&b) => CLASS_TABLE[b as usize],
        None => 1,
    }
}

// ============================================================
// words.db (dict A rows) + words.mb (dict B pool)
// ============================================================

pub struct Words {
    rows: Vec<u8>,
    pool: Vec<u8>,
    count: u32,
}

impl Words {
    pub fn load(baza: &Path) -> Result<Self> {
        let rows = std::fs::read(baza.join("words.db")).context("read words.db")?;
        let pool = std::fs::read(baza.join("words.mb")).context("read words.mb")?;
        if rows.len() < 9 {
            bail!("words.db too small");
        }
        let count = ((rows[6] as u32) << 16) | ((rows[7] as u32) << 8) | rows[8] as u32;
        if rows.len() < 12 + count as usize * 3 {
            bail!("words.db: {} bytes < needed {} for count {}", rows.len(), 12 + count as usize * 3, count);
        }
        Ok(Self { rows, pool, count })
    }

    pub fn count(&self) -> u32 {
        self.count
    }

    /// cp1251 word bytes for a dict id (pool base offset 7 per the app).
    pub fn word(&self, id: u32) -> Option<&[u8]> {
        if id >= self.count {
            return None;
        }
        let ro = 12 + id as usize * 3;
        if ro + 3 > self.rows.len() {
            return None;
        }
        let ofs = ((self.rows[ro] as usize) << 16)
            | ((self.rows[ro + 1] as usize) << 8)
            | self.rows[ro + 2] as usize;
        let start = 7usize.checked_add(ofs)?;
        if start >= self.pool.len() {
            return None;
        }
        let end = self.pool[start..]
            .iter()
            .position(|&b| b == 0)
            .map_or(self.pool.len(), |p| start + p);
        Some(&self.pool[start..end])
    }
}

// ============================================================
// container / chunk access
// ============================================================

/// Slice one chunk out of a text\N.mb container.
pub fn chunk_slice<'a>(data: &'a [u8], chunk: u32) -> Result<&'a [u8]> {
    if data.len() < 10 {
        bail!("container too small");
    }
    let count = ((data[7] as usize) << 8) | data[8] as usize;
    if chunk == 0 || chunk as usize > count {
        bail!("chunk {chunk} out of range (count {count})");
    }
    let mut ofs = 9 + 3 * count;
    let mut size = 0usize;
    for i in 1..=chunk as usize {
        let o = 6 + 3 * i;
        if o + 3 > data.len() {
            bail!("size table truncated");
        }
        let s = ((data[o] as usize) << 16) | ((data[o + 1] as usize) << 8) | data[o + 2] as usize;
        if i < chunk as usize {
            ofs += s;
        } else {
            size = s;
        }
    }
    if ofs + size > data.len() {
        bail!("chunk {chunk} beyond file end");
    }
    Ok(&data[ofs..ofs + size])
}

fn be16(d: &[u8], p: usize) -> Option<u32> {
    if p + 2 > d.len() {
        return None;
    }
    Some(((d[p] as u32) << 8) | d[p + 1] as u32)
}

fn be24(d: &[u8], p: usize) -> Option<u32> {
    if p + 3 > d.len() {
        return None;
    }
    Some(((d[p] as u32) << 16) | ((d[p + 1] as u32) << 8) | d[p + 2] as u32)
}

// ============================================================
// record split (both modes)
// ============================================================

#[derive(Debug, Clone, Copy)]
pub struct Rec {
    pub start: usize,
    pub len: usize,
    pub text: bool,
    /// payload = RichView item stream walked by the item dispatcher
    /// (FUN_00662a7c family), not a plain token run.
    pub walk: bool,
}

fn clamp_rec(recs: &mut Vec<Rec>, start: usize, len: usize, text: bool, walk: bool, chunk: &[u8]) {
    if start < chunk.len() {
        let len = len.min(chunk.len() - start);
        recs.push(Rec { start, len, text, walk });
    }
}

/// Mode 1 (splitter B): items skipped via the u24BE@0 boundary.
pub fn recs_mode_b(chunk: &[u8]) -> Vec<Rec> {
    let mut recs = Vec::new();
    let recs_at = be24(chunk, 0).unwrap_or(0) as usize;
    if recs_at < 3 || recs_at + 2 > chunk.len() {
        return recs;
    }
    let count = match be16(chunk, recs_at) {
        Some(n) => n as usize,
        None => return recs,
    };
    if count > 200_000 {
        return recs;
    }
    let mut p = recs_at + 2;
    for _ in 0..count {
        if p >= chunk.len() {
            break;
        }
        let fl = chunk[p];
        p += 1;
        let code = (fl >> 6) as usize;
        if code > 2 {
            break; // len-code 3 unmapped in the app
        }
        let field = code + 1; // len-field byte count: 1, 2 or 3
        if p + field > chunk.len() {
            break;
        }
        let len = (0..field).fold(0usize, |v, k| (v << 8) | chunk[p + k] as usize);
        p += field;
        let start = p;
        // record +0xe subtype = (fl & 7) - 1: 0 = plain token paragraph, 1-2 =
        // RichView item streams rendered via FUN_005f0740 -> dispatcher
        // FUN_00662a7c. Record +0xf ((fl>>3)&7) is a separate style field that
        // does NOT affect routing (old prose records carry typ 7 there).
        let text = matches!(fl & 7, 1..=3);
        let walk = matches!(fl & 7, 2..=3); // subtype 1..2 -> item stream
        clamp_rec(&mut recs, start, len, text, walk, chunk);
        p = start + len;
    }
    recs
}

/// Mode 0 (splitter A): items must be walked (no boundary constant).
pub fn recs_mode_a(chunk: &[u8]) -> Vec<Rec> {
    let mut recs = Vec::new();
    let n = chunk.len();
    if n == 0 {
        return recs;
    }
    // item count varint
    let b0 = chunk[0];
    let mut p = 1usize;
    let mut items = (b0 & 0x7f) as u32;
    if b0 >= 0x80 {
        items = (((b0 as u32) << 8) | chunk.get(1).copied().unwrap_or(0) as u32) & 0x7fff;
        p = 2;
    }
    if items > 200_000 {
        return recs;
    }
    for _ in 0..items {
        if p >= n {
            return recs;
        }
        if chunk[p] >= 0x80 {
            p += 2;
        } else {
            p += 3;
        }
        if p >= n {
            return recs;
        }
        let fl = chunk[p];
        p += 1;
        if (fl & 0x3f) == 0x3f {
            p += 1; // bits 0-5 escape
        }
        if fl & 0x80 != 0 {
            p += 3; // subtype byte (shared) + u24 id
        }
        if fl & 0x40 != 0 {
            if p >= n {
                return recs;
            }
            let head = chunk[p];
            let code = (head >> 6) as usize;
            p += 1;
            if p + code > n {
                return recs;
            }
            let mut value = 0u32;
            for k in 0..code {
                value = (value << 8) | chunk[p + k] as u32;
            }
            p += code;
            let basis = (head & 7) as usize + value as usize * 8;
            let width = ((head >> 3) & 3) as usize + 1; // bytes per entry
            let repeat = ((head >> 5) & 1) as usize + 1;
            p = p.saturating_add(basis * repeat * width);
        }
    }
    if p >= n {
        return recs;
    }
    // record count varint: (b & 0x3f) + value * 0x40
    let head = chunk[p];
    let code = (head >> 6) as usize;
    p += 1;
    if p + code > n {
        return recs;
    }
    let value = (0..code).fold(0u32, |v, k| (v << 8) | chunk[p + k] as u32);
    p += code;
    let count = (head & 0x3f) as usize + value as usize * 0x40;
    if count > 200_000 {
        return recs;
    }
    for _ in 0..count {
        if p >= n {
            break;
        }
        let fl = chunk[p];
        let mut q = p + 1;
        let mut typ = ((fl >> 3) & 7) as u32;
        if typ == 7 {
            if q >= n {
                break;
            }
            typ = (chunk[q] as u32) + 7;
            q += 1;
        }
        let mut sub = (fl & 7) as u32;
        if sub == 7 {
            if q >= n {
                break;
            }
            sub = (chunk[q] as u32) + 7; // record +0xf: style slot, not routed
            q += 1;
        }
        let _ = sub;
        if typ != 0 && typ != 8 {
            // optional varint record header (values unused here)
            if q >= n {
                break;
            }
            let code = (chunk[q] >> 6) as usize;
            q += 1 + code;
        }
        if q > n {
            break;
        }
        let lcode = (fl >> 6) as usize;
        let mut len = 0usize;
        if lcode > 0 {
            if q + lcode > n {
                break;
            }
            len = (0..lcode).fold(0usize, |v, k| (v << 8) | chunk[q + k] as usize);
            q += lcode;
        }
        let start = q;
        // The app's record dispatcher (FUN_006565a8) renders type 0 through the
        // direct text expander and types 1-2 through the list engine
        // FUN_005f0740, which funnels into the RichView item dispatcher. Types 3+
        // use other engines (frames/tables); they are skipped for text search.
        let text = typ <= 2;
        let walk = (1..=2).contains(&typ);
        clamp_rec(&mut recs, start, len, text, walk, chunk);
        p = start + len;
    }
    recs
}

// ============================================================
// token expansion (FUN_00662e48 replica)
// ============================================================

struct Expander<'a> {
    words: &'a Words,
    out: Vec<u8>,
    prev: u8,
}

impl<'a> Expander<'a> {
    fn push_word(&mut self, w: &[u8]) {
        if w.is_empty() {
            return;
        }
        if self.prev == 0 && first_class(w) == 0 {
            self.out.push(b' ');
        }
        self.out.extend_from_slice(w);
        self.prev = first_class(w);
    }

    fn expand(mut self, data: &[u8]) -> Vec<u8> {
        let mut i = 0usize;
        while i < data.len() {
            let b = data[i];
            match b {
                0x80 => {
                    // literal run: len byte + bytes
                    if i + 2 > data.len() {
                        break;
                    }
                    let n = data[i + 1] as usize;
                    let s = i + 2;
                    let e = (s + n).min(data.len());
                    self.push_word(&data[s..e]);
                    i = e;
                }
                0x81 => {
                    // n spaces
                    if i + 2 > data.len() {
                        break;
                    }
                    let n = data[i + 1] as usize;
                    self.out.extend(std::iter::repeat(b' ').take(n.min(220)));
                    self.prev = 1;
                    i += 2;
                }
                0x82 => {
                    // style switch: 8 bytes total, no text
                    i += 8;
                }
                0x83 => {
                    // commit -> newline
                    self.out.push(b'\n');
                    self.prev = 1;
                    i += 1;
                }
                0x84..=0xFF => {
                    let id = b as u32 - 0x80;
                    if let Some(w) = self.words.word(id) {
                        self.push_word(w);
                    }
                    i += 1;
                }
                _ => {
                    // varint: continuations < 0x80, terminator >= 0x80 (bit7 masked)
                    // — the terminator may also be a control byte (0x80-0x83)
                    // when it *follows* an open varint (e.g. `02 81` -> id 257 "л")
                    let mut v = b as u32;
                    let mut j = i + 1;
                    let mut cont = 0usize;
                    let mut ok = false;
                    while j < data.len() {
                        let c = data[j];
                        if c >= 0x80 {
                            v = (v << 7) | (c as u32 & 0x7f);
                            j += 1;
                            ok = true;
                            break;
                        }
                        v = (v << 7) | c as u32;
                        j += 1;
                        cont += 1;
                        if cont >= 3 {
                            break;
                        }
                    }
                    if ok {
                        if let Some(w) = self.words.word(v) {
                            self.push_word(w);
                        }
                    }
                    i = j;
                }
            }
            if self.out.len() > 8_000_000 {
                break; // paranoia cap
            }
        }
        self.out
    }
}

pub fn expand_tokens(data: &[u8], words: &Words) -> Vec<u8> {
    Expander { words, out: Vec::with_capacity(data.len() * 2 + 16), prev: 1 }.expand(data)
}

// ============================================================
// RichView item-stream walk (FUN_00662a7c dispatcher replica)
// ============================================================
//
// Subtype-1/2 (mode B) and type-1/2 (mode A) text records store a sequence of
// RichView items, not a plain token run. The dispatcher walks items until a
// 0xFF terminator, consuming one leading 0x00 byte if present. Item handlers
// per code (from decomp_rvitems.out):
//   text items 01,13-1C,1E-3D,40-4F -> FUN_00661554 (header + varint run)
//   02,0D  -> table  (FUN_00661dc0)
//   03     -> para+picture (FUN_00661300)
//   04/05  -> fixed structures (+10/+11)
//   06,08,11,12 -> references into the doc-level item bag (FUN_006628fc);
//                  content lives outside the record - skipped
//   07     -> object blob (FUN_00660aa0)
//   09     -> tab stops  (FUN_00660be8: count + per tab skipFont + 0x12)
//   0A     -> paragraph info x count (+0x29 each)
//   0B     -> numbering levels (FUN_0066108c)
//   0C     -> +13
//   0E,0F,10,3E,3F -> span groups with nested cell streams (FUN_0066280c)
//   1D     -> count recursive sub-collections (FUN_006627a8)

/// FUN_0065d6cc string field: [len u8][len bytes]
fn skip_str(d: &[u8], p: usize) -> usize {
    match d.get(p) {
        Some(&n) => 1 + n as usize,
        None => 1,
    }
}

/// FUN_0065d758 font-name field: 0x00 -> [00][len u8][bytes], else 1 byte
fn skip_font(d: &[u8], p: usize) -> usize {
    match d.get(p) {
        Some(0) => 1 + skip_str(d, p + 1),
        Some(_) => 1,
        None => 1,
    }
}

/// FUN_0065dc38 picture field: [kind u8][size u24BE][size bytes]
/// (advance is 4 + size regardless of the kind byte)
fn skip_pic(d: &[u8], p: usize) -> usize {
    match d.get(p) {
        Some(_) => 4 + be24(d, p + 1).unwrap_or(0) as usize,
        None => 1,
    }
}

/// Item-length varint: bytes < 0x80 continue, first byte >= 0x80 terminates;
/// each byte contributes the next higher 7-bit group.
fn read_varint(d: &[u8], mut p: usize) -> (u32, usize) {
    let mut v: u32 = 0;
    while p < d.len() {
        let b = d[p];
        p += 1;
        v = (b as u32 & 0x7f).wrapping_add(v.wrapping_mul(0x80));
        if b >= 0x80 {
            break;
        }
    }
    (v, p)
}

/// Walk one item stream [p..end), appending decoded text to `out`.
/// Returns the position right after the 0xFF terminator (or `end`).
fn walk_items(
    data: &[u8],
    mut p: usize,
    end: usize,
    words: &Words,
    out: &mut Vec<u8>,
    depth: u32,
) -> usize {
    if p < end && data[p] == 0x00 {
        p += 1; // dispatcher pre-loop skips one leading 0x00
    }
    let mut limit = 0usize;
    while p < end && data[p] != 0xFF {
        limit += 1;
        if limit > 100_000 || out.len() > 8_000_000 || depth > 24 {
            break;
        }
        let c = data[p];
        let mut q = p + 1;

        if c == 0x00 {
            // not in the app switch; defensive skip
            p = q;
            continue;
        }
        if c == 0x02 || c == 0x0D {
            // table: [code][rows u16BE][cols u8]; 0x0D header = 10 bytes,
            // 0x02 = +0x1f + [len u8][len bytes]
            let rows = be16(data, p + 1).unwrap_or(0) as usize;
            let cols = data.get(p + 3).copied().unwrap_or(0) as usize;
            q = if c == 0x0D {
                p + 10
            } else {
                p + 0x1f + 1 + data.get(p + 0x1f).copied().unwrap_or(0) as usize
            };
            if q > end {
                break;
            }
            'rows: for _r in 0..rows {
                for _k in 0..cols {
                    if q >= end {
                        break 'rows;
                    }
                    let b = data[q];
                    if b == 0 {
                        q += 1; // empty cell
                        continue;
                    }
                    q += if b & 0x80 != 0 { 6 } else { 16 }; // cell info
                    if q > end {
                        break 'rows;
                    }
                    q = walk_items(data, q, end, words, out, depth + 1);
                    if out.last() != Some(&b'\n') {
                        out.push(b'\n');
                    }
                }
                if q >= end {
                    break;
                }
            }
            p = q.min(end);
            continue;
        }
        if c == 0x03 {
            // para + picture: [03][n u8][n bytes][16 bytes][img kind][u24 size][bytes]
            let n = data.get(p + 1).copied().unwrap_or(0) as usize;
            let mut q = p + 2 + n + 16;
            let sz = be24(data, q + 1).unwrap_or(0) as usize;
            q += 4 + sz;
            p = q.min(end);
            continue;
        }
        if c == 0x04 {
            p = (p + 10).min(end);
            continue;
        }
        if c == 0x05 {
            p = (p + 11).min(end);
            continue;
        }
        if c == 0x06 {
            p = (p + 2).min(end);
            continue;
        }
        if c == 0x07 {
            p = if data.get(p + 6) == Some(&1) {
                (p + be24(data, p + 7).unwrap_or(0) as usize + 10).min(end)
            } else {
                (p + 7).min(end)
            };
            continue;
        }
        if c == 0x08 {
            p = (p + 3).min(end);
            continue;
        }
        if c == 0x09 {
            // tab stops: [code][count u16BE]; per tab: skipFont + 0x12
            let count = be16(data, p + 1).unwrap_or(0) as usize;
            q = p + 3;
            for _k in 0..count {
                if q >= end {
                    break;
                }
                q += skip_font(data, q) + 0x12;
            }
            p = q.min(end);
            continue;
        }
        if c == 0x0A {
            // paragraph info x count, fixed 0x29 bytes each
            let count = be16(data, p + 1).unwrap_or(0) as usize;
            p = (p + 3 + count * 0x29).min(end);
            continue;
        }
        if c == 0x0B {
            // numbering levels: [code][count u16BE]; per level [fl u8] with
            // inner = fl>>1 groups; per group: [2 bytes][str][str][3x u16BE +
            // flag u8]; flag&1 -> font + 8, flag&2 -> picture (FUN_0065dc38)
            let count = be16(data, p + 1).unwrap_or(0) as usize;
            q = p + 3;
            for _o in 0..count {
                if q >= end {
                    break;
                }
                let lvl = data[q];
                q += 1;
                let inner = (lvl >> 1) as usize;
                for _k in 0..inner {
                    if q + 2 > end {
                        break;
                    }
                    q += 2; // [bits u8][x u8]
                    let s1 = skip_str(data, q);
                    q += s1;
                    let s2 = skip_str(data, q);
                    q += s2;
                    if q + 7 > end {
                        break;
                    }
                    let flag = data[q + 6]; // 7th byte controls the extras
                    q += 7;
                    if flag & 1 != 0 {
                        q += skip_font(data, q) + 8;
                    }
                    if flag & 2 != 0 {
                        q += skip_pic(data, q);
                    }
                    if q > end {
                        break;
                    }
                }
            }
            p = q.min(end);
            continue;
        }
        if c == 0x0C {
            p = (p + 13).min(end);
            continue;
        }
        if c == 0x0E || c == 0x0F || c == 0x10 || c == 0x3E || c == 0x3F {
            // span groups with nested cell streams (FUN_0066280c)
            let count;
            if c == 0x0E || c == 0x3E {
                count = data.get(p + 1).copied().unwrap_or(0) as usize;
                q = p + 2;
            } else {
                count = be16(data, p + 1).unwrap_or(0) as usize;
                q = p + 3;
            }
            let val = if c == 0x10 {
                let v = be16(data, q).unwrap_or(0);
                q += 2;
                v
            } else {
                let v = data.get(q).copied().unwrap_or(0) as u32;
                q += 1;
                v
            };
            if q > end {
                break;
            }
            for _k in 0..count {
                if q >= end {
                    break;
                }
                if c == 0x3E || c == 0x3F {
                    if data[q] == 0xFF {
                        q += 1;
                        continue;
                    }
                } else {
                    q += if val < 0x100 { 1 } else { 2 }; // cell span header
                    if q > end {
                        break;
                    }
                }
                q = walk_items(data, q, end, words, out, depth + 1);
            }
            p = q.min(end);
            continue;
        }
        if c == 0x11 {
            p = (p + 3).min(end);
            continue;
        }
        if c == 0x12 {
            p = (p + 6).min(end);
            continue;
        }
        if c == 0x1D {
            // count nested 0xFF-terminated sub-collections
            let count = be16(data, p + 1).unwrap_or(0) as usize;
            q = p + 3;
            for _k in 0..count {
                if q >= end {
                    break;
                }
                q = walk_items(data, q, end, words, out, depth + 1);
            }
            p = q.min(end);
            continue;
        }

        // ---- text items (FUN_00661554) ----
        let hlen = if c == 0x01 {
            6usize
        } else if matches!(c, 0x13..=0x15 | 0x19 | 0x1A) {
            3
        } else if matches!(c, 0x16..=0x18 | 0x1B | 0x1C) {
            4
        } else if matches!(c, 0x1E..=0x3D | 0x40..=0x4F) {
            2
        } else {
            p = q; // unknown opcode in a valid stream; skip defensively
            continue;
        };
        q = p + hlen;
        // inline blob (picture/number data) for 01/14/17
        if c == 0x01 || c == 0x14 || c == 0x17 {
            q += 1 + data.get(q).copied().unwrap_or(0) as usize;
        }
        // number-as-string fields for 15/18 (u24BE), 19/1b (u8), 1a/1c (u16BE)
        if c == 0x15 || c == 0x18 {
            q += 3;
        } else if c == 0x19 || c == 0x1B {
            q += 1;
        } else if c == 0x1A || c == 0x1C {
            q += 2;
        }
        if q > end {
            break;
        }
        let (run_len, run_at) = read_varint(data, q);
        let run_end = (run_at + run_len as usize).min(end);
        if run_end > run_at {
            let seg = Expander { words, out: Vec::with_capacity(run_len as usize * 2 + 8), prev: 1 }
                .expand(&data[run_at..run_end]);
            if !seg.is_empty() {
                if !out.is_empty() && out.last() != Some(&b'\n') {
                    out.push(b'\n');
                }
                out.extend_from_slice(&seg);
            }
        }
        p = run_end;
    }
    if p < end && data[p] == 0xFF {
        p += 1; // consume the 0xFF terminator
    }
    p
}

/// Render one subtype-1/2 / type-1/2 record: walk its RichView item stream.
fn walk_record(chunk: &[u8], start: usize, end: usize, words: &Words) -> Vec<u8> {
    let mut out = Vec::new();
    walk_items(chunk, start, end, words, &mut out, 0);
    out
}

/// Strip RichView `~{...}` markers from expanded text.
///
/// The app replaces `~{<kind>;;<text>;b<N>}` with a font/section item whose
/// visible text is `<text>`; plain `~{...}` without `;;` is style-only and is
/// dropped. Applies to both new-writer docs and fonts in old ones.
pub fn strip_rv_markers(data: &mut Vec<u8>) {
    let d = data.clone();
    let mut out: Vec<u8> = Vec::with_capacity(d.len() + 16);
    let mut i = 0usize;
    let mut changed = false;
    while i < d.len() {
        if d[i] == b'~' && d.get(i + 1) == Some(&b'{') {
            if let Some(close) = d[i + 2..].iter().position(|&b| b == b'}').map(|k| i + 2 + k) {
                let inner = &d[i + 2..close];
                if let Some(s) = find_seq(inner, b";;") {
                    let from = s + 2;
                    let to = inner[from..]
                        .iter()
                        .position(|&b| b == b';')
                        .map(|k| from + k)
                        .unwrap_or(inner.len());
                    let title = &inner[from..to];
                    if !title.is_empty() {
                        if !out.is_empty() && out.last() != Some(&b'\n') {
                            out.push(b'\n');
                        }
                        out.extend_from_slice(title);
                        if out.last() != Some(&b'\n') {
                            out.push(b'\n');
                        }
                    }
                }
                i = close + 1;
                changed = true;
                continue;
            }
        }
        out.push(d[i]);
        i += 1;
    }
    if changed {
        *data = out;
    }
}

/// Find an exact byte subsequence.
fn find_seq(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    (0..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
}

/// Expand one document chunk (all text records joined).
pub fn expand_chunk(mode: u8, chunk: &[u8], words: &Words) -> Result<Vec<u8>> {
    let recs = match mode {
        1 => recs_mode_b(chunk),
        0 => recs_mode_a(chunk),
        m => bail!("chunk mode {m} not implemented"),
    };
    let mut out = Vec::new();
    for r in recs.iter().filter(|r| r.text && r.len > 0) {
        let seg = if r.walk {
            // subtype/type 1-2: RichView item stream (dispatcher walk)
            walk_record(chunk, r.start, r.start + r.len, words)
        } else {
            // subtype/type 0: plain token run
            expand_tokens(&chunk[r.start..r.start + r.len], words)
        };
        if seg.is_empty() {
            continue;
        }
        if !out.is_empty() && out.last() != Some(&b'\n') {
            out.push(b'\n');
        }
        out.extend_from_slice(&seg);
    }
    Ok(out)
}

// ============================================================
// base: docloc + words + container cache
// ============================================================

pub struct Base {
    words: Words,
    docloc: Vec<u8>,
    docs: u32,
    per_file: HashMap<u16, Vec<(u16, u32)>>, // file -> (chunk, docnum), sorted by chunk
    cache: Mutex<FileCache>,
}

struct FileCache {
    map: HashMap<u16, Arc<Vec<u8>>>,
    order: VecDeque<u16>,
}

impl Base {
    pub fn load(baza: &Path) -> Result<Self> {
        let words = Words::load(baza)?;
        let docloc = std::fs::read(baza.join("docloc.db")).context("read docloc.db")?;
        if docloc.len() < 12 {
            bail!("docloc.db too small");
        }
        let docs = (docloc.len() - 12) / 5;
        let mut per_file: HashMap<u16, Vec<(u16, u32)>> = HashMap::new();
        for d in 0..docs as u32 {
            let o = 12 + d as usize * 5;
            let file = ((docloc[o + 1] as u16) << 8) | docloc[o + 2] as u16;
            let chunk = ((docloc[o + 3] as u16) << 8) | docloc[o + 4] as u16;
            per_file.entry(file).or_default().push((chunk, d));
        }
        for v in per_file.values_mut() {
            v.sort_unstable();
        }
        Ok(Self {
            words,
            docloc,
            docs: docs as u32,
            per_file,
            cache: Mutex::new(FileCache { map: HashMap::new(), order: VecDeque::new() }),
        })
    }

    pub fn doc_count(&self) -> u32 {
        self.docs
    }

    pub fn words(&self) -> &Words {
        &self.words
    }

    /// (mode, chunk, file) for a docnum. docloc record = [mode u8][file BE16][chunk BE16].
    pub fn span(&self, docnum: u32) -> Result<(u8, u16, u16)> {
        let o = 12 + docnum as usize * 5;
        if o + 5 > self.docloc.len() {
            bail!("docnum {docnum} out of range");
        }
        Ok((
            self.docloc[o],
            ((self.docloc[o + 3] as u16) << 8) | self.docloc[o + 4] as u16,
            ((self.docloc[o + 1] as u16) << 8) | self.docloc[o + 2] as u16,
        ))
    }

    /// Docs within one container, ordered by chunk.
    pub fn file_docs(&self, file: u16) -> &[(u16, u32)] {
        match self.per_file.get(&file) {
            Some(v) => v.as_slice(),
            None => &[],
        }
    }

    pub fn file_ids(&self) -> Vec<u16> {
        let mut v: Vec<u16> = self.per_file.keys().copied().collect();
        v.sort_unstable();
        v
    }

    /// Load a container with a small LRU cache (works from worker threads).
    pub fn container(&self, baza: &Path, file: u16) -> Result<Arc<Vec<u8>>> {
        let mut cache = self.cache.lock().unwrap();
        if let Some(d) = cache.map.get(&file) {
            return Ok(d.clone());
        }
        let path = baza.join("text").join(format!("{file}.mb"));
        let data = Arc::new(
            std::fs::read(&path).with_context(|| format!("read {}", path.display()))?,
        );
        if cache.order.len() >= 16 {
            if let Some(old) = cache.order.pop_front() {
                cache.map.remove(&old);
            }
        }
        cache.order.push_back(file);
        cache.map.insert(file, data.clone());
        Ok(data)
    }

    /// Raw expanded cp1251 bytes of one document.
    pub fn doc_text_bytes(&self, baza: &Path, docnum: u32) -> Result<Vec<u8>> {
        let (mode, chunk, file) = self.span(docnum)?;
        let data = self.container(baza, file)?;
        let cb = chunk_slice(&data, chunk as u32)?;
        let mut raw = expand_chunk(mode, cb, &self.words)?;
        strip_rv_markers(&mut raw);
        Ok(raw)
    }

    /// Clean UTF-8 text of one document. Keeps the source line structure:
    /// leading indentation (requisites centering, list steps) is preserved,
    /// trailing whitespace dropped and blank-line runs collapsed.
    pub fn doc_text(&self, baza: &Path, docnum: u32) -> Result<String> {
        let raw = self.doc_text_bytes(baza, docnum)?;
        Ok(crate::db::tidy_expanded_pretty(&raw))
    }

    /// Raw bytes of the document's whole `text\{file}.mb` chunk (all of its
    /// records, embedded annexes included). Used by the annex extractor.
    pub fn chunk(&self, baza: &Path, docnum: u32) -> Result<Vec<u8>> {
        let (_mode, chunk, file) = self.span(docnum)?;
        let data = self.container(baza, file)?;
        Ok(chunk_slice(&data, chunk as u32)?.to_vec())
    }
}

// ============================================================
// tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_table_matches_rip() {
        let p = Path::new("C:\\temp\\opencode\\rosetta\\cls256.bin");
        if !p.exists() {
            return;
        }
        let b = std::fs::read(p).unwrap();
        assert_eq!(b.len(), 256);
        assert_eq!(&b[..], &CLASS_TABLE[..]);
    }

    #[test]
    fn doc136974_smoke() {
        // The known canonical doc: Постанова КМУ N 145 (was mislabeled "doc 222"
        // before the docloc word order was fixed — its bytes live at 223.mb#1).
        let baza = Path::new("C:\\Zakon.Local\\Baza");
        if !baza.exists() {
            return;
        }
        let base = Base::load(baza).unwrap();
        let text = base.doc_text(baza, 136974).unwrap();
        assert!(text.contains("КАБІНЕТ"), "136974 header missing: {text:?}");
        assert!(text.contains("МІНІСТРІВ"), "136974 header 2 missing");
        assert!(text.contains("П О С Т А Н О В А"), "kicker missing");
        // varint terminator >= 0x80: `02 81` = id 257 "л", `02 99` = id 281 "я"
        assert!(
            text.contains("п о с т а н о в л я є"),
            "operative formula broken (varint terminator rule): {text:?}"
        );
    }

    #[test]
    fn doc222_smoke() {
        // doc 222 = a different real document (закон-проект). Sanity only.
        let baza = Path::new("C:\\Zakon.Local\\Baza");
        if !baza.exists() {
            return;
        }
        let base = Base::load(baza).unwrap();
        let text = base.doc_text(baza, 222).unwrap();
        // doc 222 = Постанова ВРУ "Про проект Закону... про ліміт державних
        // грошових зобов'язань ... на 1998 рік" (file 1.mb chunk 223)
        assert!(text.contains("ВЕРХОВНОЇ РАДИ УКРАЇНИ"), "doc 222: {text:?}");
        assert!(text.contains("Закону"), "doc 222: {text:?}");
    }

    #[test]
    fn strip_rv_markers_cases() {
        // section marker: title between ";;" and the next ';'
        let mut d = b"~{1;;\xD0\xEE\xE7\xEC\xB3\xF0;b<0>}".to_vec();
        strip_rv_markers(&mut d);
        assert_eq!(d, b"\xD0\xEE\xE7\xEC\xB3\xF0\n");
        // plain marker without ";;" is dropped
        let mut d = b"ok~{0}\nend".to_vec();
        strip_rv_markers(&mut d);
        assert_eq!(d, b"ok\nend");
        // unterminated marker is kept as-is
        let mut d = b"~{1;;trunc".to_vec();
        strip_rv_markers(&mut d);
        assert_eq!(d, b"~{1;;trunc");
    }

    #[test]
    fn gk_walk_smoke() {
        // Docs written by the NEW base writer (Baza.GK): subtype-1 records are
        // RichView item streams; the dispatcher walk must yield clean text.
        let baza = Path::new("C:\\Zakon.Local\\Baza.GK");
        if !baza.exists() {
            return;
        }
        let base = Base::load(baza).unwrap();
        let text = base.doc_text(baza, 9599).unwrap();
        assert!(
            text.contains("ПРОТОКОЛ № 1"),
            "9599 head missing (item walk broken): {text:?}"
        );
        assert!(text.contains("присутні"), "9599 body missing");
        assert!(!text.contains("\u{FFFD}"), "9599: replacement chars");
        let text2 = base.doc_text(baza, 9572).unwrap();
        assert!(text2.contains("НАКАЗУЮ"), "9572 missing: {text2:?}");
        assert!(text2.contains("Хмарне сховище"), "9572 body missing");
        assert!(!text2.contains("~{"), "9572: markers not stripped");
    }

    #[test]
    fn old_typ2_walk_smoke() {
        // OLD mode-A file whose type-2 records are item streams: doc 743
        // (Постанова КМУ N 1436 від 14.09.1998).
        let baza = Path::new("C:\\Zakon.Local\\Baza");
        if !baza.exists() {
            return;
        }
        let base = Base::load(baza).unwrap();
        let text = base.doc_text(baza, 743).unwrap();
        assert!(text.contains("екранного часу"), "743: {text:?}");
        assert!(text.contains("ПУСТОВОЙТЕНКО"), "743 signature missing");
    }
}
