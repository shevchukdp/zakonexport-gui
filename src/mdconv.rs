//! Конвертація вбудованих додатків (BZh9-додатки з `text\*.mb`) у Markdown.
//!
//! The annex extractor (`attach.rs`) yields a decoded file (Word/Excel/PDF/RTF/…).
//! This module turns that decoded file into a Markdown representation so it can be
//! shown INTERNALLY in the GUI (a read-only text view) and exported as a `.md`
//! file — no external Word/Excel/PDF app required.
//!
//! Per-type strategy:
//! - **docx / dotx / pptx / zip** → inflate the OOXML zip entry
//!   (`word/document.xml` or the first `ppt/slides/slideN.xml`) and convert the
//!   XML (paragraphs, tabs, breaks, table cells) to Markdown text.
//! - **xlsx** → inflate `xl/sharedStrings.xml` + the first worksheet and build a
//!   Markdown table (cells resolved through the shared-string table).
//! - **rtf** → strip RTF control words / groups → plain text.
//! - **pdf** → best-effort: inflate each content `stream`, pull the text shown by
//!   `Tj`/`TJ`/`'`/`"` operators.
//! - **doc / dot / wbk / xls** (OLE2 compound) → extract the readable UTF-16LE /
//!   ASCII string runs (the document text is embedded in the WordDocument stream).
//! - **jpg / jpeg / png / gif** → a base64 data-URI image (renders in a Markdown
//!   viewer / browser).
//! - **rar / 7z** and anything else → a short note (+ readable strings if any).
//!
//! Every converter is total: it returns a `String` and never panics; on failure it
//! yields a human note rather than crashing the preview.

use std::io::Read as _;

/// Convert one decoded annex into Markdown text (plain-text mode: formatting
/// is dropped, structure kept). This is what «Вивантажити» exports.
pub fn to_markdown(ext: &str, bytes: &[u8]) -> String {
    match ext.to_ascii_lowercase().as_str() {
        "docx" | "dotx" => docx_to_md(bytes),
        "pptx" => pptx_to_md(bytes),
        "xlsx" => xlsx_to_md(bytes),
        "rtf" => rtf_to_md(bytes),
        "pdf" => pdf_to_md(bytes),
        "doc" | "dot" | "wbk" | "xls" => ole2_strings(bytes, ext),
        "jpg" | "jpeg" | "png" | "gif" => image_to_md(ext, bytes),
        "rar" | "7z" => format!("Архів .{ext} ({} Б) — відкрийте архіватором.", bytes.len()),
        "txt" | "log" | "csv" => String::from_utf8_lossy(bytes).to_string(),
        "zip" => generic_zip_or_note(ext, bytes),
        _ => generic(bytes),
    }
}

/// Convert one decoded annex into Markdown **with the original formatting
/// preserved** (headings as `#…`, `**bold**`, `*italic*`, markdown lists,
/// tables). Used by the internal preview's «Формат (md)» mode. Types without
/// recoverable formatting fall back to the plain conversion.
pub fn to_markdown_fmt(ext: &str, bytes: &[u8]) -> String {
    match ext.to_ascii_lowercase().as_str() {
        "docx" | "dotx" => docx_to_md_fmt(bytes),
        "rtf" => rtf_to_md_fmt(bytes),
        _ => to_markdown(ext, bytes),
    }
}

/// Combined Markdown for all annexes of a document (plain mode) — used for
/// the `<docnum>_dodatk.md` export file.
pub fn annexes_markdown(docnum: u32, items: &[(String, Vec<u8>)]) -> String {
    build_annexes(docnum, items, to_markdown)
}

/// Combined Markdown for all annexes of a document (formatted mode) — used
/// by the internal preview window.
pub fn annexes_markdown_fmt(docnum: u32, items: &[(String, Vec<u8>)]) -> String {
    build_annexes(docnum, items, to_markdown_fmt)
}

fn build_annexes(
    docnum: u32,
    items: &[(String, Vec<u8>)],
    conv: fn(&str, &[u8]) -> String,
) -> String {
    let mut out = String::with_capacity(1024);
    out.push_str(&format!("# Додатки до документу {docnum}\n"));
    for (i, (ext, bytes)) in items.iter().enumerate() {
        out.push_str(&format!(
            "\n## Додаток {} (.{ext}) — {} Б\n\n",
            i + 1,
            bytes.len()
        ));
        out.push_str(&conv(ext, bytes));
        out.push_str("\n");
    }
    out
}

// ---------------------------------------------------------------------------
// OOXML (docx / pptx)
// ---------------------------------------------------------------------------

fn docx_to_md(bytes: &[u8]) -> String {
    match zip_entry(bytes, "word/document.xml") {
        Some(xml) => ooxml_xml_to_md(&xml),
        None => "DOCX: не знайдено текстового вмісту (word/document.xml).".into(),
    }
}

fn pptx_to_md(bytes: &[u8]) -> String {
    // first slide; fall back to the first slideN.xml present
    let first = zip_entries(bytes)
        .into_iter()
        .find(|(n, _)| n.starts_with("ppt/slides/slide") && n.ends_with(".xml"));
    let name = first
        .as_ref()
        .map(|(n, _)| n.clone())
        .unwrap_or_else(|| "ppt/slides/slide1.xml".to_string());
    match zip_entry(bytes, &name) {
        Some(xml) => ooxml_xml_to_md(&xml),
        None => format!("PPTX: не знайдено слайдів ({name})."),
    }
}

/// OOXML body XML (Word `w:` / PowerPoint `a:`) → Markdown-ish text.
fn ooxml_xml_to_md(xml: &[u8]) -> String {
    let mut s = String::from_utf8_lossy(xml).to_string();
    // tabs / line breaks (both namespaces, self-closed and attributed forms)
    for pat in &["<w:tab/>", "<a:tab/>"] {
        s = replace_all(&s, pat, "\t");
    }
    for pat in &["<w:br/>", "<a:br/>"] {
        s = replace_all(&s, pat, "\n");
    }
    // table cells / rows (docx) → markdown table markers
    s = replace_tag(&s, "<w:tc>", " | ");
    s = replace_tag(&s, "<w:tc ", " | ");
    s = replace_tag(&s, "<w:tr>", "\n");
    s = replace_tag(&s, "<w:tr ", "\n");
    // paragraph boundaries (attributed forms keep their attributes out too)
    s = replace_tag(&s, "<w:p>", "\n");
    s = replace_tag(&s, "<w:p ", "\n");
    s = replace_tag(&s, "<a:p>", "\n");
    s = replace_tag(&s, "<a:p ", "\n");
    // drop every remaining tag, keep the inner text (w:t / a:t content)
    let s = strip_tags(&s);
    let s = unescape(&s);
    let s = s.replace('\u{1}', " | ").replace('\u{2}', "\n");
    collapse_blank(s.trim())
}

