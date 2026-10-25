//! Minimal pure-Rust PDF writer: embeds a SUBSET of a system TrueType font
//! (default C:\Windows\Fonts\arial.ttf) and renders document text to A4
//! pages. No external renderer: hand-rolled PDF 1.4 (TrueType /FontFile2 +
//! custom /Encoding Differences + /Widths from hmtx), font stream compressed
//! with flate2 (rust backend, no C).
//!
//! TTF subset: the used characters' glyphs (+ composite-component closure,
//! + .notdef) are copied into a new font file with a full-size 0..n glyph
//! array (original glyph ids are kept, so composite references need no
//! rewriting), a format-4 cmap over the used characters, and a post 2.0
//! table with synthetic glyph names ("g<id>") that the PDF /Differences
//! array references.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

/// The default font for PDF export (Arial ships with every Windows).
pub fn default_font_path() -> PathBuf {
    Path::new(r"C:\Windows\Fonts\arial.ttf").to_path_buf()
}

// ---------------------------------------------------------------------------
// big-endian readers
// ---------------------------------------------------------------------------

fn be_u16(b: &[u8], i: usize) -> u16 {
    u16::from_be_bytes([b[i], b[i + 1]])
}
fn be_u32(b: &[u8], i: usize) -> u32 {
    u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}
fn be_i16(b: &[u8], i: usize) -> i16 {
    be_u16(b, i) as i16
}
const fn tag_of(s: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*s)
}
const T_HEAD: u32 = tag_of(b"head");
const T_HHEA: u32 = tag_of(b"hhea");
const T_MAXP: u32 = tag_of(b"maxp");
const T_HMTX: u32 = tag_of(b"hmtx");
const T_LOCA: u32 = tag_of(b"loca");
const T_GLYF: u32 = tag_of(b"glyf");
const T_CMAP: u32 = tag_of(b"cmap");
const T_POST: u32 = tag_of(b"post");
const T_NAME: u32 = tag_of(b"name");
const T_OS2: u32 = tag_of(b"OS/2");

// ---------------------------------------------------------------------------
// cmap (format 4 / format 12 lookup)
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum CMap {
    F4 {
        /// raw format-4 subtable bytes (range offsets are dereferenced in it)
        sub: Vec<u8>,
        seg_count: u16,
        end_code: Vec<u16>,
        start_code: Vec<u16>,
        id_delta: Vec<i16>,
        id_range_off: Vec<u16>,
        /// the font's numGlyphs (exclusive bound; stale cmap mappings
        /// pointing past it are clamped to .notdef)
        num_glyphs: u16,
    },
    F12 {
        starts: Vec<u32>,
        ends: Vec<u32>,
        gids: Vec<u32>,
        num_glyphs: u16,
    },
}

