//! The Trident morphology "grammar" graph (`.g`) — structural decode.
//!
//! Layout (verified against the real `lmu.g` / `lmr.g` files):
//!
//! ```text
//! 0x00  "Trident Software, Inc." (22 B) + NUL + 1 pad NUL
//! 0x18  "grammar" + NUL (+ 2 NULs)
//! 0x22  language char ('U' / 'R') + NUL
//! 0x24  template file name (NUL-terminated 12-byte slot:
//!       "TEMPLATE.TXT" in lmu.g, "lmr.gt" in lmr.g)
//! 0x30–0x5F  reserved NULs
//! 0x60  the u16 graph stream (template-walk data; the edge semantics
//!       live in the DLL's graph-walk code — a later item)
//! ```
//!
//! Both real files end with NUL-terminated vertex/arc marker names:
//! `POINT1`, `POINT2`, `STEP`, `REPEAT` — the word-building template
//! markers. The `.gw` companion file is a flat u16 weight stream (the
//! 0x8000 flag bit = the documented weight flag; the four trailing
//! `0x8053 0x8054 0x8057 0x8058 0 0` are terminator markers).

/// The file's vendor magic (19 bytes + NUL at 0x13).
pub const MAGIC_VENDOR: &[u8] = b"Trident Software, Inc.";
/// The format tag at 0x14.
pub const MAGIC_TAG: &[u8] = b"grammar";
/// The NUL-terminated vertex/arc marker names at the file tail.
pub const TAIL_MARKERS: [&str; 4] = ["POINT1", "POINT2", "STEP", "REPEAT"];

/// A structurally decoded `.g` grammar graph.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GGraph {
    /// the header language char ('U' = Ukrainian, 'R' = Russian)
    pub language: char,
    /// the template file name from the header ("TEMPLATE.TXT" / "lmr.gt")
    pub template: String,
    /// the offset where the u16 graph stream starts (0x60 in both files)
    pub stream_off: u32,
    /// the number of u16 values in the stream
    pub stream_u16: u32,
    /// the first u16 values of the stream (display / fingerprint)
    pub stream_head: Vec<u16>,
    /// which of the known tail markers are present in the file
    pub tail_markers: Vec<&'static str>,
}

impl GGraph {
    /// Parse the header + stream fingerprint of a `.g` file.
    ///
    /// Returns `None` when the buffer is too short or the magic is not the
    /// Trident vendor magic (a truncated / foreign file).
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 0x60 {
            return None;
        }
        let vendor = &bytes[0..MAGIC_VENDOR.len()];
        if vendor != MAGIC_VENDOR || bytes[MAGIC_VENDOR.len()] != 0x00 {
            return None;
        }
        let tag_start = 0x18;
        if &bytes[tag_start..tag_start + MAGIC_TAG.len()] != MAGIC_TAG {
            return None;
        }
        let language = bytes[0x22] as char;
        if !(language.is_ascii_alphabetic()) {
            return None;
        }
        // template name: NUL-terminated 12-byte slot at 0x24
        let mut template = String::new();
        for &b in &bytes[0x24..0x30] {
            if b == 0x00 {
                break;
            }
            template.push(b as char);
        }
        // the u16 stream: the first non-zero byte at/after 0x60
        let mut stream_off = bytes.len();
        for (i, &b) in bytes[0x60..].iter().enumerate() {
            if b != 0x00 {
                stream_off = 0x60 + i;
                break;
            }
        }
        let stream_off = stream_off as u32;
        let stream_len = bytes.len().saturating_sub(stream_off as usize);
        let stream_u16 = (stream_len / 2) as u32;
        let mut stream_head = Vec::new();
        for i in 0..16 {
            let o = stream_off as usize + 2 * i;
            if o + 2 > bytes.len() {
                break;
            }
            stream_head.push(u16::from_le_bytes([bytes[o], bytes[o + 1]]));
        }
        // the known tail markers (each NUL-terminated)
        let tail_markers = TAIL_MARKERS
            .iter()
            .copied()
            .filter(|m| {
                let mb = m.as_bytes();
                bytes.windows(mb.len() + 1).any(|w| {
                    &w[..mb.len()] == mb && w[mb.len()] == 0x00
                })
            })
            .collect();
        Some(GGraph {
            language,
            template,
            stream_off,
            stream_u16,
            stream_head,
            tail_markers,
        })
    }
}

