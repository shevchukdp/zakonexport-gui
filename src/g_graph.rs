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