impl CMap {
    /// codepoint -> glyph id (0 = .notdef). A mapping that points past the
    /// font's own glyph table (stale/quirky cmap data) is clamped to 0.
    fn lookup(&self, cp: u32) -> u16 {
        match self {
            CMap::F12 {
                starts,
                ends,
                gids,
                num_glyphs,
            } => {
                // binary search: last range with start <= cp
                let mut lo = 0usize;
                let mut hi = starts.len();
                while lo + 1 < hi {
                    let mid = (lo + hi) / 2;
                    if starts[mid] <= cp {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                if starts[lo] <= cp && cp <= ends[lo] {
                    let g = gids[lo] as u16;
                    if g >= *num_glyphs { 0 } else { g }
                } else {
                    0
                }
            }
            CMap::F4 {
                sub,
                seg_count,
                end_code,
                start_code,
                id_delta,
                id_range_off,
                num_glyphs,
            } => {
                let c = cp as u16;
                if cp > 0xFFFF {
                    return 0;
                }
                // lower bound: first segment with endCode >= c
                let mut lo = 0usize;
                let mut hi = *seg_count as usize;
                while lo < hi {
                    let mid = (lo + hi) / 2;
                    if end_code[mid] >= c {
                        hi = mid;
                    } else {
                        lo = mid + 1;
                    }
                }
                let seg = lo;
                if seg >= *seg_count as usize || c < start_code[seg] {
                    return 0; // past the last segment, or a gap between segments
                }
                let delta = id_delta[seg] as i32;
                let roff = id_range_off[seg];
                if roff == 0 {
                    let g = (c as i32 - start_code[seg] as i32 + delta) as u16 & 0xFFFF;
                    return if g >= *num_glyphs { 0 } else { g };
                }
                // per the spec: the glyph index is read at
                // (address of this idRangeOffset value) + roff + 2; the
                // idRangeOffset array starts at 16 + 6*segCount in the subtable
                let entry_addr = 16 + 6 * (*seg_count as usize) + 2 * seg;
                let g = be_u16(&sub[entry_addr + roff as usize + 2..], 0) as i32;
                if g == 0 {
                    0
                } else {
                    let r = (g + delta) as u16 & 0xFFFF;
                    if r >= *num_glyphs {
                        0
                    } else {
                        r
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// parsed font
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct TtfFont {
    data: Vec<u8>,
    /// (tag, offset, length)
    tables: Vec<(u32, usize, usize)>,
    pub num_glyphs: u16,
    pub units_per_em: u16,
    /// 32-bit glyph offsets (loca16 values are scaled by 1)
    pub loca: Vec<u32>,
    pub glyf_off: usize,
    pub glyf_len: usize,
    pub cmap: CMap,
    /// (advanceWidth, lsb) per original glyph index
    pub hmtx: Vec<(u16, i16)>,
    pub ascent: i16,
    pub descent: i16,
    pub italic_angle: f32,
    /// the original tables we copy verbatim
    head: (usize, usize),
    hhea: (usize, usize),
    maxp: (usize, usize),
    name: Option<(usize, usize)>,
    os2: Option<(usize, usize)>,
}

fn table<'a>(f: &'a TtfFont, tag: u32) -> Option<&'a [u8]> {
    f.tables
        .iter()
        .find(|(t, _, _)| *t == tag)
        .map(|(_, o, l)| &f.data[*o..*o + *l])
}

/// (offset, length) of a table, or None
fn table_range(f: &TtfFont, tag: u32) -> Option<(usize, usize)> {
    f.tables
        .iter()
        .find(|(t, _, _)| *t == tag)
        .map(|(_, o, l)| (*o, *l))
}

/// Parse a TrueType font (sfnt, not OTF/TTCollection — checked by magic).
pub fn parse(data: &[u8]) -> Result<TtfFont> {
    if data.len() < 12 {
        bail!("font file too small");
    }
    let ver = be_u32(data, 0);
    // 0x00010000 / 0x00020000 (TrueType) or 'true' (0x74727565)
    if ver != 0x0001_0000 && ver != 0x0002_0000 && ver != 0x7472_7565 {
        bail!(
            "not a TrueType font (magic 0x{ver:08x}; OTF/CFF and TTC are not supported)"
        );
    }
    let n = be_u16(data, 4) as usize;
    let mut tables = Vec::with_capacity(n);
    for i in 0..n {
        let o = 12 + 16 * i;
        if o + 16 > data.len() {
            bail!("truncated table directory");
        }
        let tag = be_u32(data, o);
        let off = be_u32(data, o + 8) as usize;
        let len = be_u32(data, o + 12) as usize;
        if off + len > data.len() {
            bail!("table {tag:08x} out of bounds");
        }
        tables.push((tag, off, len));
    }
    let mut font = TtfFont {
        data: data.to_vec(),
        tables,
        num_glyphs: 0,
        units_per_em: 0,
        loca: Vec::new(),
        glyf_off: 0,
        glyf_len: 0,
        cmap: CMap::F12 {
            starts: Vec::new(),
            ends: Vec::new(),
            gids: Vec::new(),
            num_glyphs: 0,
        },
        hmtx: Vec::new(),
        ascent: 0,
        descent: 0,
        italic_angle: 0.0,
        head: (0, 0),
        hhea: (0, 0),
        maxp: (0, 0),
        name: None,
        os2: None,
    };
    let (head_off, head_len) =
        table_range(&font, T_HEAD).context("missing head table")?;
    font.units_per_em = be_u16(&font.data[head_off + 14..], 0);
    // indexToLocFormat is the second-to-last field of head
    let loca32 = be_i16(&font.data[head_off + head_len - 4..], 0) == 1;
    font.head = (head_off, head_len);

    let (maxp_off, maxp_len) = table_range(&font, T_MAXP).context("missing maxp table")?;
    font.num_glyphs = be_u16(&font.data[maxp_off + 4..], 0);
    font.maxp = (maxp_off, maxp_len);

    let (hhea_off, hhea_len) = table_range(&font, T_HHEA).context("missing hhea table")?;
    font.ascent = be_i16(&font.data[hhea_off + 4..], 0);
    font.descent = be_i16(&font.data[hhea_off + 6..], 0);
    font.hhea = (hhea_off, hhea_len);

    let (hmtx_off, hmtx_len) = table_range(&font, T_HMTX).context("missing hmtx table")?;
    let n_entries = (hmtx_len / 4).min(font.num_glyphs as usize);
    for i in 0..font.num_glyphs as usize {
        let (adv, lsb) = if i < n_entries {
            let o = hmtx_off + 4 * i;
            (
                be_u16(&font.data[o..], 0),
                be_i16(&font.data[o + 2..], 0),
            )
        } else {
            // the spec: glyphs beyond the last entry use the last one
            let o = hmtx_off + 4 * (n_entries - 1);
            (
                be_u16(&font.data[o..], 0),
                be_i16(&font.data[o + 2..], 0),
            )
        };
        font.hmtx.push((adv, lsb));
    }

    let (glyf_off, glyf_len) = table_range(&font, T_GLYF).context("missing glyf table")?;
    font.glyf_off = glyf_off;
    font.glyf_len = glyf_len;
    let loca = table(&font, T_LOCA).context("missing loca table")?;
    font.loca = if loca32 {
        (0..=font.num_glyphs as usize)
            .map(|i| be_u32(&loca[4 * i..], 0))
            .collect()
    } else {
        (0..=font.num_glyphs as usize)
            .map(|i| be_u16(&loca[2 * i..], 0) as u32)
            .collect()
    };

    // post (italic angle; glyph names are rewritten anyway). Layout:
    // version(4), italicAngle F16.16(4), underlinePosition(2),
    // underlineThickness(2) [v3.0 stops here, v2.0 adds glyph names]
    if let Some(post) = table(&font, T_POST) {
        if post.len() >= 8 {
            font.italic_angle = be_u32(post, 4) as f32 / 65536.0;
        }
    }
    font.name = table_range(&font, T_NAME);
    font.os2 = table_range(&font, T_OS2);

    // cmap: prefer the Windows Unicode subtable (3,1), else (0,3)
    let cmap_tbl = table(&font, T_CMAP).context("missing cmap table")?;
    let n_sub = be_u16(cmap_tbl, 2) as usize;
    let mut found: Option<(usize, u16)> = None; // (subtable offset inside the table, format)
    for i in 0..n_sub {
        let o = 4 + 8 * i;
        let plat = be_u16(cmap_tbl, o);
        let enc = be_u16(cmap_tbl, o + 2);
        let sub_off = be_u32(cmap_tbl, o + 4) as usize;
        if (plat, enc) == (3, 1) || (plat, enc) == (0, 3) {
            let fmt = be_u16(&cmap_tbl[sub_off..], 0);
            // (3,1) beats (0,3); format 4 beats format 12 within the same
            if fmt == 4 || found.is_none() || fmt < found.unwrap().1 {
                found = Some((sub_off, fmt));
            }
            if plat == 3 && enc == 1 {
                break;
            }
        }
    }
    let (sub_off, _) = found.context("no usable cmap subtable (need platform 3/1 or 0/3)")?;
    let sub = &cmap_tbl[sub_off..];
    let fmt = be_u16(sub, 0);
    font.cmap = if fmt == 4 {
        let seg_count = be_u16(sub, 6) as usize / 2; // segCountX2
        let mut end_code = Vec::with_capacity(seg_count);
        for i in 0..seg_count {
            end_code.push(be_u16(sub, 14 + 2 * i), );
        }
        let start_off = 14 + 2 * seg_count + 2; // + reservedPad
        let mut start_code = Vec::with_capacity(seg_count);
        for i in 0..seg_count {
            start_code.push(be_u16(sub, start_off + 2 * i));
        }
        let delta_off = start_off + 2 * seg_count;
        let mut id_delta = Vec::with_capacity(seg_count);
        for i in 0..seg_count {
            id_delta.push(be_i16(sub, delta_off + 2 * i));
        }
        let r_off = delta_off + 2 * seg_count;
        let mut id_range_off = Vec::with_capacity(seg_count);
        for i in 0..seg_count {
            id_range_off.push(be_u16(sub, r_off + 2 * i));
        }
        CMap::F4 {
            sub: sub.to_vec(),
            seg_count: seg_count as u16,
            end_code,
            start_code,
            id_delta,
            id_range_off,
            num_glyphs: font.num_glyphs,
        }
    } else if fmt == 12 {
        let n_ranges = be_u32(sub, 8) as usize;
        let mut starts = Vec::with_capacity(n_ranges);
        let mut ends = Vec::with_capacity(n_ranges);
        let mut gids = Vec::with_capacity(n_ranges);
        for i in 0..n_ranges {
            starts.push(be_u32(sub, 12 + 12 * i));
            ends.push(be_u32(sub, 16 + 12 * i));
            gids.push(be_u32(sub, 20 + 12 * i));
        }
        CMap::F12 {
            starts,
            ends,
            gids,
            num_glyphs: font.num_glyphs,
        }
    } else {
        bail!("unsupported cmap format {fmt} (need 4 or 12)");
    };
    Ok(font)
}

// ---------------------------------------------------------------------------
// subsetting
// ---------------------------------------------------------------------------

/// Glyph ids referenced by a composite glyph (parsed from the original
/// glyf record; simple glyphs return none).
fn composite_refs(rec: &[u8]) -> Vec<u16> {
    if rec.len() < 12 {
        return Vec::new();
    }
    if be_i16(rec, 0) >= 0 {
        return Vec::new(); // simple glyph
    }
    let mut out = Vec::new();
    let mut p = 10usize;
    while p + 4 <= rec.len() {
        // component: glyphIndex(2), optional offset args(4), flags(2), extras
        let idx = be_u16(rec, p);
        // the flags word decides the args size but sits AFTER the args —
        // read the candidate at p+2: if its ARG bits (0x03) are set, args
        // are present and the real flags live at p+4 (the ttf-parser trick)
        let flags0 = be_u16(rec, p + 2);
        let more = if flags0 & 0x0003 != 0 {
            be_u16(rec, p + 4)
        } else {
            flags0
        };
        out.push(idx);
        let mut adv = if flags0 & 0x0003 != 0 { 8 } else { 4 };
        if more & 0x0008 != 0 {
            adv += 2; // SCALE
        }
        if more & 0x0010 != 0 {
            adv += 4; // X_AND_YSCALE
        }
        if more & 0x0020 != 0 {
            adv += 2; // XSCALE
        }
        if more & 0x0400 != 0 {
            adv += 2; // WE_HAVE_INSTRUCTIONS
        }
        if more & 0x0800 != 0 {
            adv += 2; // WE_HAVE_A_ANCHORS
        }
        if more & 0x1000 != 0 {
            adv += 2; // WE_HAVE_A_DEVICE
        }
        if more & 0x2000 != 0 {
            adv += 8; // WE_HAVE_A_BBOX
        }
        p += adv;
        if more & 0x0040 == 0 {
            break; // no MORE_COMPONENTS
        }
    }
    out
}

/// A built subset font file + everything the PDF layer needs.
#[derive(Debug)]
pub struct SubsetFont {
    /// the subset font file bytes (sfnt)
    pub font_bytes: Vec<u8>,
    pub units_per_em: u16,
    pub ascent: i16,
    pub descent: i16,
    pub italic_angle: f32,
    /// glyph advance (font units) for each gid 0..n
    pub widths: Vec<u16>,
    /// glyph names (post 2.0 + PDF /Differences) for each gid 0..n
    pub glyph_names: Vec<String>,
    /// distinct characters of the text: (codepoint, encoding byte, gid)
    pub chars: Vec<(u32, u8, u16)>,
    /// the last encoding byte actually used (>= 32)
    pub last_char: u8,
}

fn build_cmap_table(chars: &[(u32, u8, u16)]) -> Vec<u8> {
    // A format-4 segment [start..end] with idRangeOffset == 0 maps its
    // codes to CONSECUTIVE glyph ids: glyph(c) = (c - start + delta) mod
    // 65536. Our code -> glyph mapping is not contiguous (glyph ids are
    // the original font's), so each distinct code gets its OWN single-code
    // segment: start == end == code, delta = (gid - code) mod 65536.
    let mut codes: Vec<(u16, u16)> = chars
        .iter()
        .map(|(cp, _, g)| (*cp as u16, *g))
        .filter(|(code, _)| *code != 0xFFFF) // reserved for the terminator
        .collect();
    codes.sort();
    // de-duplicate (two chars can share a codepoint only via BMP folding)
    codes.dedup();
    let mut segs: Vec<(u16, u16, u16)> = Vec::new(); // (start, end, delta)
    for (code, gid) in codes {
        // glyph(c) = (c - start + delta) mod 65536; with start == end == code
        // the delta must be the glyph id itself (NOT gid - code)
        segs.push((code, code, gid));
    }
    // the spec requires the last segment's endCode to be 0xFFFF; delta 0
    // maps U+FFFF to glyph 0 (.notdef)
    segs.push((0xFFFF, 0xFFFF, 0));
    let seg_count = segs.len();
    let mut body = Vec::new();
    body.extend_from_slice(&4u16.to_be_bytes()); // format
    // the spec layout: format(2) length(2) language(2) segCountX2(2)
    // searchRange(2) entrySelector(2) rangeShift(2), then the arrays
    let sub_len = (16 + 8 * seg_count) as u16;
    body.extend_from_slice(&sub_len.to_be_bytes()); // length
    body.extend_from_slice(&0u16.to_be_bytes()); // language
    body.extend_from_slice(&(2 * seg_count as u16).to_be_bytes()); // segCountX2
    // searchRange = 2 * 2^floor(log2(segCount))
    let entry_selector = 31u32.saturating_sub((seg_count as u32).leading_zeros()) as u16;
    let pow2 = 1u32 << entry_selector;
    let search_range = (2 * pow2) as u16;
    let range_shift = (2 * seg_count as u32 - 2 * pow2) as u16;
    body.extend_from_slice(&search_range.to_be_bytes());
    body.extend_from_slice(&entry_selector.to_be_bytes());
    body.extend_from_slice(&range_shift.to_be_bytes());
    for (_, end, _) in &segs {
        body.extend_from_slice(&end.to_be_bytes()); // endCodes
    }
    body.extend_from_slice(&0u16.to_be_bytes()); // reservedPad
    for (start, _, _) in &segs {
        body.extend_from_slice(&start.to_be_bytes()); // startCodes
    }
    for (_, _, delta) in &segs {
        body.extend_from_slice(&delta.to_be_bytes()); // idDelta
    }
    for _ in 0..seg_count {
        body.extend_from_slice(&0u16.to_be_bytes()); // idRangeOffset (all 0)
    }
    // two subtables (0,3) and (3,1) sharing the same format-4 body;
    // the body follows the 4-byte header + two 8-byte records
    let body_off = 20u32;
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_be_bytes()); // version
    out.extend_from_slice(&2u16.to_be_bytes()); // nSubtables
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&3u16.to_be_bytes());
    out.extend_from_slice(&body_off.to_be_bytes());
    out.extend_from_slice(&3u16.to_be_bytes());
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&body_off.to_be_bytes());
    out.extend_from_slice(&body);
    out
}

fn build_post_table(font: &TtfFont, glyph_names: &[String]) -> Vec<u8> {
    // post format 2.0: every glyph gets an explicit (synthetic) name so the
    // PDF /Differences array can reference them
    let angle_bits = (font.italic_angle * 65536.0) as u32;
    // name strings: NUL-terminated, each padded to an even length; offsets
    // are relative to the start of the name array
    let mut offs: Vec<u16> = Vec::with_capacity(glyph_names.len());
    let mut pos = 0usize;
    for name in glyph_names {
        offs.push(pos as u16);
        pos += name.len() + 1;
        if pos % 2 == 1 {
            pos += 1;
        }
    }
    let mut out = Vec::new();
    out.extend_from_slice(&0x0002_0000u32.to_be_bytes()); // version
    out.extend_from_slice(&angle_bits.to_be_bytes()); // italicAngle F16.16
    out.extend_from_slice(&0i16.to_be_bytes()); // underlinePosition
    out.extend_from_slice(&0i16.to_be_bytes()); // underlineThickness
    out.extend_from_slice(&0u32.to_be_bytes()); // isFixed
    for _ in 0..8 {
        out.extend_from_slice(&0i16.to_be_bytes()); // min/max mem
    }
    out.extend_from_slice(&(glyph_names.len() as u16).to_be_bytes()); // numStrings
    for o in &offs {
        out.extend_from_slice(&o.to_be_bytes());
    }
    for name in glyph_names {
        out.extend_from_slice(name.as_bytes());
        out.push(0);
        if (name.len() + 1) % 2 == 1 {
            out.push(0); // pad to even
        }
    }
    out
}

fn build_font_file(
    font: &TtfFont,
    keep: &[bool],
    n: usize,
    chars: &[(u32, u8, u16)],
    glyph_names: &[String],
) -> Result<Vec<u8>> {
    // glyf: full-size 0..n arrays, unkept glyphs are zero-length
    let mut glyf: Vec<u8> = Vec::new();
    let mut loca: Vec<u32> = Vec::with_capacity(n + 1);
    for g in 0..n {
        loca.push(glyf.len() as u32);
        if g < font.num_glyphs as usize && keep[g] {
            let off = font.loca[g];
            let off2 = font.loca[g + 1];
            glyf.extend_from_slice(
                &font.data[font.glyf_off + off as usize..font.glyf_off + off2 as usize],
            );
        }
    }
    loca.push(glyf.len() as u32);

    let cmap_tbl = build_cmap_table(chars);
    let post_tbl = build_post_table(font, glyph_names);

    // head: indexToLocFormat = 1 (32-bit loca); pad a 44-byte head to 50
    let (head_off, head_len) = font.head;
    let mut head = font.data[head_off..head_off + head_len].to_vec();
    if head.len() == 44 {
        head.extend_from_slice(&[0u8; 4]);
    }
    let hlen = head.len();
    head[hlen - 4] = 0; // indexToLocFormat = 1 (32-bit loca)
    head[hlen - 3] = 1;

    // maxp: numGlyphs = n (offset 4)
    let (maxp_off, maxp_len) = font.maxp;
    let mut maxp = font.data[maxp_off..maxp_off + maxp_len].to_vec();
    maxp[4] = (n as u32 >> 8) as u8;
    maxp[5] = (n as u32 & 0xFF) as u8;

    // hhea: verbatim
    let (hhea_off, hhea_len) = font.hhea;
    let hhea = font.data[hhea_off..hhea_off + hhea_len].to_vec();

    // hmtx: n entries (font.hmtx is pre-padded to num_glyphs in parse)
    let mut hmtx = Vec::with_capacity(4 * n);
    for g in 0..n {
        let (adv, lsb) = if g < font.num_glyphs as usize {
            font.hmtx[g]
        } else {
            (0, 0)
        };
        hmtx.extend_from_slice(&adv.to_be_bytes());
        hmtx.extend_from_slice(&lsb.to_be_bytes());
    }

    let mut loca_bytes = Vec::with_capacity(4 * (n + 1));
    for l in &loca {
        loca_bytes.extend_from_slice(&l.to_be_bytes());
    }

    let mut tables: Vec<(u32, Vec<u8>)> = Vec::new();
    tables.push((T_HEAD, head));
    tables.push((T_HHEA, hhea));
    tables.push((T_MAXP, maxp));
    tables.push((T_HMTX, hmtx));
    tables.push((T_LOCA, loca_bytes));
    tables.push((T_GLYF, glyf));
    tables.push((T_CMAP, cmap_tbl));
    tables.push((T_POST, post_tbl));
    if let Some((o, l)) = &font.name {
        tables.push((T_NAME, font.data[*o..*o + *l].to_vec()));
    }
    if let Some((o, l)) = &font.os2 {
        tables.push((T_OS2, font.data[*o..*o + *l].to_vec()));
    }
    tables.sort_by_key(|(t, _)| *t);

    // sfnt header: version, table count, search params, then the table
    // directory (16 bytes per table), then the 2-byte-aligned table data
    // (per the spec: searchRange = 16 * 2^floor(log2(n)), rangeShift = n*16 - searchRange)
    let n_t = tables.len() as u16;
    let entry_selector = 31u32.saturating_sub((n_t as u32).leading_zeros()) as u16;
    let pow2 = 1u32 << entry_selector;
    let search_range = (pow2 * 16) as u16;
    let range_shift = (n_t as u32 * 16 - pow2 * 16) as u16;
    // the directory sits between the 12-byte header and the table data
    let data_start = (12 + 16 * n_t as usize) as u32;
    let lens: Vec<u32> = tables.iter().map(|(_, d)| d.len() as u32).collect();
    let mut offsets: Vec<u32> = Vec::with_capacity(n_t as usize);
    let mut pos = data_start;
    for i in 0..n_t as usize {
        offsets.push(pos);
        pos += lens[i];
        if lens[i] % 2 == 1 {
            pos += 1; // 2-byte alignment pad
        }
    }
    let mut out = Vec::new();
    out.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    out.extend_from_slice(&n_t.to_be_bytes());
    out.extend_from_slice(&search_range.to_be_bytes());
    out.extend_from_slice(&entry_selector.to_be_bytes());
    out.extend_from_slice(&range_shift.to_be_bytes());
    for i in 0..n_t as usize {
        let tag = tables[i].0;
        out.extend_from_slice(&tag.to_be_bytes());
        out.extend_from_slice(&0u32.to_be_bytes()); // checksum (not verified)
        out.extend_from_slice(&offsets[i].to_be_bytes());
        out.extend_from_slice(&lens[i].to_be_bytes());
    }
    for (_, data) in &tables {
        if out.len() % 2 == 1 {
            out.push(0); // 2-byte alignment
        }
        out.extend_from_slice(data);
    }
    Ok(out)
}

/// Build the subset from the characters used in `text`.
pub fn subset_font(font: &TtfFont, text: &str) -> Result<SubsetFont> {
    // 1) distinct characters (BMP only; >BMP / controls → space)
    let mut seen: Vec<u32> = Vec::new();
    for ch in text.chars() {
        let cp = ch as u32;
        if cp < 32 || cp > 0xFFFF {
            continue;
        }
        if !seen.contains(&cp) {
            seen.push(cp);
        }
    }
    // encoding: byte 32 = space, the rest in first-use order → 33..255
    // (at most 223 non-space characters fit into one byte)
    let n_non_space = seen.iter().filter(|c| **c != 32).count();
    if n_non_space > 223 {
        bail!("more than 223 distinct non-space characters in one document (1-byte PDF encoding limit)");
    }
    let mut chars: Vec<(u32, u8, u16)> = Vec::new();
    let mut byte = 33u8;
    for &cp in &seen {
        if cp == 32 {
            chars.push((32, 32, font.cmap.lookup(32)));
            continue;
        }
        chars.push((cp, byte, font.cmap.lookup(cp)));
        byte = byte.wrapping_add(1); // safe: at most 223 non-space chars
    }
    if !chars.iter().any(|c| c.1 == 32) {
        chars.push((32, 32, font.cmap.lookup(32)));
    }
    let last_char = chars.iter().map(|c| c.1).max().unwrap_or(32);

    // 2) glyph closure: .notdef + the used gids + composite references
    let n_orig = font.num_glyphs as usize;
    let mut keep: Vec<bool> = vec![false; n_orig];
    let mut queue: Vec<u16> = Vec::new();
    for g in chars.iter().map(|c| c.2).chain(std::iter::once(0u16)) {
        if g < n_orig as u16 && !keep[g as usize] {
            keep[g as usize] = true;
            queue.push(g);
        }
    }
    while let Some(g) = queue.pop() {
        let off = font.loca[g as usize];
        let off2 = font.loca[g as usize + 1];
        let rec = &font.data[font.glyf_off + off as usize..font.glyf_off + off2 as usize];
        for r in composite_refs(rec) {
            if r < n_orig as u16 && !keep[r as usize] {
                keep[r as usize] = true;
                queue.push(r);
            }
        }
    }
    // the subset file's arrays span 0..=max kept gid (unkept → empty glyph)
    let n = keep.iter().rposition(|k| *k).map(|i| i + 1).unwrap_or(1);

    // 3) widths + names for every gid 0..n
    let widths: Vec<u16> = (0..n)
        .map(|g| {
            if g < font.num_glyphs as usize {
                font.hmtx[g].0
            } else {
                0
            }
        })
        .collect();
    let space_gid = chars
        .iter()
        .find(|c| c.1 == 32)
        .map(|c| c.2)
        .unwrap_or(0);
    let glyph_names: Vec<String> = (0..n)
        .map(|g| {
            if g == 0 {
                String::from(".notdef")
            } else if g as u16 == space_gid {
                String::from("space")
            } else {
                format!("g{g}")
            }
        })
        .collect();

    // 4) the subset font file
    let font_bytes = build_font_file(font, &keep, n, &chars, &glyph_names)?;
    Ok(SubsetFont {
        font_bytes,
        units_per_em: font.units_per_em,
        ascent: font.ascent,
        descent: font.descent,
        italic_angle: font.italic_angle,
        widths,
        glyph_names,
        chars,
        last_char,
    })
}

// ---------------------------------------------------------------------------
// PDF assembly
// ---------------------------------------------------------------------------

/// A4 page, 12 pt, 15 pt leading, 0.85" side margins / 1" top & bottom.
const FONT_SIZE: f64 = 12.0;
const LEADING: f64 = 15.0;
const PAGE_W: f64 = 595.28;
const PAGE_H: f64 = 841.89;
const MARGIN_X: f64 = 61.2;
const MARGIN_Y: f64 = 72.0;

/// Format a PDF number cleanly (no `:g` in Rust): up to 2 decimals, trim
/// trailing zeros. 12 -> "12", 595.28 -> "595.28", 61.2 -> "61.2".
fn fnum(x: f64) -> String {
    let s = format!("{:.2}", x);
    let s = s.trim_end_matches('0');
    let s = s.trim_end_matches('.');
    s.to_string()
}

/// Escape a PDF literal string body (bytes 32..255; only ( ) \ need quotes)
fn pdf_string(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + 4);
    for &b in bytes {
        match b {
            0x5C => out.extend_from_slice(br"\"),
            0x28 => out.extend_from_slice(br"\("),
            0x29 => out.extend_from_slice(br"\)"),
            _ => out.push(b),
        }
    }
    out
}

/// Wrap + paginate the lines into PDF content streams (one per page),
/// using the subset encoding (char → byte) and hmtx widths.
fn content_streams(sub: &SubsetFont, lines: &[&str]) -> Vec<Vec<u8>> {
    // char → encoding byte (space by default)
    let mut byte_of: [u8; 65536] = [32; 65536];
    for &(cp, b, _) in &sub.chars {
        byte_of[cp as usize] = b;
    }
    // byte → glyph id (for widths)
    let mut gid_of_byte: [u16; 256] = [0; 256];
    for &(_, b, g) in &sub.chars {
        gid_of_byte[b as usize] = g;
    }
    let pt_per_unit = FONT_SIZE / sub.units_per_em as f64;
    let avail = PAGE_W - 2.0 * MARGIN_X;
    let space_w = sub.widths[gid_of_byte[32] as usize] as f64 * pt_per_unit;

    let mut pager = Pager::new();
    for line in lines {
        let mut seg: Vec<u8> = Vec::new(); // encoded bytes of the current line piece
        let mut cur_w = 0.0f64;
        for w in line.split(' ') {
            if w.is_empty() {
                continue;
            }
            let w_units: f64 = w
                .chars()
                .map(|c| {
                    let cp = c as u32;
                    let b = if cp < 65536 {
                        byte_of[cp as usize]
                    } else {
                        32
                    };
                    sub.widths[gid_of_byte[b as usize] as usize] as f64
                })
                .sum();
            let add = w_units * pt_per_unit;
            let sep = if seg.is_empty() { 0.0 } else { space_w };
            if cur_w > 0.0 && cur_w + sep + add > avail {
                if !seg.is_empty() {
                    seg.push(32); // the wrap point
                }
                pager.emit(&seg);
                seg.clear();
                cur_w = 0.0;
            }
            for c in w.chars() {
                let cp = c as u32;
                seg.push(if cp < 65536 { byte_of[cp as usize] } else { 32 });
            }
            cur_w += if cur_w == 0.0 { add } else { add + sep };
        }
        if !seg.is_empty() || line.is_empty() {
            pager.emit(&seg);
        }
    }
    pager.finish()
}

/// Page state: opens a content stream (BT/Td) at the top of a new page and
/// closes it (ET) when the bottom margin is hit.
struct Pager {
    pages: Vec<Vec<u8>>,
    page: Vec<u8>,
    in_page: bool,
    y: f64,
}

impl Pager {
    fn new() -> Self {
        Self {
            pages: Vec::new(),
            page: Vec::new(),
            in_page: false,
            y: PAGE_H - MARGIN_Y,
        }
    }
    fn emit(&mut self, seg: &[u8]) {
        if self.y < MARGIN_Y {
            self.end_page();
        }
        if !self.in_page {
            let fs_ = fnum(FONT_SIZE);
            let ls_ = fnum(LEADING);
            let mx_ = fnum(MARGIN_X);
            self.page.extend_from_slice(
                format!("BT /F1 {fs_} Tf {ls_} TL {mx_} {} Td\n", fnum(self.y)).as_bytes(),
            );
            self.in_page = true;
        }
        self.page.extend_from_slice(b"(");
        self.page.extend_from_slice(&pdf_string(seg));
        self.page.extend_from_slice(b") Tj T*\n");
        self.y -= LEADING;
    }
    fn end_page(&mut self) {
        if self.in_page {
            self.page.extend_from_slice(b"ET\n");
            self.pages.push(std::mem::take(&mut self.page));
            self.in_page = false;
        }
    }
    fn finish(mut self) -> Vec<Vec<u8>> {
        self.end_page();
        self.pages
    }
}

/// Write one document to `path` as a minimal PDF (A4, embedded subset of
/// `font_path`, the default being C:\Windows\Fonts\arial.ttf).
pub fn write_pdf_with_font(path: &Path, title: &str, lines: &[&str], font_path: &Path) -> Result<()> {
    let raw = std::fs::read(font_path)
        .with_context(|| format!("read font {}", font_path.display()))?;
    let font = parse(&raw)?;
    let text: String = lines.join("\n");
    let sub = subset_font(&font, &text)?;

    let streams0 = content_streams(&sub, lines);
    let mut streams = streams0;
    if streams.is_empty() {
        streams.push(Vec::new()); // at least one (empty) page
    }

    // ---- font objects ----------------------------------------------------
    // /Encoding Differences: byte → glyph name (the subset post-table names)
    let mut diff: Vec<String> = Vec::new();
    for b in 32..=sub.last_char {
        let g = sub
            .chars
            .iter()
            .find(|c| c.1 == b)
            .map(|c| c.2)
            .unwrap_or(0);
        diff.push(format!("{b} /{}", sub.glyph_names[g as usize]));
    }
    // /Widths: FirstChar..LastChar (32..last_char), font units
    let widths: Vec<u16> = (32..=sub.last_char)
        .map(|b| {
            let g = sub
                .chars
                .iter()
                .find(|c| c.1 == b)
                .map(|c| c.2)
                .unwrap_or(0);
            sub.widths[g as usize]
        })
        .collect();
    let widths_pdf: Vec<String> = widths.iter().map(|w| w.to_string()).collect();
    let font_file: Vec<u8> = {
        use flate2::{write::DeflateEncoder, Compression};
        use std::io::Write;
        let mut enc = DeflateEncoder::new(Vec::new(), Compression::fast());
        enc.write_all(&sub.font_bytes)?;
        enc.finish()?
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    // days → civil date (Howard Hinnant's civil-from-days)
    let z = now / 86400;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    let date = format!("D:{y:04}{m:02}{d:02}0000Z");

    // ---- object layout ---------------------------------------------------
    // 1 Catalog, 2 Pages, 3 Font, 4 FontDescriptor, 5 FontFile2, 6 Info,
    // 7..(6+np) Pages, (7+np)..(7+2np-1) contents
    let np = streams.len().max(1);
    let kids: Vec<String> = (7..(7 + np)).map(|i| format!("{i} 0 R")).collect();

    let title_hex: String = {
        let u16: Vec<u16> = title.encode_utf16().collect();
        let mut hex = String::with_capacity(2 * (1 + u16.len()) * 2);
        for b in &0xFEFFu16.to_be_bytes() {
            hex.push_str(&format!("{b:02X}"));
        }
        for u in &u16 {
            for b in &u.to_be_bytes() {
                hex.push_str(&format!("{b:02X}"));
            }
        }
        hex
    };

    let mut objects: Vec<String> = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        format!(
            "<< /Type /Pages /Kids [{}] /Count {np} >>",
            kids.join(" ")
        ),
        format!(
            "<< /Type /Font /Subtype /TrueType /BaseFont /Arial /Encoding \
             <</Differences [{}]>> /FirstChar 32 /LastChar {} /Widths [{}] \
             /FontDescriptor 4 0 R >>",
            diff.join(" "),
            sub.last_char,
            widths_pdf.join(" ")
        ),
        format!(
            "<< /Type /FontDescriptor /FontName /Arial /Flags 35 /StemV 80 \
             /ItalicAngle {} /Ascent {} /Descent {} /FontFile2 5 0 R >>",
            sub.italic_angle, sub.ascent, sub.descent
        ),
        String::new(), // the font-file stream (filled below)
        format!(
            "<< /Title <{title_hex}> /CreationDate ({date}) >>"
        ),
    ];
    for i in 0..np {
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] \
             /Resources << /Font << /F1 3 0 R >> /Contents {} 0 R >>",
            fnum(PAGE_W),
            fnum(PAGE_H),
            7 + np + i
        ));
    }
    for s in &streams {
        objects.push(format!("<< /Length {} >>", s.len()));
    }