/// The `.gw` companion: a flat u16 weight stream.
///
/// Returns `(weights, flagged)` — the raw u16 values and the count of
/// values carrying the 0x8000 flag bit. An odd trailing byte is dropped.
pub fn parse_gw(gw: &[u8]) -> (Vec<u16>, u32) {
    let mut out = Vec::with_capacity(gw.len() / 2);
    for pair in gw.chunks_exact(2) {
        let v = u16::from_le_bytes([pair[0], pair[1]]);
        out.push(v);
    }
    let flagged = out.iter().filter(|v| **v & 0x8000 != 0).count() as u32;
    (out, flagged)
}

// ---------------------------------------------------------------------------
// v0.33.0 (roadmap v0.34.0): the graph-template walker (part 1)
//
// The `FUN_10019060`/`FUN_10019300` pair is the DLL's byte-marker graph
// walker: it reads the u16 stream as a byte sequence and dispatches on the
// marker bytes — `0x27` (`'`) = a **vertex** record (`| 2` flag, a named
// point), `0x2d` (`-`) = an **edge / arc** record (`FUN_10011830` link),
// `0x28` (`(`) = a sub-graph terminator, `0x2e..0x31` = a `FUN_100166e0`
// child reference, `0x32` = a length-prefixed template-char run, `0x3b` =
// a nested structure. The full walk builds a 16-byte slot array (vertices +
// edges) via the `DAT_1002f1f0` template table — the part-2 DLL-image pin.
//
// **The structural skeleton (this part-1)** counts the two primary marker
// classes over `[stream_off, len)` and records the consumed u16 length (the
// stream-end fingerprint); the `.gw` companion's 134 weights (0x8000 flag
// = a weighted / active edge) are mapped onto the edges by index.
//
// Graph shape (UA+EN): a word-building template — `POINT1`/`POINT2` are the
// anchor vertices, `STEP`/`REPEAT` are the arc kinds (the tail marker
// names). EN: a word-building template graph — `POINT1`/`POINT2` are the
// anchor vertices, `STEP`/`REPEAT` are the arc kinds (the tail marker
// names); each arc optionally carries a `.gw` weight (0x8000 = active).
// ---------------------------------------------------------------------------

/// One decoded `.g` graph **vertex** (the `0x27` marker record, roadmap
/// v0.34.0): the walker's `| 2` named-point record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GVertex {
    /// the vertex's stream offset (where the `0x27` marker sits).
    pub off: u32,
    /// the record flags (the walker's low flag byte: `2` = named, `8` = a
    /// char, `9` = a char with a value — structural, part-1).
    pub flags: u8,
}

/// One decoded `.g` graph **edge / arc** (the `0x2d` marker record, roadmap
/// v0.34.0), optionally weighted by a `.gw` companion value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GEdge {
    /// the edge's stream offset (where the `0x2d` marker sits).
    pub off: u32,
    /// the `.gw` weight value (the `0x8000` flag bit = a weighted / active
    /// edge); `None` when no `.gw` value maps to this edge index.
    pub weight: Option<u16>,
}

/// The result of a structural (part-1) walk of a `.g` u16 graph stream:
/// the vertex / edge marker counts + the consumed stream length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GGraphWalk {
    /// the number of `0x27` vertex markers in the stream region.
    pub vertices: u32,
    /// the number of `0x2d` edge / arc markers in the stream region.
    pub edges: u32,
    /// the number of u16 values consumed (== [`GGraph::stream_u16`] — the
    /// stream-end fingerprint; the walk terminates at the stream end).
    pub consumed: u32,
}