/// Replace complete opening tags whose text starts with `open` (e.g. `<w:p>` or
/// the attributed `<w:p `) with `rep`, consuming the tag's closing `>`.
fn replace_tag(s: &str, open: &str, rep: &str) -> String {
    if open.ends_with('>') {
        return s.replace(open, rep);
    }
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        let Some(a_rel) = s[i..].find(open) else {
            out.push_str(&s[i..]);
            break;
        };
        let a = i + a_rel;
        out.push_str(&s[i..a]);
        match s[a..].find('>') {
            Some(g) => {
                out.push_str(rep);
                i = a + g + 1;
            }
            None => {
                out.push_str(rep);
                i = s.len();
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// OOXML formatted mode (docx → Markdown with headings / bold / italic / lists)
// ---------------------------------------------------------------------------

fn docx_to_md_fmt(bytes: &[u8]) -> String {
    match zip_entry(bytes, "word/document.xml") {
        Some(xml) => ooxml_xml_to_md_fmt(&xml),
        None => "DOCX: не знайдено текстового вмісту (word/document.xml).".into(),
    }
}

/// OOXML body XML → Markdown WITH the original formatting preserved:
/// paragraph styles (Heading N / Title / Підзаголовок) become `#…`, bold /
/// italic runs become `**…**` / `*…*`, list paragraphs become `- …` items
/// (indented by the paragraph indent), tables keep their `|` cells.
/// Best-effort — Word has thousands of ways to lay this out, this is the
/// common subset.
fn ooxml_xml_to_md_fmt(xml: &[u8]) -> String {
    let s = String::from_utf8_lossy(xml).to_string();
    let mut out = String::new();
    let mut pos = 0usize;
    while pos < s.len() {
        // the next paragraph start: the earliest of `<w:p>` / `<w:p …>`
        let a = s[pos..].find("<w:p>");
        let b = s[pos..].find("<w:p ");
        let rel = match (a, b) {
            (Some(x), Some(y)) => x.min(y),
            (Some(x), None) => x,
            (None, Some(y)) => y,
            (None, None) => break,
        };
        let pstart = pos + rel;
        let pclose = s[pstart..]
            .find("</w:p>")
            .map(|c| pstart + c + 6)
            .unwrap_or(s.len());
        out.push_str(&para_to_md_fmt(&s[pstart..pclose]));
        pos = pclose;
    }
    collapse_blank(out.trim())
}

/// One `<w:p…>…</w:p>` paragraph → one Markdown line.
fn para_to_md_fmt(block: &str) -> String {
    let level = heading_level_from_style(elem_attr(block, "w:pStyle", "w:val").as_deref());
    let in_list = block.contains("<w:numId") || block.contains("<w:numPr");
    // paragraph left indent (twips) → markdown list/indent levels
    let ind = elem_attr(block, "w:ind", "w:left")
        .and_then(|v| v.parse::<u32>().ok())
        .map(|twips| ((twips / 720).min(4)) as usize)
        .unwrap_or(0);
    // table cells / tabs → markdown markers (outside the <w:t> text)
    let block = block
        .replace("<w:tc>", "\u{1}")
        .replace("<w:tc ", "\u{1}")
        .replace("<w:tab/>", "    ");
    let text = para_runs_md(&block).replace('\u{1}', " | ");
    let mut line = String::new();
    if level > 0 {
        line.push_str(&"#".repeat(level));
        line.push(' ');
    }
    if in_list {
        line.push_str(&"  ".repeat(ind));
        line.push_str("- ");
    } else if ind > 0 {
        line.push_str(&"  ".repeat(ind));
    }
    line.push_str(&text);
    line.push('\n');
    line
}

/// The paragraph's text runs: `<w:t>` contents wrapped in `**…**` / `*…*`
/// per the nearest preceding `<w:rPr>` flags.
fn para_runs_md(block: &str) -> String {
    let mut out = String::new();
    let mut p = 0usize;
    while let Some(rel) = block[p..].find("<w:t") {
        let t0 = p + rel;
        // `<w:t` is a prefix of `<w:tab` / `<w:tc` / `<w:tbl>` — verify
        match block.as_bytes().get(t0 + 4) {
            Some(b'>') | Some(b' ') => {}
            _ => {
                p = t0 + 1;
                continue;
            }
        }
        let gt = match block[t0 + 4..].find('>') {
            Some(g) => g,
            None => break,
        };
        let inner_start = t0 + 4 + gt + 1;
        let close = match block[inner_start..].find("</w:t>") {
            Some(c) => inner_start + c,
            None => break,
        };
        let text = unescape(&block[inner_start..close]);
        let (bold, ital) = run_flags(block, t0);
        out.push_str(&wrap_md(&text, bold, ital));
        p = close + 6;
    }
    out
}

/// Bold / italic of a run: from the `<w:rPr>` INSIDE its own `<w:r…>` start
/// (an rPr belonging to a previous run must not leak into this one).
fn run_flags(block: &str, t0: usize) -> (bool, bool) {
    let start = t0.saturating_sub(400);
    let pre = &block[start..t0];
    // the current run started at the last `<w:r>` / `<w:r …>` before this <w:t>
    let run_rel = match (pre.rfind("<w:r>"), pre.rfind("<w:r ")) {
        (Some(a), Some(b)) => a.max(b),
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (None, None) => return (false, false), // run started far away — no flags
    };
    let seg = &pre[run_rel..];
    let pr_close = match seg.rfind("</w:rPr>") {
        Some(r) => r,
        None => return (false, false), // no <w:rPr> inside the current run
    };
    let pr_open = seg[..pr_close].rfind("<w:rPr>").map(|o| o).unwrap_or(0);
    let region = &seg[pr_open..pr_close + 8];
    let b = (region.contains("<w:b/>") || region.contains("<w:b "))
        && !region.contains("<w:b w:val=\"0\"")
        && !region.contains("<w:b w:val=\"false\"");
    let i = (region.contains("<w:i/>") || region.contains("<w:i "))
        && !region.contains("<w:i w:val=\"0\"")
        && !region.contains("<w:i w:val=\"false\"");
    (b, i)
}

/// Word built-in heading styles → markdown heading level (0 = not a heading).
fn heading_level_from_style(v: Option<&str>) -> usize {
    let Some(v) = v else {
        return 0;
    };
    let lv = v.to_ascii_lowercase();
    if let Some(rest) = lv.strip_prefix("heading") {
        return match rest.parse::<u32>() {
            Ok(n) if (1..=6).contains(&n) => n as usize,
            _ => 1,
        };
    }
    match lv.as_str() {
        "title" | "заголовок" | "heading" => 1,
        "subtitle" | "підзаголовок" => 2,
        _ => 0,
    }
}

/// Wrap a text run per its bold / italic state.
fn wrap_md(text: &str, bold: bool, ital: bool) -> String {
    match (bold, ital) {
        (true, true) => format!("***{text}***"),
        (true, false) => format!("**{text}**"),
        (false, true) => format!("*{text}*"),
        (false, false) => text.to_string(),
    }
}

/// The `key` attribute of the FIRST `<tag…>` element in `s` (or None).
fn elem_attr(s: &str, tag: &str, key: &str) -> Option<String> {
    let open = format!("<{tag}");
    let m = s.find(&open)?;
    let gt = s[m + open.len()..].find('>')?;
    let inner = &s[m + open.len()..m + open.len() + gt];
    let k = inner.find(key)?;
    // a real attribute: at the element start or preceded by a space
    let word_start = k == 0 || inner.as_bytes()[k - 1] == b' ';
    if !word_start {
        return None;
    }
    let rest = &inner[k + key.len()..];
    let rest = rest.strip_prefix("=")?;
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn xlsx_to_md(bytes: &[u8]) -> String {
    let shared = zip_entry(bytes, "xl/sharedStrings.xml")
        .map(|x| parse_shared_strings(&x))
        .unwrap_or_default();
    // find the first worksheet: sheet1.xml, else the first xl/worksheets/sheet*.xml
    let sheet_name = if zip_entry(bytes, "xl/worksheets/sheet1.xml").is_some() {
        Some("xl/worksheets/sheet1.xml".to_string())
    } else {
        zip_entries(bytes)
            .into_iter()
            .find(|(n, _)| n.starts_with("xl/worksheets/sheet") && n.ends_with(".xml"))
            .map(|(n, _)| n)
    };
    match sheet_name {
        Some(n) => match zip_entry(bytes, &n) {
            Some(sheet) => xlsx_sheet_to_md(&sheet, &shared),
            None => if shared.is_empty() {
                "XLSX: порожній файл або не знайдено листів.".into()
            } else {
                shared.join("\n")
            },
        },
        None => {
            if shared.is_empty() {
                "XLSX: порожній файл або не знайдено листів.".into()
            } else {
                shared.join("\n")
            }
        }
    }
}

/// All `<si>…</si>` strings in `xl/sharedStrings.xml`.
fn parse_shared_strings(xml: &[u8]) -> Vec<String> {
    let s = String::from_utf8_lossy(xml).to_string();
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(a) = s[i..].find("<si>") {
        let start = i + a + 4;
        if let Some(b) = s[start..].find("</si>") {
            let inner = &s[start..start + b];
            out.push(unescape(&strip_tags(inner)));
            i = start + b + 5;
        } else {
            break;
        }
    }
    out
}

/// One worksheet → a Markdown table (first row = header + separator).
fn xlsx_sheet_to_md(sheet: &[u8], shared: &[String]) -> String {
    let s = String::from_utf8_lossy(sheet).to_string();
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut pos = 0;
    while let Some(a) = s[pos..].find("<row") {
        // row block ends at the next </row> (or end)
        let block_start = pos + a;
        let close = s[block_start..].find("</row>").map(|k| block_start + k + 6);
        let block = match close {
            Some(e) => &s[block_start..e],
            None => &s[block_start..],
        };
        let mut cells: Vec<String> = Vec::new();
        let mut cpos = 0;
        while let Some(ca) = block[cpos..].find("<c ") {
            let cstart = cpos + ca;
            // a cell ends at the next <c or </row>/end
            let cend = block[cstart + 2..]
                .find("<c ")
                .map(|k| cstart + 2 + k)
                .or_else(|| block[cstart + 2..].find("</row>").map(|k| cstart + 2 + k))
                .unwrap_or(block.len());
            let cell = &block[cstart..cend];
            cells.push(resolve_cell(cell, shared));
            cpos = cend;
        }
        if !cells.is_empty() {
            rows.push(cells);
        }
        pos = close.unwrap_or(block_start + 4);
    }
    if rows.is_empty() {
        return "XLSX: на першому листі не знайдено клітинок.".into();
    }
    let ncols = rows.iter().map(|r| r.len()).max().unwrap_or(1);
    let mut out = String::new();
    for (i, row) in rows.iter().enumerate() {
        out.push('|');
        for c in 0..ncols {
            let v = row.get(c).map(|s| s.as_str()).unwrap_or("");
            out.push(' ');
            out.push_str(v);
            out.push_str(" |");
        }
        out.push('\n');
        if i == 0 {
            out.push('|');
            for _ in 0..ncols {
                out.push_str(" --- |");
            }
            out.push('\n');
        }
    }
    out
}

/// Resolve one `<c …>` cell to its text value.
fn resolve_cell(cell: &str, shared: &[String]) -> String {
    let typ = attr(cell, "t");
    if typ.as_deref() == Some("s") {
        if let Some(v) = tag_text(cell, "v") {
            if let Ok(idx) = v.parse::<usize>() {
                return shared.get(idx).cloned().unwrap_or_default();
            }
        }
        String::new()
    } else if typ.as_deref() == Some("inlineStr") {
        // <is><t>…</t></is>
        if let Some(is) = cell.find("<is>") {
            if let Some(t) = cell[is..].find("<t>") {
                if let Some(te) = cell[t..].find("</t>") {
                    return unescape(&cell[t + 3..te]);
                }
            }
        }
        String::new()
    } else if typ.as_deref() == Some("b") {
        tag_text(cell, "v").unwrap_or_default()
    } else {
        // number / formula result / default
        tag_text(cell, "v").unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// RTF
// ---------------------------------------------------------------------------

/// RTF table groups (`\fonttbl`, `\colortbl`, `\stylesheet`, …) whose contents
/// are formatting metadata, never document text — skipped whole.
const RTF_TABLE_GROUPS: &[&str] = &[
    "fonttbl", "colortbl", "stylesheet", "info", "header", "headerf", "footer",
    "footerf", "pntxtb", "pntxta", "latentstyle", "latentstyles", "themedata",
    // \info entry words (Word exports them as sibling groups)
    "author", "operator", "title", "subject", "docword", "docpages",
    "doccompany", "doctemplate", "docid",
];

/// Plain mode: strip RTF down to readable plain text.
fn rtf_to_md(bytes: &[u8]) -> String {
    rtf_convert(bytes, false)
}

/// Formatted mode: RTF → Markdown with the original formatting preserved
/// (headings `#…`, `**bold**`, `*italic*`, `_underline_`, lists).
fn rtf_to_md_fmt(bytes: &[u8]) -> String {
    rtf_convert(bytes, true)
}

/// Strip RTF down to readable text (best-effort, no external lib).
///
/// Handles: the ANSI codepage declared by `\ansicpgNNNN` (Ukrainian bases are
/// cp1251), `\'xx` codepage byte escapes, `\u` unicode escapes, `\*` comments,
/// and drops metadata groups (font/colour tables, style definitions, and
/// sibling `{\f2…}` / `{\s4…}` definition groups).
///
/// In `fmt` mode the original formatting is preserved as Markdown:
/// - paragraph styles "heading N" / "заголовок N" / "title" (and a big bold
///   font) → `#`..`######` headings,
/// - `\b` / `\i` / `\ul` → `**…**` / `*…*` / `_…_` spans,
/// - bullet characters (`•`, …) → `- …`, list indent from `\li`/`\fi`.
fn rtf_convert(bytes: &[u8], fmt: bool) -> String {
    let enc = rtf_encoding(bytes);
    let text = enc.decode(bytes).0.into_owned();
    let b: Vec<char> = text.chars().collect();
    let n = b.len();
    let mut out = String::with_capacity(text.len());
    // formatting state (fmt mode only)
    let mut bold = false;
    let mut italic = false;
    let mut under = false;
    let mut fs: u16 = 20; // font size, half-points
    let mut li: u32 = 0; // left indent, twentieths of a point
    let mut style_defs: Vec<(String, String)> = Vec::new(); // (\sN id, name)
    let mut style: Option<String> = None;
    let mut line_started = false; // a non-space char was emitted on this line
    let mut line_level = 0usize; // the current line's heading level
    let mut bold_opened = false; // '**' opened on this line (to close at \par)
    let mut italic_opened = false;
    let mut under_opened = false;
    let mut i = 0;
    // Emit a character with the line-start handling (heading prefix, list
    // indent, bullet, open formatting spans) — shared by the plain-char,
    // `\'xx` and `\u` paths.
    macro_rules! emit_char {
        ($c:expr) => {{
            let cc = $c;
            if !cc.is_whitespace() {
                let bullet =
                    fmt && !line_started && matches!(cc, '•' | '●' | '◦' | '▪' | '‣');
                if fmt && !line_started {
                    line_level = heading_level_rtf(style.as_deref(), bold, fs);
                    let ind = (li / 1080).min(3) as usize;
                    if line_level > 0 {
                        out.push_str(&"#".repeat(line_level));
                        out.push(' ');
                    }
                    out.push_str(&"  ".repeat(ind));
                    if !bullet {
                        if bold && line_level == 0 {
                            out.push_str("**");
                            bold_opened = true;
                        }
                        if italic {
                            out.push_str("*");
                            italic_opened = true;
                        }
                        if under {
                            out.push('_');
                            under_opened = true;
                        }
                    }
                }
                if bullet {
                    out.push_str("- ");
                } else {
                    out.push(cc);
                }
                line_started = true;
            } else {
                out.push(cc);
            }
        }};
    }
    while i < n {
        match b[i] {
            '\\' => {
                if i + 1 >= n {
                    i += 1;
                    continue;
                }
                match b[i + 1] {
                    '\'' => {
                        // \'xx — one byte in the declared codepage
                        if i + 3 < n
                            && b[i + 2].is_ascii_hexdigit()
                            && b[i + 3].is_ascii_hexdigit()
                        {
                            let hex: String = b[i + 2..i + 4].iter().collect();
                            if let Ok(val) = u8::from_str_radix(&hex, 16) {
                                emit_char!(enc
                                    .decode(&[val])
                                    .0
                                    .chars()
                                    .next()
                                    .unwrap_or('\u{FFFD}'));
                                i += 4;
                                continue;
                            }
                        }
                        i += 1;
                    }
                    '\\' => {
                        out.push('\\');
                        i += 2;
                    }
                    '{' | '}' => i += 2,
                    ' ' => {
                        out.push(' ');
                        i += 2;
                    }
                    '~' => {
                        out.push(' ');
                        i += 2;
                    }
                    '-' => {
                        out.push('-');
                        i += 2;
                    }
                    '*' => {
                        // \* — a comment runs to the next control word or the
                        // closing brace of the enclosing group
                        i += 2;
                        while i < n && b[i] != '\\' && b[i] != '}' {
                            i += 1;
                        }
                    }
                    _ => {
                        if b[i + 1].is_ascii_alphabetic() {
                            // \word + optional signed value
                            let mut j = i + 1;
                            while j < n && b[j].is_ascii_alphabetic() {
                                j += 1;
                            }
                            let word: String = b[i + 1..j].iter().collect();
                            let val_start = j;
                            if j < n && (b[j] == '-' || b[j].is_ascii_digit()) {
                                j += 1;
                                while j < n && b[j].is_ascii_digit() {
                                    j += 1;
                                }
                            }
                            let value: Option<String> = if j > val_start {
                                Some(b[val_start..j].iter().collect())
                            } else {
                                None
                            };
                            match word.as_str() {
                                "u" => {
                                    // \u<code> — unicode escape; an optional '?'
                                    // after the value is the fallback char
                                    let num: String = b[i + 2..j].iter().collect();
                                    if let Ok(code) = num.parse::<i32>() {
                                        if let Some(ch) = char::from_u32(code.unsigned_abs()) {
                                            emit_char!(ch);
                                        }
                                        if j < n && b[j] == '?' {
                                            j += 1; // eat the fallback
                                        }
                                    }
                                }
                                "par" | "parard" | "page" | "line" | "sect" => {
                                    if fmt {
                                        close_fmt_spans(
                                            &mut out,
                                            under_opened,
                                            italic_opened,
                                            bold_opened,
                                        );
                                    }
                                    out.push('\n');
                                    line_started = false;
                                    line_level = 0;
                                    bold_opened = false;
                                    italic_opened = false;
                                    under_opened = false;
                                }
                                "tab" => out.push('\t'),
                                "b" if fmt => {
                                    let on = value
                                        .as_deref()
                                        .map_or(true, |v| v != "0" && v != "false");
                                    if on && !bold {
                                        bold = true;
                                        // the span marker goes at the toggle
                                        // point — unless the line has not
                                        // started yet (then the line-start
                                        // logic places it, or suppresses it
                                        // on heading lines)
                                        if line_started && line_level == 0 {
                                            out.push_str("**");
                                            bold_opened = true;
                                        }
                                    } else if !on && bold {
                                        if bold_opened {
                                            out.push_str("**");
                                            bold_opened = false;
                                        }
                                        bold = false;
                                    }
                                }
                                "i" if fmt => {
                                    let on = value
                                        .as_deref()
                                        .map_or(true, |v| v != "0" && v != "false");
                                    if on && !italic {
                                        italic = true;
                                        if line_started {
                                            out.push_str("*");
                                            italic_opened = true;
                                        }
                                    } else if !on && italic {
                                        if italic_opened {
                                            out.push_str("*");
                                            italic_opened = false;
                                        }
                                        italic = false;
                                    }
                                }
                                "ul" if fmt => {
                                    let on = value
                                        .as_deref()
                                        .map_or(true, |v| v != "0" && v != "false");
                                    if on && !under {
                                        under = true;
                                        if line_started {
                                            out.push('_');
                                            under_opened = true;
                                        }
                                    } else if !on && under {
                                        if under_opened {
                                            out.push('_');
                                            under_opened = false;
                                        }
                                        under = false;
                                    }
                                }
                                "fs" => {
                                    if let Some(v) = &value {
                                        if let Ok(x) = v.parse::<u16>() {
                                            fs = x;
                                        }
                                    }
                                }
                                "li" | "fi" => {
                                    if let Some(v) = &value {
                                        if let Ok(x) = v.parse::<u32>() {
                                            li = li.max(x);
                                        }
                                    }
                                }
                                "s" => {
                                    // paragraph-style switch: remember the name
                                    // (heading detection in fmt mode)
                                    if let Some(id) = &value {
                                        style = style_defs
                                            .iter()
                                            .find(|(k, _)| k == id)
                                            .map(|(_, v)| v.clone());
                                    } else {
                                        style = None;
                                    }
                                }
                                _ => {}
                            }
                            // a control word is delimited by a single space
                            if j < n && b[j] == ' ' {
                                j += 1;
                            }
                            i = j;
                        } else {
                            i += 2;
                        }
                    }
                }
                continue;
            }
            '{' => {
                // a metadata/definition group → drop it whole; style
                // definitions (`{\sN … name;}`) are remembered by name
                if let Some(end) = def_group_end(&b, i) {
                    if let Some((id, name)) = style_def_info(&b, i, end) {
                        style_defs.push((id, name));
                    }
                    i = end;
                } else {
                    i += 1;
                }
                continue;
            }
            '}' => {
                i += 1;
                continue;
            }
            c => {
                emit_char!(c);
                i += 1;
            }
        }
    }
    if fmt {
        close_fmt_spans(&mut out, under_opened, italic_opened, bold_opened);
    }
    collapse_blank(out.trim())
}

/// Close the formatting spans opened on the current line (reverse open order).
fn close_fmt_spans(out: &mut String, under: bool, italic: bool, bold: bool) {
    if under {
        out.push('_');
    }
    if italic {
        out.push('*');
    }
    if bold {
        out.push_str("**");
    }
}

/// An RTF paragraph → a Markdown heading level: by the paragraph style name
/// ("heading 3", "заголовок 3", "title"…) or a big bold font size.
fn heading_level_rtf(style: Option<&str>, bold: bool, fs: u16) -> usize {
    if let Some(s) = style {
        let lv = s.trim().to_ascii_lowercase();
        for prefix in ["heading", "заголовок"] {
            if let Some(rest) = lv.strip_prefix(prefix) {
                let n: u32 = rest.trim().parse().unwrap_or(1);
                return n.clamp(1, 6) as usize;
            }
        }
        if matches!(lv.as_str(), "title" | "заголовок" | "heading") {
            return 1;
        }
        if matches!(lv.as_str(), "subtitle" | "підзаголовок") {
            return 2;
        }
    }
    // fallback: a big bold line looks like a heading (fs is in half-points)
    if bold {
        match fs {
            48.. => 1,
            40..=47 => 2,
            32..=39 => 3,
            28..=31 => 4,
            _ => 0,
        }
    } else {
        0
    }
}

/// For a style definition group `{\s4 … name;}` → (id, name); None for
/// non-style groups. The name is the last text before the group's trailing `;`.
fn style_def_info(b: &[char], i: usize, end: usize) -> Option<(String, String)> {
    // i points at '{', end just past the group's closing '}'
    let mut k = i + 1;
    if k + 1 < end && b[k] == '\\' && b[k + 1] == '*' {
        k += 2;
    }
    while k < end && matches!(b[k], '\r' | '\n' | ' ') {
        k += 1;
    }
    if k >= end || b[k] != '\\' {
        return None;
    }
    let mut w = k + 1;
    while w < end && b[w].is_ascii_alphabetic() {
        w += 1;
    }
    // only \sN (single-letter "s") style definitions
    if w == k + 1 || w >= end || b[w - 1] != 's' {
        return None;
    }
    let mut d = w;
    while d < end && b[d].is_ascii_digit() {
        d += 1;
    }
    if d == w {
        return None;
    }
    let id: String = b[w..d].iter().collect();
    if end < 2 || b[end - 1] != '}' || b[end - 2] != ';' {
        return Some((id, String::new()));
    }
    // the name: the last text run before the trailing ';'
    let zone = &b[i + 1..end - 2];
    let last_bs = zone.iter().rposition(|c| *c == '\\').unwrap_or(0);
    // skip the control word + its value + the one delimiter space
    let mut k2 = last_bs + 1;
    while k2 < zone.len() && zone[k2].is_ascii_alphabetic() {
        k2 += 1;
    }
    if k2 < zone.len() && (zone[k2] == '-' || zone[k2].is_ascii_digit()) {
        k2 += 1;
        while k2 < zone.len() && zone[k2].is_ascii_digit() {
            k2 += 1;
        }
    }
    if k2 < zone.len() && zone[k2] == ' ' {
        k2 += 1;
    }
    let name: String = zone[k2..].iter().collect();
    Some((id, name.trim().to_string()))
}

/// Index just past the closing `}` of the group starting at `b[i] == '{'`
/// (or None if unbalanced).
fn group_end(b: &[char], i: usize) -> Option<usize> {
    let mut d = 0i32;
    let mut j = i;
    while j < b.len() {
        match b[j] {
            '{' => d += 1,
            '}' => {
                d -= 1;
                j += 1;
                if d == 0 {
                    return Some(j);
                }
            }
            _ => {}
        }
        j += 1;
    }
    None
}

/// If the group at `b[i]` is formatting metadata rather than document text — a
/// table group (`{\fonttbl…}`, `{\*\stylesheet…}`) or a font/style *definition*
/// (`{\f2…;}`, `{\s4…;}`, which end with `;`) — return the index just past its
/// closing `}`.
fn def_group_end(b: &[char], i: usize) -> Option<usize> {
    let n = b.len();
    let mut k = i + 1;
    // optional leading comment marker \*
    let has_star = k + 1 < n && b[k] == '\\' && b[k + 1] == '*';
    if has_star {
        k += 2;
    }
    while k < n && matches!(b[k], '\r' | '\n' | ' ') {
        k += 1;
    }
    let mut table_word = false;
    let mut struct_word = false;
    if k < n && b[k] == '\\' {
        let wstart = k + 1;
        let mut wend = wstart;
        while wend < n && b[wend].is_ascii_alphabetic() {
            wend += 1;
        }
        let word: String = b[wstart..wend].iter().collect();
        table_word = RTF_TABLE_GROUPS.contains(&word.as_str());
        // font family / paragraph style / style-type DEFINITION markers
        struct_word =
            (word == "f" || word == "s" || word == "st")
                && b.get(wend).map_or(false, |c| c.is_ascii_digit() || *c == '-');
    }
    // a \*-prefixed group is a metadata keyword group (Word convention)
    if !table_word && !struct_word && !has_star {
        return None;
    }
    let end = group_end(b, i)?;
    // a definition group ends with a trailing ';'; a switch group
    // (`{\f157 text…}`) holds real content and is kept. (The ';' sits just
    // before the closing '}'.)
    if struct_word && b.get(end.saturating_sub(2)) != Some(&';') {
        return None;
    }
    Some(end)
}

/// The `Encoding` for the raw text bytes: the ANSI codepage declared by
/// `\ansicpgNNNN` (absent → Windows-1252, the RTF default).
fn rtf_encoding(bytes: &[u8]) -> &'static encoding_rs::Encoding {
    let cpg = rtf_ansicpg(bytes);
    match cpg {
        1250 => encoding_rs::WINDOWS_1250,
        1251 => encoding_rs::WINDOWS_1251,
        1252 => encoding_rs::WINDOWS_1252,
        1253 => encoding_rs::WINDOWS_1253,
        1254 => encoding_rs::WINDOWS_1254,
        1255 => encoding_rs::WINDOWS_1255,
        1256 => encoding_rs::WINDOWS_1256,
        1257 => encoding_rs::WINDOWS_1257,
        1258 => encoding_rs::WINDOWS_1258,
        850 => encoding_rs::ISO_8859_2,
        858 => encoding_rs::ISO_8859_15,
        1048 => encoding_rs::KOI8_R,
        _ => encoding_rs::WINDOWS_1252,
    }
}

/// The `\ansicpg` value in the RTF header (0 when absent).
fn rtf_ansicpg(bytes: &[u8]) -> u32 {
    let head = &bytes[..bytes.len().min(2048)];
    let Some(p) = find_bytes(head, br"\ansicpg") else {
        return 0;
    };
    let rest = &head[p + 8..];
    let mut val = 0u32;
    for d in rest.iter().take_while(|c| c.is_ascii_digit()) {
        val = val * 10 + u32::from(*d - b'0');
    }
    val
}

// ---------------------------------------------------------------------------
// PDF (best-effort)
// ---------------------------------------------------------------------------

/// Best-effort PDF text extraction: inflate each content stream and pull the
/// text shown by `Tj` / `TJ` / `'` / `"` operators. Not a full parser. Works on
/// the raw bytes (no lossy UTF-8 pass, which would corrupt binary streams).
fn pdf_to_md(bytes: &[u8]) -> String {
    let mut found = String::new();
    let mut i = 0;
    while i + 6 <= bytes.len() {
        let a = match find_bytes(&bytes[i..], b"stream") {
            Some(p) => i + p,
            None => break,
        };
        // skip the newline after "stream"
        let mut data_start = a + 6;
        if bytes.get(data_start) == Some(&b'\r') {
            data_start += 1;
        }
        if bytes.get(data_start) == Some(&b'\n') {
            data_start += 1;
        }
        match find_bytes(&bytes[data_start..], b"endstream") {
            Some(e) => {
                // strip the trailing CRLF that precedes "endstream"
                let mut end = data_start + e;
                if bytes.get(end.saturating_sub(1)) == Some(&b'\n') {
                    end -= 1;
                }
                if bytes.get(end.saturating_sub(1)) == Some(&b'\r') {
                    end -= 1;
                }
                let data = &bytes[data_start.min(end)..end];
                // the content stream is usually Flate-compressed; inflate_or_raw
                // falls back to the raw bytes when it is not
                found.push_str(&pdf_text_ops(&inflate_or_raw(data)));
                i = end + 9;
                if i <= a {
                    i = a + 1;
                }
            }
            None => break,
        }
    }
    let found = collapse_blank(found.trim());
    // quality gate: a scanned / vector-only PDF yields binary garbage from
    // image streams — require a real text ratio, otherwise just say so
    let chars = found.chars().collect::<Vec<_>>();
    let good = chars
        .iter()
        .filter(|c| c.is_ascii_alphanumeric() || c.is_ascii_whitespace() || ('\u{0400}'..='\u{04FF}').contains(c))
        .count();
    if chars.is_empty() || good * 100 < chars.len() * 55 {
        "PDF: текст не розпізнано (скан/векторна графіка або інше стискання). Відкрийте PDF-заглядачем.".into()
    } else {
        found
    }
}

/// Pull text-show strings from a (decompressed) PDF content stream.
fn pdf_text_ops(data: &[u8]) -> String {
    let mut out = String::new();
    let n = data.len();
    let mut i = 0;
    while i < n {
        let c = data[i];
        if c == b'(' {
            // a literal string (…) possibly part of a Tj / TJ array
            let (text, next) = pdf_paren_string(data, i);
            out.push_str(&text);
            out.push(' ');
            i = next;
        } else {
            i += 1;
        }
    }
    out
}

/// Read one PDF literal string starting at `data[i] == b'('` → (text, end_index).
fn pdf_paren_string(data: &[u8], i: usize) -> (String, usize) {
    let mut out = String::new();
    let mut depth = 1;
    let mut j = i + 1;
    let n = data.len();
    while j < n {
        let c = data[j];
        if c == b'\\' {
            // escape
            j += 1;
            if j < n {
                match data[j] {
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'(' => out.push('('),
                    b')' => out.push(')'),
                    b'\\' => out.push('\\'),
                    b'\n' => {} // line continuation
                    b'\r' => {
                        if j + 1 < n && data[j + 1] == b'\n' {
                            j += 1;
                        }
                    }
                    d if d.is_ascii_digit() => {
                        // octal escape (up to 3 digits)
                        let mut oct = String::new();
                        let mut k = j;
                        while k < n && data[k].is_ascii_digit() && oct.len() < 3 {
                            oct.push(data[k] as char);
                            k += 1;
                        }
                        if let Ok(v) = u32::from_str_radix(&oct, 8) {
                            out.push(char::from(v as u8));
                        }
                        j = k - 1;
                    }
                    _ => out.push(c as char),
                }
            }
            j += 1;
            continue;
        }
        if c == b'(' {
            depth += 1;
            out.push('(');
        } else if c == b')' {
            depth -= 1;
            if depth == 0 {
                return (out, j + 1);
            }
            out.push(')');
        } else {
            out.push(c as char);
        }
        j += 1;
    }
    (out, n)
}

// ---------------------------------------------------------------------------
// OLE2 (Word .doc / Excel .xls / templates / backups)
// ---------------------------------------------------------------------------

/// Extract the readable text runs (UTF-16LE + ASCII) from an OLE2 compound file.
/// Word stores the document text inside the WordDocument stream; a plain
/// string-run scan recovers the readable content for a preview.
fn ole2_strings(bytes: &[u8], ext: &str) -> String {
    let mut out = String::new();
    // UTF-16LE runs (Word 97–2003 text is predominantly UTF-16LE)
    let mut i = 0;
    let mut run = String::new();
    while i + 1 < bytes.len() {
        let unit = u16::from_le_bytes([bytes[i], bytes[i + 1]]);
        if is_text_unit(unit) {
            run.push(char::from_u32(unit as u32).unwrap_or('\u{FFFD}'));
            i += 2;
        } else {
            if run.chars().count() >= 6 {
                if let Some(t) = keep_ole2_line(run.trim()) {
                    out.push_str(t);
                    out.push('\n');
                }
            }
            run.clear();
            i += 2;
        }
    }
    if run.chars().count() >= 6 {
        if let Some(t) = keep_ole2_line(run.trim()) {
            out.push_str(t);
        }
    }
    let out = collapse_blank(out.trim());
    if out.is_empty() {
        format!("Бінарний файл .{ext} (OLE2) — читабельний текст не виділено.")
    } else {
        out
    }
}

/// Which extracted OLE2 string runs are real document text (versus BIFF
/// number-format strings like `#,##0"грн."` or font names like `Arial Cyr1`).
fn keep_ole2_line(t: &str) -> Option<&str> {
    if t.chars().count() < 6 {
        return None;
    }
    // number-format / font-table noise
    if t.contains('#') || t.contains('_') {
        return None;
    }
    let cyr = t.chars().filter(|c| ('\u{0400}'..='\u{04FF}').contains(c)).count();
    if cyr >= 2 {
        return Some(t);
    }
    let alpha = t.chars().filter(|c| c.is_ascii_alphanumeric()).count();
    if alpha >= 12 {
        return Some(t);
    }
    None
}

/// A UTF-16 unit that looks like document text (Cyrillic, Latin, digits, common
/// punctuation and whitespace).
fn is_text_unit(u: u16) -> bool {
    match u {
        0x0009 | 0x000A | 0x000D | 0x0020..=0x007E => true, // tab/newlines/ASCII printable
        0x0400..=0x04FF => true,                              // Cyrillic
        0x2010..=0x2027 => true,                               // common punctuation
        0x2030..=0x205E => true,
        0x00A0..=0x00FF => true,                               // Latin-1
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Images / misc
// ---------------------------------------------------------------------------

fn image_to_md(ext: &str, bytes: &[u8]) -> String {
    if bytes.len() <= 400_000 {
        let mime = match ext {
            "png" => "image/png",
            "gif" => "image/gif",
            _ => "image/jpeg",
        };
        format!("![{ext}] (data:{mime};base64,{}{})", mime, base64(bytes))
    } else {
        format!("Зображення .{ext} ({} Б) — занадто велике для вбудування.", bytes.len())
    }
}

fn generic_zip_or_note(ext: &str, bytes: &[u8]) -> String {
    // a bare .zip — list what's inside (entry names) rather than the payload
    let names: Vec<String> = zip_entries(bytes)
        .into_iter()
        .map(|(n, _)| n)
        .take(20)
        .collect();
    if names.is_empty() {
        format!("ZIP-архів .{ext} ({} Б).", bytes.len())
    } else {
        format!("ZIP-архів .{ext}:\n{}", names.join("\n"))
    }
}

/// Mostly-printable check without floats: at least 90% of the first 4 KiB are
/// printable ASCII / tab / newline.
fn generic(bytes: &[u8]) -> String {
    let denom = bytes.len().min(4096).max(1);
    let num = bytes
        .iter()
        .take(4096)
        .filter(|b| **b >= 0x20 || **b == b'\n' || **b == b'\t')
        .count();
    if num * 10 >= denom * 9 {
        String::from_utf8_lossy(bytes).to_string()
    } else {
        let head: String = bytes
            .iter()
            .take(8)
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ");
        format!("Бінарний файл ({} Б). Початок: `{head}`.", bytes.len())
    }
}

// ---------------------------------------------------------------------------
// zip (PK local file headers) + inflate
// ---------------------------------------------------------------------------

/// Scan the PK\x03\x04 local file headers (no central directory needed).
fn zip_entries(data: &[u8]) -> Vec<(String, u16)> {
    full_zip_entries(data)
        .into_iter()
        .map(|e| (e.name, e.method))
        .collect()
}

/// Decompressed payload of a named zip entry (deflate or stored).
fn zip_entry(data: &[u8], name: &str) -> Option<Vec<u8>> {
    let entries = full_zip_entries(data);
    let e = entries.iter().find(|e| e.name == name)?;
    let raw_start = e.data_off;
    let raw_end = if e.comp_len > 0 {
        raw_start + e.comp_len
    } else if e.data_desc {
        // data descriptor: the stream runs until the next local/central header
        let mut j = raw_start;
        while j + 4 <= data.len()
            && &data[j..j + 4] != b"PK\x03\x04"
            && &data[j..j + 4] != b"PK\x01\x02"
        {
            j += 1;
        }
        j
    } else {
        raw_start
    };
    let raw = &data[raw_start..raw_end.min(data.len())];
    if e.method == 8 {
        Some(inflate_or_raw(raw))
    } else if e.method == 0 {
        Some(raw.to_vec())
    } else {
        None
    }
}

struct FullEntry {
    name: String,
    method: u16,
    data_off: usize,
    comp_len: usize,
    data_desc: bool,
}

fn full_zip_entries(data: &[u8]) -> Vec<FullEntry> {
    let mut v: Vec<FullEntry> = Vec::new();
    let mut i = 0;
    while i + 30 <= data.len() {
        if &data[i..i + 4] == b"PK\x03\x04" {
            let method = u16::from_le_bytes([data[i + 8], data[i + 9]]);
            let flags = u16::from_le_bytes([data[i + 6], data[i + 7]]);
            let comp_len = u32::from_le_bytes([
                data[i + 18],
                data[i + 19],
                data[i + 20],
                data[i + 21],
            ]) as usize;
            let name_len = u16::from_le_bytes([data[i + 26], data[i + 27]]) as usize;
            let extra_len = u16::from_le_bytes([data[i + 28], data[i + 29]]) as usize;
            let name_start = i + 30;
            if name_start + name_len <= data.len() {
                let name = String::from_utf8_lossy(&data[name_start..name_start + name_len])
                    .to_string();
                let data_off = name_start + name_len + extra_len;
                v.push(FullEntry {
                    name,
                    method,
                    data_off,
                    comp_len,
                    data_desc: flags & 0x8 != 0,
                });
                i = if comp_len > 0 { data_off + comp_len } else { data_off + 1 };
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    v
}

/// Inflate a raw DEFLATE (RFC1951) stream; if that fails, return the raw bytes.
/// Always succeeds (falls back to the input), so callers get a plain `Vec<u8>`.
fn inflate_or_raw(data: &[u8]) -> Vec<u8> {
    if data.is_empty() {
        return Vec::new();
    }
    // a zip DEFLATE entry is raw (no zlib/gzip wrapper)
    let mut dec = flate2::read::DeflateDecoder::new(data);
    let mut out = Vec::with_capacity(data.len() * 2);
    if dec.read_to_end(&mut out).is_ok() && !out.is_empty() {
        return out;
    }
    // not deflate (or stored) — treat as stored
    data.to_vec()
}

// ---------------------------------------------------------------------------
// small string helpers
// ---------------------------------------------------------------------------

fn replace_all(s: &str, from: &str, to: &str) -> String {
    s.replace(from, to)
}

/// Drop every `<…>` tag, keep the inner text. Byte-precise: multi-byte UTF-8
/// text runs survive intact (tags are ASCII, so byte-level tracking is safe).
fn strip_tags(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(s.len());
    let mut in_tag = false;
    for &c in b {
        if in_tag {
            if c == b'>' {
                in_tag = false;
            }
        } else if c == b'<' {
            in_tag = true;
        } else {
            out.push(c);
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

/// Collapse 3+ consecutive newlines to two (blank-line runs → one blank line).
fn collapse_blank(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut blanks = 0usize;
    for c in s.chars() {
        if c == '\n' {
            blanks += 1;
            if blanks <= 2 {
                out.push('\n');
            }
        } else {
            blanks = 0;
            out.push(c);
        }
    }
    out
}

/// `find_bytes` — index of the first occurrence of `needle` in `hay` (or None).
fn find_bytes(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > hay.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

/// `attr` — read a double-quoted XML attribute from a tag's opening text.
fn attr(tag: &str, key: &str) -> Option<String> {
    let pat = format!(" {key}=\"");
    let a = tag.find(&pat)?;
    let rest = &tag[a + pat.len()..];
    let b = rest.find('"')?;
    Some(rest[..b].to_string())
}

/// `tag_text` — inner text of a `<v>…</v>` (or generic) element inside `s`.
fn tag_text(s: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let a = s.find(&open)? + open.len();
    let sub = &s[a..];
    let b = sub.find(&close)?;
    Some(sub[..b].to_string())
}

// ---- base64 (standard, no external crate) --------------------------------

const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len() * 4 / 3 + 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(B64[(n >> 6) as usize & 63] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(B64[(n) as usize & 63] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_known_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn rtf_strips_control_words() {
        let rtf = br"{\rtf1\ansi\deff0 {\fonttbl} {\colortbl} \pard Hello\par World\tab end}";
        let t = rtf_to_md(rtf);
        assert!(t.contains("Hello"), "{t}");
        assert!(t.contains("World"), "{t}");
        assert!(t.contains('\t'), "{t}");
        assert!(!t.contains("\\rtf"), "{t}");
    }

    #[test]
    fn strip_tags_and_unescape() {
        assert_eq!(strip_tags("<a>1<b>2</b></a>"), "12");
        assert_eq!(unescape("&amp;&lt;"), "&<");
    }

    #[test]
    fn pdf_paren_string_escapes() {
        let data = br"(Hi \) there) Tj (line\nbreak)";
        let (t, _) = pdf_paren_string(data, 0);
        assert_eq!(t, "Hi ) there");
    }

    #[test]
    fn real_docx_sample_from_base() {
        // if a decoded docx sample exists in the probe folder, convert it
        let p = std::path::Path::new(r"C:\temp\opencode\dkrs_extract\x_docx\docx");
        if !p.exists() {
            return;
        }
        let b = std::fs::read(p).unwrap();
        let md = to_markdown("docx", &b);
        assert!(!md.is_empty());
    }

    /// A minimal STORED (method 0) zip — enough for the in-memory docx tests
    /// (no deflation needed).
    fn stored_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        for (name, data) in entries {
            out.extend_from_slice(b"PK\x03\x04");
            out.extend_from_slice(&20u16.to_le_bytes()); // version needed
            out.extend_from_slice(&0u16.to_le_bytes()); // flags
            out.extend_from_slice(&0u16.to_le_bytes()); // method 0 = stored
            out.extend_from_slice(&0u16.to_le_bytes()); // mod time
            out.extend_from_slice(&0u16.to_le_bytes()); // mod date
            out.extend_from_slice(&0u32.to_le_bytes()); // crc32
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes()); // extra len
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(data);
        }
        out
    }

    #[test]
    fn docx_fmt_keeps_headings_bold_lists() {
        let xml = br#"<w:document><w:body>
            <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Title</w:t></w:r></w:p>
            <w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Bold</w:t></w:r><w:r><w:t> plain</w:t></w:r></w:p>
            <w:p><w:pPr><w:numPr><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>item</w:t></w:r></w:p>
        </w:body></w:document>"#;
        let zip = stored_zip(&[("word/document.xml", xml)]);
        let md = to_markdown_fmt("docx", &zip);
        assert!(md.contains("# Title"), "{md}");
        assert!(md.contains("**Bold** plain"), "{md}");
        assert!(md.contains("- item"), "{md}");
    }

    #[test]
    fn rtf_fmt_keeps_headings_and_spans() {
        // \'95 = cp1252 bullet (default codepage when \ansicpg is absent)
        let rtf = br"{\rtf1\ansi\deff0{\s1\b\fs48 heading 1;}{\pard\b\fs48 Big Bold Title\par\b0\fs24 normal text\par\i some italics\i0\par\li1080 \'95 bullet item\par}}";
        let md = rtf_to_md_fmt(rtf);
        // the big bold line becomes a level-1 heading (no stray ** on it)
        assert!(md.contains("# Big Bold Title"), "{md}");
        assert!(!md.contains("**#"), "{md}");
        assert!(md.contains("normal text"), "{md}");
        assert!(md.contains("*some italics*"), "{md}");
        assert!(
            md.lines().any(|l| l.trim_start().starts_with("- ") && l.contains("bullet item")),
            "{md}"
        );
    }

    #[test]
    fn rtf_fmt_style_name_headings() {
        let rtf = br"{\rtf1\ansi\deff0{\s4\qc\fs44 heading 4;}{\pard\s4 Section four text\par}}";
        let md = rtf_to_md_fmt(rtf);
        assert!(md.contains("#### Section four text"), "{md}");
    }
}
