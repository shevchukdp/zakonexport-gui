//! Вбудовані додатки (BZh9 BZip2-потоки) всередині chunks `text\*.mb`.
//!
//! The Zakon base stores document annexes (Word/Excel/PDF/RTF/…) INLINE in the
//! `text\{file}.mb` chunk that carries the document's records. Each annex is a
//! BZip2 stream (`BZh9…`) prefixed by a short ASCII type-name
//! (`doc`, `docx`, `rtf`, `xls`, `xlsx`, `pdf`, `jpg`, `rar`, …). That is why
//! a raw-signature scan for `D0CF11E0` / `%PDF` / `{\rtf` / `PK\x03\x04` finds
//! nothing: the payloads are BZip2-compressed, so their inner signatures never
//! appear.
//!
//! Verified by extraction (7-Zip BZip2 codec): a decoded stream yields a real
//! file of the declared type (OLE2 Word, OOXML .docx/.xlsx/.pptx, %PDF-1.x,
//! `{\rtf1}`, JFIF JPEG, RAR).
//!
//! A document's annexes are exactly the BZh9 streams inside ITS OWN chunk
//! (docnum → unique (file, chunk) via docloc.db; no other docnum shares a
//! chunk). So "related attachments of the previewed document" = the BZh9
//! streams in that document's chunk.

use std::io::Read;

/// One embedded annex: a BZip2 stream inside the document's chunk.
pub struct Annex {
    /// type-name read backwards from the BZh9 magic (may be `"??"` when the
    /// preceding byte is not alphanumeric; the decoded magic is authoritative).
    pub ext: String,
    /// offset of the BZh9 magic within the chunk
    pub start: usize,
    /// exclusive end within the chunk (next BZh9, or the chunk end)
    pub end: usize,
}

/// type-names the base actually writes (sanitized on use).
const KNOWN_EXTS: &[&str] = &[
    "doc", "docx", "dot", "dotx", "rtf", "xls", "xlsx", "wbk", "pdf", "jpg", "jpeg", "gif",
    "png", "zip", "rar", "7z", "pub", "log", "pptx", "csv", "txt",
];

/// Read the alnum type-name that immediately precedes a BZh9 offset.
fn type_name(chunk: &[u8], bz: usize) -> String {
    let mut j = bz;
    let mut name = String::new();
    while j > 0 && name.len() < 8 {
        let c = chunk[j - 1];
        if c.is_ascii_alphanumeric() {
            name.insert(0, c as char);
            j -= 1;
        } else {
            break;
        }
    }
    name.to_ascii_lowercase()
}

/// Extension from the decoded file's magic (authoritative when the stored
/// name is missing/unknown).
fn ext_from_magic(b: &[u8]) -> String {
    if b.len() >= 8 && b[..8] == [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1] {
        "doc".into() // OLE2/CFB: Word .doc / Excel .xls (binary)
    } else if b.len() >= 4 && b[..4] == [0x50, 0x4B, 0x03, 0x04] {
        "zip".into() // OOXML .docx/.xlsx/.pptx or a plain zip
    } else if b.len() >= 4 && b[..4] == [0x25, 0x50, 0x44, 0x46] {
        "pdf".into()
    } else if b.len() >= 5 && b[..5] == [0x7B, 0x5C, 0x72, 0x74, 0x66] {
        "rtf".into()
    } else if b.len() >= 3 && b[..3] == [0xFF, 0xD8, 0xFF] {
        "jpg".into()
    } else if b.len() >= 4 && b[..4] == [0x52, 0x61, 0x72, 0x21] {
        "rar".into()
    } else {
        "bin".into()
    }
}

/// Find every BZh9 annex stream in a document's chunk.
pub fn find_annexes(chunk: &[u8]) -> Vec<Annex> {
    let n = chunk.len();
    let mut offs: Vec<usize> = Vec::new();
    let mut i = 0usize;
    while i + 4 <= n {
        if chunk[i] == b'B' && chunk[i + 1] == b'Z' && chunk[i + 2] == b'h' && chunk[i + 3] == b'9' {
            offs.push(i);
            i += 1;
        } else {
            i += 1;
        }
    }
    let mut v = Vec::with_capacity(offs.len());
    for (k, &o) in offs.iter().enumerate() {
        let ext = type_name(chunk, o);
        let end = offs.get(k + 1).copied().unwrap_or(n);
        v.push(Annex { ext, start: o, end });
    }
    v
}