impl GGraph {
    /// A structural (part-1) walk of the `.g` u16 graph stream: count the
    /// `0x27` vertex markers + the `0x2d` edge markers over
    /// `[stream_off, len)` and record the consumed u16 length (the stream-end
    /// fingerprint). The full `FUN_10019060` / `FUN_10019300` graph
    /// construction (the `DAT_1002f1f0` template table) is the part-2
    /// DLL-image pin, deferred. `stream` is the full `.g` file.
    pub fn walk(&self, stream: &[u8]) -> GGraphWalk {
        let region = &stream[self.stream_off as usize..];
        let vertices = region.iter().filter(|&&b| b == 0x27).count() as u32;
        let edges = region.iter().filter(|&&b| b == 0x2d).count() as u32;
        // the consumed u16 count over the walked region (== stream_u16 when
        // `stream` is the whole file — the stream-end fingerprint)
        let consumed = (region.len() / 2) as u32;
        GGraphWalk {
            vertices,
            edges,
            consumed,
        }
    }

    /// The decoded vertices of the stream (the `0x27` marker offsets) — a
    /// structural listing (the full vertex records need the part-2 table).
    pub fn vertices(&self, stream: &[u8]) -> Vec<GVertex> {
        let region = &stream[self.stream_off as usize..];
        region
            .iter()
            .enumerate()
            .filter(|&(_, b)| *b == 0x27)
            .map(|(i, &b)| GVertex {
                off: self.stream_off + i as u32,
                flags: b,
            })
            .collect()
    }

    /// The decoded edges / arcs of the stream (the `0x2d` marker offsets).
    pub fn edges(&self, stream: &[u8]) -> Vec<GEdge> {
        let region = &stream[self.stream_off as usize..];
        region
            .iter()
            .enumerate()
            .filter(|&(_, b)| *b == 0x2d)
            .map(|(i, _)| GEdge {
                off: self.stream_off + i as u32,
                weight: None,
            })
            .collect()
    }
}