    // ---- write -----------------------------------------------------------
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(b"%PDF-1.4\n%\xC2\xEB\xE6\xBF\xBD\n");
    let mut offsets: Vec<u32> = Vec::new();
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(out.len() as u32);
        out.extend_from_slice(&format!("{} 0 obj\n", i + 1).into_bytes());
        if i == 4 {
            // the FontFile2 stream
            out.extend_from_slice(
                &format!(
                    "<< /Length {} /Filter /FlateDecode >>\nstream\n",
                    font_file.len()
                )
                .into_bytes(),
            );
            out.extend_from_slice(&font_file);
            out.extend_from_slice(b"\nendstream\n");
        } else if i + 1 >= 7 + np {
            // a content stream: the dict was pushed into `objects` too
            let idx = i + 1 - (7 + np);
            out.extend_from_slice(obj.as_bytes());
            out.extend_from_slice(b"\nstream\n");
            out.extend_from_slice(&streams[idx]);
            out.extend_from_slice(b"\nendstream\n");
        } else {
            out.extend_from_slice(obj.as_bytes());
        }
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref_at = out.len() as u32;
    let n_obj = objects.len() + 1;
    out.extend_from_slice(&format!("xref\n0 {n_obj}\n").into_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for off in &offsets {
        out.extend_from_slice(&format!("{off:010} 00000 n \n").into_bytes());
    }
    out.extend_from_slice(
        &format!(
            "trailer\n<< /Size {n_obj} /Root 1 0 R /Info 6 0 R >>\nstartxref\n{xref_at}\n%%EOF\n"
        )
        .into_bytes(),
    );
    std::fs::write(path, out)?;
    Ok(())
}

/// Write one document to `path` as a minimal PDF (default system font).
pub fn write_pdf(path: &Path, title: &str, lines: &[&str]) -> Result<()> {
    let font_path = default_font_path();
    if !font_path.exists() {
        bail!("PDF font not found: {} (Arial)", font_path.display());
    }
    write_pdf_with_font(path, title, lines, &font_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(path: &str) -> std::path::PathBuf {
        std::path::Path::new(path).to_path_buf()
    }

    #[test]
    fn parses_arial() {
        let p = default_font_path();
        if !p.exists() {
            eprintln!("skip: arial.ttf missing");
            return;
        }
        let raw = std::fs::read(&p).unwrap();
        let f = parse(&raw).expect("parse arial.ttf");
        assert!(f.num_glyphs > 100, "num_glyphs={}", f.num_glyphs);
        assert!(f.units_per_em >= 1000, "upem={}", f.units_per_em);
        // a common glyph must exist
        assert!(f.cmap.lookup(b'A' as u32) != 0, "no glyph for 'A'");
        assert!(f.cmap.lookup(0x0456u32) != 0, "no glyph for 'і' (U+0456)");
        // a stale cmap segment may point past the font's own glyph table
        // (this machine's arial.ttf does: standard Cyrillic maps to gids
        // >= numGlyphs); such mappings must be clamped to .notdef
        for cp in [0x41u32, 0x0410, 0x0412, 0x0438, 0x0456, 0x00A0, 0x2014, 0x2022] {
            let g = f.cmap.lookup(cp);
            assert!(
                g < f.num_glyphs,
                "U+{cp:04X} -> gid {g} (num_glyphs={})",
                f.num_glyphs
            );
        }
    }

    #[test]
    fn subset_and_write() {
        let p = default_font_path();
        if !p.exists() {
            return;
        }
        let raw = std::fs::read(&p).unwrap();
        let f = parse(&raw).unwrap();
        let lines = vec!["ЗАТВЕРДЖЕНО", "Постанова № 175", "про ліміт зобов'язань", ""];
        let text: String = lines.join("\n");
        let sub = subset_font(&f, &text).unwrap();
        assert!(sub.font_bytes.len() < 300_000, "subset too big: {}", sub.font_bytes.len());
        // the subset file must be a valid sfnt we can parse back
        let f2 = parse(&sub.font_bytes).expect("re-parse subset");
        assert_eq!(f2.num_glyphs as usize, sub.widths.len());
        // a used Cyrillic glyph must survive the subset round-trip with a
        // real (non-.notdef) gid: 'і' (U+0456) has a valid glyph in arial
        let g = f2.cmap.lookup(0x0456u32);
        assert!(
            g != 0 && (g as usize) < f2.num_glyphs as usize,
            "'і' gid lost in the subset: {g} (num_glyphs={})",
            f2.num_glyphs
        );

        let out = tmp(r"C:\temp\opencode\pdf_test.doc.pdf");
        write_pdf_with_font(&out, "test doc", &lines, &p).unwrap();
        let bytes = std::fs::read(&out).unwrap();
        assert!(bytes.starts_with(b"%PDF-1.4"), "pdf magic");
        let s = String::from_utf8_lossy(&bytes).to_string();
        assert!(s.contains("/Type /Page"), "page obj");
        assert!(s.contains("startxref"), "xref");
        assert!(s.contains("%%EOF"), "eof");
        let _ = std::fs::remove_file(&out);
    }
}