/// BZip2-decode one annex → (extension, decoded file bytes).
///
/// The window is `[start..end)` = one BZip2 stream plus the (non-BZip2) gap up
/// to the next annex. A counting reader feeds the decoder; the first stream
/// decodes cleanly, then the decoder stops at the non-`BZh` gap. The decoded
/// output is the complete annex file.
pub fn decode(chunk: &[u8], a: &Annex) -> (String, Vec<u8>) {
    let data = &chunk[a.start..a.end];
    struct Counting<'a> {
        d: &'a [u8],
        p: usize,
    }
    impl<'a> Read for Counting<'a> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = buf.len().min(self.d.len() - self.p);
            if n == 0 {
                return Ok(0);
            }
            buf[..n].copy_from_slice(&self.d[self.p..self.p + n]);
            self.p += n;
            Ok(n)
        }
    }
    let mut r = Counting { d: data, p: 0 };
    let mut dec = bzip2::read::BzDecoder::new(&mut r);
    let mut out = Vec::with_capacity(a.end - a.start);
    // The gap after the stream (next-annex territory) is not a BZip2 stream, so
    // read_to_end may error there — the first stream's output is already in `out`.
    let _ = dec.read_to_end(&mut out);

    let stored = a.ext.to_ascii_lowercase();
    let ext = if KNOWN_EXTS.contains(&stored.as_str()) && !stored.is_empty() {
        stored
    } else {
        ext_from_magic(&out)
    };
    (ext, out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn finds_bzh9_streams() {
        // fake chunk: two BZh9 streams with a type-name before each + a gap
        let mut c = Vec::new();
        c.extend_from_slice(b"docxBZh9first");
        c.extend_from_slice(b"~~~!!"); // non-alnum gap (type_name must stop here)
        c.extend_from_slice(b"rtfBZh9second");
        let a = find_annexes(&c);
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].ext, "docx");
        assert_eq!(a[1].ext, "rtf");
        assert_eq!(a[0].start, 4);
        assert_eq!(a[0].end, 21); // up to the 2nd BZh9
        assert_eq!(a[1].end, c.len());
    }

    #[test]
    fn decodes_a_real_bzip2_stream() {
        // a real 55-byte BZip2 stream of "hello bzip2 annex" (7z -tbzip2).
        // Prefix it with a type-name so type_name() sees "doc".
        const BZ: &[u8] = &[
            0x42, 0x5a, 0x68, 0x39, 0x31, 0x41, 0x59, 0x26, 0x53, 0x59, 0xc7, 0x7e, 0x74, 0x56,
            0x00, 0x00, 0x03, 0x99, 0x80, 0x40, 0x00, 0x10, 0x00, 0x32, 0x65, 0xc0, 0x50, 0x20,
            0x00, 0x31, 0x00, 0x00, 0xc5, 0x30, 0x06, 0x9a, 0x26, 0x06, 0xf5, 0x0d, 0x20, 0xb8,
            0x52, 0xe7, 0x85, 0xdc, 0x91, 0x4e, 0x14, 0x24, 0x31, 0xdf, 0x9d, 0x15, 0x80,
        ];
        let mut chunk = Vec::new();
        chunk.extend_from_slice(b"doc");
        chunk.extend_from_slice(BZ);
        let a = find_annexes(&chunk);
        assert_eq!(a.len(), 1);
        let (ext, out) = decode(&chunk, &a[0]);
        assert_eq!(ext, "doc");
        assert_eq!(out, b"hello bzip2 annex");
    }

    #[test]
    fn gk_doc9275_real_annexes() {
        // Real ДКРС base (if present on this machine): the document mapped by
        // docloc record 9275 (file 128, chunk 33) carries four embedded .docx
        // annexes inside its own chunk.
        let baza = Path::new(crate::BASE_AUDIT);
        if !baza.exists() {
            return;
        }
        let base = crate::render::Base::load(baza).unwrap();
        let chunk = base.chunk(baza, 9275).unwrap();
        let a = find_annexes(&chunk);
        assert_eq!(a.len(), 4, "doc 9275 should have 4 embedded annexes");
        for x in &a {
            assert_eq!(x.ext, "docx", "all annexes of 9275 are docx");
        }
        // the first decodes to a valid OOXML (.docx = zip) file
        let (ext, bytes) = decode(&chunk, &a[0]);
        assert_eq!(ext, "docx");
        assert!(
            bytes.len() > 4 && &bytes[0..4] == b"PK\x03\x04",
            "decoded annex is not a zip/OOXML: len={}",
            bytes.len()
        );
    }
}