/// Map the `.gw` companion weight stream onto the walked edges (roadmap
/// v0.34.0): edge `i` carries `weights[i]` when `i < weights.len()` (the
/// 134-value stream is longer than the edge set — the surplus is the
/// template's own weight table). The `0x8000` flag bit on a weight marks a
/// weighted / active edge. Returns `(per-edge weights, flagged count)`.
pub fn map_gw_weights(weights: &[u16], edge_count: u32) -> (Vec<Option<u16>>, u32) {
    let mapped: Vec<Option<u16>> = (0..edge_count)
        .map(|i| weights.get(i as usize).copied())
        .collect();
    let flagged = mapped
        .iter()
        .filter(|w| matches!(w, Some(v) if v & 0x8000 != 0))
        .count() as u32;
    (mapped, flagged)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn synth_lang(l: u8, template: &str) -> Vec<u8> {
        let mut b = vec![0x00; 0x100];
        b[..22].copy_from_slice(MAGIC_VENDOR);
        b[0x18..0x1F].copy_from_slice(MAGIC_TAG);
        b[0x22] = l;
        let t = template.as_bytes();
        b[0x24..0x24 + t.len()].copy_from_slice(t);
        b[0x60] = 0x02;
        b[0x61] = 0x00;
        b
    }

    #[test]
    fn parses_the_header_fields() {
        let g = GGraph::parse(&synth_lang(b'U', "TEMPLATE.TXT")).expect("synth");
        assert_eq!(g.language, 'U');
        assert_eq!(g.template, "TEMPLATE.TXT");
        assert_eq!(g.stream_off, 0x60);
        assert_eq!(g.stream_head.first().copied(), Some(2));
    }

    #[test]
    fn rejects_short_and_foreign() {
        assert!(GGraph::parse(&[0u8; 0x5F]).is_none(), "too short");
        let mut b = synth_lang(b'U', "X");
        b[0] = b'Q';
        assert!(GGraph::parse(&b).is_none(), "bad vendor magic");
        let mut b = synth_lang(b'U', "X");
        b[0x18] = b'g';
        b[0x19] = b'x';
        assert!(GGraph::parse(&b).is_none(), "bad format tag");
    }

    #[test]
    fn gw_stream_flags() {
        // 3 values: 0x0044 (plain), 0x8053 (flagged), 0x8000 (flag only)
        let (w, f) = parse_gw(&[0x44, 0x00, 0x53, 0x80, 0x00, 0x80]);
        assert_eq!(w, vec![0x0044, 0x8053, 0x8000]);
        assert_eq!(f, 2);
        // an odd trailing byte is dropped
        assert_eq!(parse_gw(&[0x01, 0x00, 0x02]).0, vec![0x0001]);
    }

    #[test]
    fn real_lmu_g_parses() {
        let Some(v) = std::fs::read(r"C:\Zakon.Local\Baza\SOFT\lmu.g").ok() else {
            eprintln!("skipped: no base");
            return;
        };
        let g = GGraph::parse(&v).expect("lmu.g");
        assert_eq!(g.language, 'U', "ukr language char");
        assert_eq!(g.template, "TEMPLATE.TXT");
        assert_eq!(g.stream_off, 0x60);
        assert_eq!(g.stream_u16, ((v.len() - 0x60) / 2) as u32);
        // the observed stream head (fingerprint)
        assert_eq!(
            g.stream_head[..9],
            [2, 0x1c, 2, 0xc, 1, 0xdd0a, 0, 0x14c, 0xf000],
            "the lmu.g stream head"
        );
        // all four tail markers are present in the real file
        assert_eq!(
            g.tail_markers,
            vec!["POINT1", "POINT2", "STEP", "REPEAT"]
        );
    }

    #[test]
    fn real_lmr_g_parses_russian() {
        let Some(v) = std::fs::read(r"C:\Zakon.Local\Baza\SOFT\lmr.g").ok() else {
            eprintln!("skipped: no base");
            return;
        };
        let g = GGraph::parse(&v).expect("lmr.g");
        assert_eq!(g.language, 'R', "rus language char");
        assert_eq!(g.template, "lmr.gt");
        assert_eq!(g.stream_off, 0x60);
        assert_eq!(g.stream_head[..5], [2, 0x1c, 2, 0xc, 1]);
        assert_eq!(
            g.tail_markers,
            vec!["POINT1", "POINT2", "STEP", "REPEAT"]
        );
    }

    #[test]
    fn walk_counts_markers_and_consumed() {
        // a synthetic stream: 0x60 start, with a couple of 0x27/0x2d markers
        let mut b = synth_lang(b'U', "TEMPLATE.TXT");
        b[0x60] = 0x02;
        b[0x61] = 0x00;
        b[0x64] = 0x27; // vertex marker
        b[0x66] = 0x2d; // edge marker
        b[0x68] = 0x27; // vertex marker
        let g = GGraph::parse(&b).expect("synth");
        let w = g.walk(&b);
        assert_eq!(w.vertices, 2, "two 0x27 vertex markers");
        assert_eq!(w.edges, 1, "one 0x2d edge marker");
        // consumed == the walked region's u16 count (== stream_u16 for the
        // whole file)
        assert_eq!(w.consumed, g.stream_u16, "consumed is the stream-end fingerprint");
        // the decoded listings match the counts
        assert_eq!(g.vertices(&b).len(), 2);
        assert_eq!(g.edges(&b).len(), 1);
        assert_eq!(g.vertices(&b)[0].off, 0x64);
        assert_eq!(g.edges(&b)[0].off, 0x66);
        assert_eq!(g.vertices(&b)[0].flags, 0x27, "the raw marker byte is the flag");
    }

    #[test]
    fn map_gw_weights_clamps_and_flags() {
        // a 134-value stream onto a 3-edge set: edges 0..3 get values, the
        // surplus is dropped; the flagged count only counts mapped weights
        let weights: Vec<u16> = (0..134).map(|i| if i < 2 { 0x8000 } else { 0x0044 }).collect();
        let (mapped, flagged) = map_gw_weights(&weights, 3);
        assert_eq!(mapped.len(), 3);
        assert_eq!(mapped[0], Some(0x8000));
        assert_eq!(mapped[1], Some(0x8000));
        assert_eq!(mapped[2], Some(0x0044), "edge 2 carries the plain weight");
        assert_eq!(flagged, 2, "two mapped weights carry the 0x8000 flag");
        // an edge set larger than the stream: the surplus edges are None
        let (mapped, flagged) = map_gw_weights(&[0x8053], 3);
        assert_eq!(mapped, vec![Some(0x8053), None, None]);
        assert_eq!(flagged, 1);
        // an empty edge set maps to nothing
        let (mapped, flagged) = map_gw_weights(&[0x8053], 0);
        assert!(mapped.is_empty());
        assert_eq!(flagged, 0);
    }

    #[test]
    fn real_lmu_g_walk_fingerprint() {
        let Some(v) = std::fs::read(r"C:\Zakon.Local\Baza\SOFT\lmu.g").ok() else {
            eprintln!("skipped: no base");
            return;
        };
        let g = GGraph::parse(&v).expect("lmu.g");
        let w = g.walk(&v);
        assert_eq!(w.vertices, 31, "31 0x27 vertex markers in lmu.g");
        assert_eq!(w.edges, 26, "26 0x2d edge markers in lmu.g");
        // the consumed fingerprint == the full u16 stream length
        assert_eq!(w.consumed, g.stream_u16, "walk terminates at the stream end");
    }

    #[test]
    fn real_lmr_g_walk_fingerprint() {
        let Some(v) = std::fs::read(r"C:\Zakon.Local\Baza\SOFT\lmr.g").ok() else {
            eprintln!("skipped: no base");
            return;
        };
        let g = GGraph::parse(&v).expect("lmr.g");
        let w = g.walk(&v);
        assert_eq!(w.vertices, 37, "37 0x27 vertex markers in lmr.g");
        assert_eq!(w.edges, 47, "47 0x2d edge markers in lmr.g");
        assert_eq!(w.consumed, g.stream_u16, "walk terminates at the stream end");
    }

    #[test]
    fn real_lmu_gw_maps_onto_edges() {
        let Some(graw) = std::fs::read(r"C:\Zakon.Local\Baza\SOFT\lmu.g").ok() else {
            eprintln!("skipped: no base");
            return;
        };
        let Some(gw) = std::fs::read(r"C:\Zakon.Local\Baza\SOFT\lmu.gw").ok() else {
            eprintln!("skipped: no base");
            return;
        };
        let g = GGraph::parse(&graw).expect("lmu.g");
        let w = g.walk(&graw);
        let (weights, _) = parse_gw(&gw);
        let (mapped, flagged) = map_gw_weights(&weights, w.edges);
        // 26 walked edges against a 134-value stream: the first 26 map by
        // index (the surplus is the template's own weight table). The mapped
        // values are index-aligned with the stream head (all plain, low
        // weights — the 0x8000-flagged weights live in the template-table
        // region of the 134-stream, not on the walked edge set).
        assert_eq!(mapped.len(), 26 as usize);
        for i in 0..26 {
            assert_eq!(mapped[i], Some(weights[i]), "edge {i} maps to weights[{i}]");
        }
        // the flagged count over the walked-edge subset (0 here: the 0x80xx
        // weights are past index 26) — documented, not asserted >= 1
        assert_eq!(
            flagged,
            (0..26).filter(|i| weights[*i] & 0x8000 != 0).count() as u32
        );
    }

    #[test]
    fn real_lmu_gw_is_134_weights() {
        let Some(v) = std::fs::read(r"C:\Zakon.Local\Baza\SOFT\lmu.gw").ok() else {
            eprintln!("skipped: no base");
            return;
        };
        let (w, f) = parse_gw(&v);
        assert_eq!(w.len(), 134, "134 u16 weights");
        // the observed terminator markers (last six)
        assert_eq!(
            &w[128..],
            &[0x8053, 0x8054, 0x8057, 0x8058, 0x0000, 0x0000]
        );
        assert!(f > 0, "the 0x8000 flag bit is used");
    }
}
