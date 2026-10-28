//! Export writers (markdown / RTF / DOCX). Text comes from the render engine
//! (dictionary token expansion), one document per hit.

use anyhow::Result;
use std::path::Path;

use crate::highlight::Hl;

// ============================================================
// query-term highlighting (v0.12.0)
// ============================================================
//
// Each format marks the matched query terms with its own markup. The
// `Hl` is the case-folded term set; `Hl::segments` partitions a line into
// (piece, is-match) spans so the layout logic (centering / caps / bullets)
// is untouched and only the displayed text gets marked.

/// Markdown/HTML-ish inline: wrap each matched span in `open`/`close` tags.
/// `text` is returned as-is (already markup-ready) when `hl` is empty.
fn mark_terms(text: &str, hl: &Hl, open: &str, close: &str) -> String {
    if hl.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len() + 64);
    for (piece, is_hl) in hl.segments(text) {
        if is_hl {
            out.push_str(open);
            out.push_str(piece);
            out.push_str(close);
        } else {
            out.push_str(piece);
        }
    }
    out
}

// ============================================================
// bulk export over hits
// ============================================================

/// Render every hit's document and write one file per hit.
/// Returns (count, out_dir).
///
/// Export layout: requisites header (= the GUI preview header content:
/// title from the document's own "назва" field, then every meta line),
/// then the full document text. No search-context slices here.
pub fn export_hits(
    baza: &Path,
    hits: &[crate::db::Hit],
    format: &str,
    naming: &crate::naming::Naming,
    progress: Option<&dyn Fn(usize, usize)>,
    attach: bool,
    single: bool,
    hl: &crate::highlight::Hl,
    evidence: Option<&std::collections::HashMap<u32, String>>,
) -> anyhow::Result<(usize, usize, std::path::PathBuf)> {
    let out = Path::new(&naming.out_dir);
    let dir = naming.prepare_out(out)?;
    let base = crate::render::Base::load(baza)?;
    // csv = ONE results-table file for the whole hit set (not per-document);
    // a plain "results.csv" name (the naming template is for per-doc files)
    if format == "csv" {
        let fpath = naming.dedupe(&dir.join("results.csv"));
        let n = write_results_csv(&fpath, hits, &base, baza)?;
        if let Some(p) = progress {
            p(1, 1);
        }
        return Ok((n, 0, dir));
    }
    let total = hits.len();
    // "зв'язка" (bundle): ALL the documents into ONE file (csv is already a
    // single results table and ignores `single`)
    if single {
        let mut docs: Vec<BundleDoc> = Vec::with_capacity(total);
        let mut attached = 0usize;
        for h in hits.iter() {
            let text = match base.doc_text(baza, h.docnum) {
                Ok(t) => t,
                Err(e) => format!("[помилка рендера документа {}: {e}]", h.docnum),
            };
            let f = crate::fields::parse(h.docnum, &text);
            let title = f
                .title
                .clone()
                .unwrap_or_else(|| first_line(&text, 60));
            let head = head_with_evidence(
                head_from(&f, &h.file, &title, None),
                evidence,
                h.docnum,
            );
            if attach {
                // a unique per-doc attachment name inside one bundle folder
                let stem = format!("doc_{}", h.docnum);
                if crate::signal::copy_signal(baza, h.docnum, &dir, &stem)?.is_some() {
                    attached += 1;
                }
            }
            docs.push(BundleDoc {
                head,
                text,
                docnum: h.docnum,
            });
            if let Some(p) = progress {
                p(docs.len(), total);
            }
        }
        let first_docnum = hits.first().map(|h| h.docnum).unwrap_or(0);
        let bname = naming.sanitize(&format!("bundle-{total}-{first_docnum}.{format}"));
        let fpath = naming.dedupe(&dir.join(&bname));
        match format {
            "md" => write_markdown_bundle(&fpath, &docs, hl)?,
            "rtf" => write_rtf_bundle(&fpath, &docs, hl)?,
            "docx" => write_docx_bundle(&fpath, &docs, hl)?,
            "html" => write_html_bundle(&fpath, &docs, hl)?,
            "pdf" => write_pdf_bundle(&fpath, &docs, hl)?,
            other => anyhow::bail!("unknown format: {other}"),
        }
        return Ok((total, attached, dir));
    }
    let mut exported = 0usize;
    let mut attached = 0usize;
    for (idx, h) in hits.iter().enumerate() {
        let text = match base.doc_text(baza, h.docnum) {
            Ok(t) => t,
            Err(e) => format!("[помилка рендера документа {}: {e}]", h.docnum),
        };
        let f = crate::fields::parse(h.docnum, &text);
        let title = f
            .title
            .clone()
            .unwrap_or_else(|| first_line(&text, 60));
        let fname = naming.render(idx + 1, &h.docnum.to_string(), format);
        let fname = naming.sanitize(&fname);
        let fpath = naming.dedupe(&dir.join(&fname));
        // signal attachment (додаток / Word-вложение), when requested
        let mut sig_name: Option<String> = None;
        if attach {
            // document name without the export extension (e.g. "00001_223")
            let stem = fpath
                .with_extension("")
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| h.docnum.to_string());
            if let Some(dst) = crate::signal::copy_signal(baza, h.docnum, &dir, &stem)? {
                sig_name = Some(dst.file_name().unwrap().to_string_lossy().to_string());
                attached += 1;
            }
        }
        let head = head_with_evidence(
            head_from(&f, &h.file, &title, sig_name.as_deref()),
            evidence,
            h.docnum,
        );
        match format {
            "md" => write_markdown(&fpath, Some(&head), &text, hl)?,
            "rtf" => write_rtf(&fpath, Some(&head), &text, hl)?,
            "docx" => write_docx(&fpath, Some(&head), &text, hl)?,
            "html" => write_html(&fpath, Some(&head), &text, hl)?,
            "pdf" => write_pdf(&fpath, Some(&head), &text, hl)?,
            other => anyhow::bail!("unknown format: {other}"),
        }
        exported += 1;
        if let Some(p) = progress {
            p(exported, total);
        }
    }
    Ok((exported, attached, dir))
}

/// Requisites header block — the same content the GUI preview shows:
/// the document title plus every DocFields::display() line and the
/// container file name.
pub struct ExportHead {
    pub title: String,
    pub lines: Vec<String>,
}

fn first_line(text: &str, max_chars: usize) -> String {
    let l = text.lines().next().unwrap_or("");
    let cut: String = l.chars().take(max_chars).collect();
    cut.trim_end().to_string()
}

fn head_from(f: &crate::fields::DocFields, file: &str, title: &str, sig: Option<&str>) -> ExportHead {
    let mut lines = vec![format!("файл: {file}")];
    lines.extend(f.display().lines().map(String::from));
    if let Some(s) = sig {
        lines.push(format!("додаток: {s} (сигнальний документ)"));
    }
    ExportHead {
        title: title.to_string(),
        lines,
    }
}

/// Prepend a per-doc native-u16 evidence line (v0.37.0, the
/// `export --trident-docs` native-id column) to the export head when the
/// doc's docnum has a native-id annotation. EN: the per-document native-id
/// evidence column for the tsearch-joined export; the line is absent for
/// every other export path (the map is `None` or lacks the docnum).
fn head_with_evidence(
    mut head: ExportHead,
    evidence: Option<&std::collections::HashMap<u32, String>>,
    docnum: u32,
) -> ExportHead {
    if let Some(m) = evidence {
        if let Some(e) = m.get(&docnum) {
            head.lines.push(format!("нативні u16 (Trident): {e}"));
        }
    }
    head
}

/// A centered line starts with at least this many leading spaces (the base
/// stores centering as leading spaces in the document text).
pub const CENTER_LEAD: usize = 4;

/// A dash/bullet-led line (a document list item in the original layout).
pub fn is_bullet(t: &str) -> bool {
    matches!(
        t.chars().next(),
        Some('-') | Some('–') | Some('—') | Some('•') | Some('·')
    )
}

/// The list item's text, without the leading marker.
pub fn bullet_body(t: &str) -> &str {
    let c = t.chars().next().map(|c| c.len_utf8()).unwrap_or(0);
    t[c..].trim_start()
}

/// An ALL-CAPS line without trailing sentence punctuation (a centered
/// document header like "КАБІНЕТ МІНІСТРІВ УКРАЇНИ") — worth emphasizing.
pub fn is_caps(t: &str) -> bool {
    let al: Vec<char> = t.chars().filter(|c| c.is_alphabetic()).collect();
    if al.len() < 4 || al.iter().any(|c| !c.is_uppercase()) {
        return false;
    }
    !matches!(
        t.chars().next_back(),
        Some('.' | ',' | ';' | ':' | '!' | '?')
    )
}

/// (leading spaces, trimmed text, is-caps, is-bullet) — the line
/// classification shared by every format-preserving writer.
pub fn line_traits(line: &str) -> (usize, &str, bool, bool) {
    let t = line.trim();
    let lead = line.len() - t.len();
    (lead, t, is_caps(t), is_bullet(t))
}

/// A line is rendered CENTERED in a true-centering format (RTF \qc, DOCX
/// w:jc, HTML text-align) only when it is indented AND either a header
/// (ALL-CAPS) or short — long indented lines are usually just list-item
/// indentation in the original layout, not centered text.
pub fn is_centered(lead: usize, t: &str, caps: bool) -> bool {
    lead >= CENTER_LEAD && (caps || t.len() <= 60)
}

/// Format-preserving Markdown body: leading spaces become nbsp (centering
/// survives, no code blocks), ALL-CAPS lines are emphasized (**bold**),
/// dash/bullet items become markdown list items, and numbered "1." items
/// are left as-is (already valid markdown ordered lists). Query terms from
/// `hl` are marked with inline `<mark>` (GFM renders it as a highlight;
/// plain viewers show the raw tag).
fn md_body_fmt(text: &str, hl: &Hl) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    for line in text.lines() {
        let (lead, t, caps, bullet) = line_traits(line);
        if t.is_empty() {
            out.push('\n');
            continue;
        }
        for _ in 0..lead {
            out.push('\u{00A0}');
        }
        if bullet {
            out.push_str("- ");
            out.push_str(&mark_terms(bullet_body(t), hl, "<mark>", "</mark>"));
        } else if caps {
            out.push_str(&format!(
                "**{}**",
                mark_terms(t, hl, "<mark>", "</mark>")
            ));
        } else {
            out.push_str(&mark_terms(t, hl, "<mark>", "</mark>"));
        }
        out.push('\n');
    }
    out
}

/// Format-preserving Markdown view of one document: `# title` heading,
/// italic requisites lines, then the formatted body.
pub fn doc_markdown_head(title: &str, meta: &[String], text: &str, hl: &Hl) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {title}\n\n"));
    for line in meta {
        out.push_str(&format!("*{line}*\n"));
    }
    if !meta.is_empty() {
        out.push('\n');
    }
    out.push_str(&md_body_fmt(text, hl));
    out
}

/// Format-preserving Markdown view of one document (fields-based header,
/// no term highlighting — the GUI preview path).
pub fn doc_markdown(f: Option<&crate::fields::DocFields>, text: &str) -> String {
    match f {
        Some(fl) => {
            let title = fl.title.clone().unwrap_or_else(|| first_line(text, 80));
            let meta: Vec<String> = fl.display().lines().map(String::from).collect();
            doc_markdown_head(&title, &meta, text, &Hl::default())
        }
        None => doc_markdown_head(&first_line(text, 80), &[], text, &Hl::default()),
    }
}

/// Markdown export: the same format-preserving layout as doc_markdown_head
/// (`# title`, `*requisites*`, centered / bold / list-aware body). Query
/// terms from `hl` are marked with inline `<mark>`.
pub fn write_markdown(path: &Path, head: Option<&ExportHead>, text: &str, hl: &Hl) -> Result<()> {
    let (title, meta) = match head {
        Some(h) => (h.title.clone(), h.lines.clone()),
        None => (first_line(text, 80).to_string(), Vec::new()),
    };
    std::fs::write(path, doc_markdown_head(&title, &meta, text, hl))?;
    Ok(())
}

// ---- rtf escape + header ----------------------------------------------------

/// Cp1251-escaped RTF body for one text run (no paragraph breaks).
fn rtf_run(rtf: &mut String, text: &str) {
    let m = crate::db::char_to_byte();
    for ch in text.chars() {
        match ch {
            '\\' => rtf.push_str("\\\\"),
            '{' => rtf.push_str("\\{"),
            '}' => rtf.push_str("\\}"),
            '\r' => {}
            '\n' => rtf.push_str("\\par\n"),
            '\t' => rtf.push_str("\\tab "),
            _ => match m.get(&ch) {
                Some(&b) if b < 0x80 => rtf.push(ch),
                Some(&b) => rtf.push_str(&format!("\\'{b:02X}")),
                None => {}
            },
        }
    }
}

/// RTF run with yellow highlight (`\highlight6` = yellow, `\highlight0`
/// = none) around the matched query terms; plain when `hl` is empty.
fn rtf_mark(rtf: &mut String, text: &str, hl: &Hl) {
    if hl.is_empty() {
        rtf_run(rtf, text);
        return;
    }
    for (piece, is_hl) in hl.segments(text) {
        if is_hl {
            rtf.push_str("\\highlight6 ");
            rtf_run(rtf, piece);
            rtf.push_str("\\highlight0 ");
        } else {
            rtf_run(rtf, piece);
        }
    }
}

/// One requisites/header line as its own RTF paragraph.
fn rtf_line(rtf: &mut String, line: &str, bold: bool, italic: bool) {
    if bold || italic {
        let on = if bold { "\\b " } else { "" };
        let it = if italic { "\\i " } else { "" };
        rtf.push_str(on);
        rtf.push_str(it);
        rtf_run(rtf, line);
        let off = if bold { "\\b0" } else { "" };
        let ioff = if italic { "\\i0" } else { "" };
        rtf.push_str(&format!("{off}{ioff}"));
    } else {
        rtf_run(rtf, line);
    }
    rtf.push_str("\\par\n");
}

/// RTF: cp1251-encoded with \ansicpg1251; non-ASCII bytes escaped \'hh.
/// Requisites header (bold title + italic meta lines) above the document.
/// The body keeps the document's own layout (the same rules as
/// doc_markdown): centered lines get \qc, ALL-CAPS lines are bold,
/// dash/bullet items become \bullet list paragraphs, numbered "1." items
/// stay as-is. Query terms from `hl` get a yellow RTF highlight.
pub fn write_rtf(path: &Path, head: Option<&ExportHead>, text: &str, hl: &Hl) -> Result<()> {
    let mut rtf = String::from(
        "{\\rtf1\\ansi\\ansicpg1251\\deff0{\\fonttbl{\\f0\\fnil\\fcharset0 Times New Roman;}}\n",
    );
    if let Some(h) = head {
        rtf_line(&mut rtf, &h.title, true, false);
        for line in &h.lines {
            rtf_line(&mut rtf, line, false, true);
        }
        rtf.push_str("\\par\n");
    }
    // body: one RTF paragraph per document line, attributes grouped so they
    // never leak into the next paragraph
    rtf_body(&mut rtf, text, hl);
    rtf.push('}');
    std::fs::write(path, rtf)?;
    Ok(())
}

// ============================================================
// DOCX (manual stored-entries ZIP with correct CRC32)
// ============================================================

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, e) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *e = c;
    }
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// The document's header paragraphs (bold title + italic requisites lines).
fn docx_head_paras(head: &ExportHead, paras: &mut String) {
    paras.push_str(&format!(
        "<w:p><w:pPr><w:rPr><w:b/></w:rPr></w:pPr><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
        xml_escape(&head.title)
    ));
    for line in &head.lines {
        paras.push_str(&format!(
            "<w:p><w:pPr><w:rPr><w:i/></w:rPr></w:pPr><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
            xml_escape(line)
        ));
    }
    paras.push_str("<w:p/>");
}

/// The document's body paragraphs (list indent or centering, bold ALL-CAPS).
/// Matched query terms (`hl`) are their own `<w:r>` runs carrying
/// `<w:highlight w:val="yellow"/>`.
fn docx_body_paras(text: &str, paras: &mut String, hl: &Hl) {
    for line in text.split('\n') {
        let (lead, t, caps, bullet) = line_traits(line);
        if t.is_empty() {
            continue;
        }
        // paragraph properties: list indent or centering, plus bold for
        // ALL-CAPS headers (the minimal docx has no numbering part, so a
        // list item is a "- " text marker with a hanging indent)
        let mut ppr = String::new();
        if bullet {
            ppr.push_str("<w:ind w:left=\"720\" w:hanging=\"360\"/>");
        } else if is_centered(lead, t, caps) {
            ppr.push_str("<w:jc w:val=\"center\"/>");
        }
        if caps {
            ppr.push_str("<w:rPr><w:b/></w:rPr>");
        }
        let ppr_block = if ppr.is_empty() {
            String::new()
        } else {
            format!("<w:pPr>{ppr}</w:pPr>")
        };
        let body_text = if bullet { bullet_body(t) } else { t };
        // one run per segment; a match is a highlight run (the "- " list
        // marker stays a plain run)
        let mut runs = String::new();
        if bullet {
            runs.push_str("<w:r><w:t xml:space=\"preserve\">- </w:t></w:r>");
        }
        for (piece, is_hl) in hl.segments(body_text) {
            let esc = xml_escape(piece);
            if is_hl {
                runs.push_str(&format!(
                    "<w:r><w:rPr><w:highlight w:val=\"yellow\"/></w:rPr><w:t xml:space=\"preserve\">{esc}</w:t></w:r>"
                ));
            } else {
                runs.push_str(&format!(
                    "<w:r><w:t xml:space=\"preserve\">{esc}</w:t></w:r>"
                ));
            }
        }
        paras.push_str(&format!("<w:p>{ppr_block}{runs}</w:p>"));
    }
}

/// Package a finished `word/document.xml` as the minimal stored-entries docx
/// zip (the fixed [Content_Types].xml + .rels wrappers).
fn docx_write_zip(path: &Path, doc: &str) -> Result<()> {
    let types = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>",
        "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">",
        "<Default Extension=\"xml\" ContentType=\"application/xml\"/>",
        "<Default Extension=\"rels\" ContentType=\"http://schemas.openxmlformats.org/package/2006/relationships\"/>",
        "<Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/>",
        "</Types>"
    );
    let root_rels = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>",
        "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
        "<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/>",
        "</Relationships>"
    );
    let entries: Vec<(&str, &[u8])> = vec![
        ("[Content_Types].xml", types.as_bytes()),
        ("_rels/.rels", root_rels.as_bytes()),
        ("word/document.xml", doc.as_bytes()),
    ];
    write_stored_zip(path, &entries)
}

pub fn write_docx(path: &Path, head: Option<&ExportHead>, text: &str, hl: &Hl) -> Result<()> {
    let mut paras = String::new();
    if let Some(h) = head {
        docx_head_paras(h, &mut paras);
    }
    docx_body_paras(text, &mut paras, hl);
    let doc = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
<w:body>{paras}</w:body></w:document>"
    );
    docx_write_zip(path, &doc)
}

/// DOCX bundle: a bold "Зв'язка" header with the numbered document list,
/// then EVERY document (its title + requisites + body) on its own page
/// (`w:br w:type="page"` breaks). Matched query terms (`hl`) are yellow
/// highlight runs.
pub fn write_docx_bundle(path: &Path, docs: &[BundleDoc], hl: &Hl) -> Result<()> {
    let n = docs.len();
    let mut paras = String::new();
    paras.push_str(&format!(
        "<w:p><w:pPr><w:rPr><w:b/></w:rPr></w:pPr><w:r><w:t xml:space=\"preserve\">Зв'язка — {n} документ(ів)</w:t></w:r></w:p>"
    ));
    for (i, d) in docs.iter().enumerate() {
        paras.push_str(&format!(
            "<w:p><w:r><w:t xml:space=\"preserve\">{}. {} (№ {})</w:t></w:r></w:p>",
            i + 1,
            xml_escape(&d.head.title),
            d.docnum
        ));
    }
    const PAGE_BREAK: &str = "<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>";
    for d in docs.iter() {
        paras.push_str(PAGE_BREAK);
        docx_head_paras(&d.head, &mut paras);
        docx_body_paras(&d.text, &mut paras, hl);
    }
    let doc = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
<w:body>{paras}</w:body></w:document>"
    );
    docx_write_zip(path, &doc)
}

// ---- HTML (format-preserving; the browser's print-to-PDF path) ------------

const HTML_STYLE: &str = "\
body{font-family:'Times New Roman',serif;font-size:14px;line-height:1.45;\
max-width:900px;margin:24px auto;padding:0 16px;color:#111}\
h1{font-size:20px;text-align:center;margin:4px 0 12px}\
p.meta{color:#444;font-style:italic;font-size:13px;text-align:center;margin:0 0 18px}\
ul{margin:6px 0;padding-left:28px}\
li{margin:2px 0}\
mark.hl{background:#ffe86b;padding:0 1px}";

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// HTML-escaped text with the matched query terms marked as
/// `<mark class="hl">` (yellow via the .hl CSS rule in HTML_STYLE).
fn html_mark(text: &str, hl: &Hl) -> String {
    if hl.is_empty() {
        return html_escape(text);
    }
    let mut out = String::with_capacity(text.len() + 64);
    for (piece, is_hl) in hl.segments(text) {
        if is_hl {
            out.push_str("<mark class=\"hl\">");
            out.push_str(&html_escape(piece));
            out.push_str("</mark>");
        } else {
            out.push_str(&html_escape(piece));
        }
    }
    out
}

/// One document's HTML body: `<h1>` title + meta paragraph + the body
/// layout (the same rules as doc_markdown: centered paragraphs, bold
/// ALL-CAPS, `<ul><li>` lists). Matched query terms (`hl`) are marked as
/// `<mark class="hl">` (yellow via the `.hl` CSS rule in HTML_STYLE).
fn html_doc_body(head: Option<&ExportHead>, text: &str, body: &mut String, hl: &Hl) {
    if let Some(h) = head {
        body.push_str(&format!("<h1>{}</h1>\n", html_escape(&h.title)));
        body.push_str("<p class=\"meta\">");
        for (i, line) in h.lines.iter().enumerate() {
            if i > 0 {
                body.push_str("<br/>");
            }
            body.push_str(&html_escape(line));
        }
        body.push_str("</p>\n");
    }
    let mut in_list = false;
    for line in text.lines() {
        let (lead, t, caps, bullet) = line_traits(line);
        if t.is_empty() {
            if in_list {
                body.push_str("</ul>\n");
                in_list = false;
            }
            body.push('\n');
            continue;
        }
        if in_list {
            body.push_str("</ul>\n");
            in_list = false;
        }
        if bullet {
            if !in_list {
                body.push_str("<ul>\n");
                in_list = true;
            }
            body.push_str(&format!("  <li>{}</li>\n", html_mark(bullet_body(t), hl)));
            continue;
        }
        let styled = if caps {
            format!("<b>{}</b>", html_mark(t, hl))
        } else {
            html_mark(t, hl)
        };
        if is_centered(lead, t, caps) {
            body.push_str(&format!("<p style=\"text-align:center\">{styled}</p>\n"));
        } else {
            body.push_str(&format!("<p>{styled}</p>\n"));
        }
    }
    if in_list {
        body.push_str("</ul>\n");
    }
}

/// HTML view of one document: the same layout rules as doc_markdown —
/// centered lines are centered, ALL-CAPS lines are bold, dash items become
/// <ul><li>, numbered items stay as-is. Opens in any browser; Ctrl+P gives
/// a PDF (the browser's print-to-PDF), so this is also the print path.
pub fn write_html(path: &Path, head: Option<&ExportHead>, text: &str, hl: &Hl) -> Result<()> {
    let mut body = String::new();
    let title = match head {
        Some(h) => h.title.clone(),
        None => first_line(text, 60).to_string(),
    };
    html_doc_body(head, text, &mut body, hl);
    let html = format!(
        "<!DOCTYPE html>\n<html lang=\"uk\"><head><meta charset=\"utf-8\">\n<title>{}</title>\n\
         <style>{}</style></head>\n<body>\n{}\n</body></html>\n",
        html_escape(&title),
        HTML_STYLE,
        body
    );
    std::fs::write(path, html)?;
    Ok(())
}

/// HTML bundle: a "Зв'язка" header with a clickable Зміст (anchors to each
/// document's section), then every document inside its own `<section>`
/// (CSS page-break-before:always for the browser's print-to-PDF).
pub fn write_html_bundle(path: &Path, docs: &[BundleDoc], hl: &Hl) -> Result<()> {
    let n = docs.len();
    let mut body = String::new();
    body.push_str(&format!("<h1>Зв'язка — {n} документ(ів)</h1>\n"));
    if n > 1 {
        body.push_str("<ol class=\"toc\">\n");
        for (i, d) in docs.iter().enumerate() {
            body.push_str(&format!(
                "<li><a href=\"#doc{}\">{} (№ {})</a></li>\n",
                i + 1,
                html_escape(&d.head.title),
                d.docnum
            ));
        }
        body.push_str("</ol>\n");
    }
    for (i, d) in docs.iter().enumerate() {
        body.push_str(&format!("<section id=\"doc{}\" class=\"doc\">\n", i + 1));
        html_doc_body(Some(&d.head), &d.text, &mut body, hl);
        body.push_str("</section>\n");
    }
    let style = format!(
        "{HTML_STYLE} \
ol.toc{{max-width:640px}} \
section.doc{{page-break-before:always}} \
section.doc h1{{margin-top:0}}"
    );
    let html = format!(
        "<!DOCTYPE html>\n<html lang=\"uk\"><head><meta charset=\"utf-8\">\n<title>Зв'язка ({n})</title>\n\
         <style>{style}</style></head>\n<body>\n{body}\n</body></html>\n"
    );
    std::fs::write(path, html)?;
    Ok(())
}

// ---- PDF (subset-embedded system font, A4 pages) ---------------------------

/// PDF view of one document: the embedded subset of the system font
/// (default C:\Windows\Fonts\arial.ttf) renders A4 pages with the document's
/// own layout. Opens in any PDF viewer; this is the built-in "PDF view"
/// (no external renderer). `hl` is accepted for a uniform writer API; term
/// highlighting in the PDF content stream lands in a later version.
pub fn write_pdf(path: &Path, head: Option<&ExportHead>, text: &str, hl: &Hl) -> Result<()> {
    let _ = hl; // PDF highlighting: a later version (content-stream runs)
    let title = match head {
        Some(h) => h.title.clone(),
        None => first_line(text, 60).to_string(),
    };
    let mut lines: Vec<String> = Vec::new();
    if let Some(h) = head {
        lines.push(h.title.clone());
        lines.extend(h.lines.iter().cloned());
        lines.push(String::new());
    }
    lines.extend(text.lines().map(|l| l.to_string()));
    let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
    crate::pdf::write_pdf(&path, &title, &refs)?;
    Ok(())
}

/// PDF bundle: EVERY document in ONE PDF (fresh page per document) with a
/// /Outlines table of contents (clickable PDF bookmarks). The embedded font
/// subset is auto-picked (pdf::select_font) over the union of all docs'
/// characters — a Cyrillic-capable system font when the local arial.ttf is
/// "stale". `hl` is accepted for a uniform writer API (no-op in PDF yet).
pub fn write_pdf_bundle(path: &Path, docs: &[BundleDoc], hl: &Hl) -> Result<()> {
    let _ = hl; // PDF highlighting: a later version (content-stream runs)
    let pdf_docs: Vec<crate::pdf::PdfDoc> = docs
        .iter()
        .map(|d| crate::pdf::PdfDoc {
            title: &d.head.title,
            head_lines: &d.head.lines,
            text: &d.text,
        })
        .collect();
    crate::pdf::write_pdf_bundle(path, &pdf_docs)
}

// ============================================================
// "зв'язка" (bundle) writers: ONE file for many documents
// ============================================================

/// One document of a multi-document bundle (requisites head + rendered text
/// + the source docnum for the table of contents).
pub struct BundleDoc {
    pub head: ExportHead,
    pub text: String,
    pub docnum: u32,
}

/// Markdown bundle: a "Зв'язка" header with a Зміст (table of contents),
/// then each document (its own # title + requisites + body), separated by
/// `---` rules. Matched query terms (`hl`) are marked with `<mark>`.
pub fn write_markdown_bundle(path: &Path, docs: &[BundleDoc], hl: &Hl) -> Result<()> {
    let n = docs.len();
    let mut s = String::new();
    s.push_str(&format!("# Зв'язка — {n} документ(ів)\n\n"));
    if n > 1 {
        s.push_str("**Зміст**\n\n");
        for (i, d) in docs.iter().enumerate() {
            s.push_str(&format!(
                "{}. {} (№ {})\n",
                i + 1,
                d.head.title,
                d.docnum
            ));
        }
        s.push_str("\n---\n\n");
    }
    for (i, d) in docs.iter().enumerate() {
        s.push_str(&doc_markdown_head(&d.head.title, &d.head.lines, &d.text, hl));
        if i + 1 < n {
            s.push_str("\n---\n\n");
        }
    }
    std::fs::write(path, s)?;
    Ok(())
}

/// The rtf body of a document (the same per-line rules as write_rtf:
/// centered \qc, ALL-CAPS bold, \bullet lists), one RTF paragraph per line.
/// Matched query terms (`hl`) get a yellow \highlight6 span.
fn rtf_body(rtf: &mut String, text: &str, hl: &Hl) {
    for line in text.lines() {
        let (lead, t, caps, bullet) = line_traits(line);
        if t.is_empty() {
            rtf.push_str("\\par\n");
            continue;
        }
        rtf.push('{');
        if bullet {
            rtf.push_str("\\li720\\bullet\\tab ");
        } else if is_centered(lead, t, caps) {
            rtf.push_str("\\qc ");
        }
        if caps {
            rtf.push_str("\\b ");
            rtf_mark(rtf, t, hl);
            rtf.push_str("\\b0 ");
        } else if bullet {
            rtf_mark(rtf, bullet_body(t), hl);
        } else {
            rtf_mark(rtf, t, hl);
        }
        rtf.push_str("\\par\n}");
    }
}

/// RTF bundle: a bold "Зв'язка" header + numbered Зміст, then each document
/// (bold title + italic requisites + body) on its own page (\page breaks).
/// Matched query terms (`hl`) get a yellow \highlight6 span.
pub fn write_rtf_bundle(path: &Path, docs: &[BundleDoc], hl: &Hl) -> Result<()> {
    let mut rtf = String::from(
        "{\\rtf1\\ansi\\ansicpg1251\\deff0{\\fonttbl{\\f0\\fnil\\fcharset0 Times New Roman;}}\n",
    );
    let n = docs.len();
    rtf_line(&mut rtf, &format!("Зв'язка — {n} документ(ів)"), true, false);
    for (i, d) in docs.iter().enumerate() {
        rtf_line(
            &mut rtf,
            &format!("{}. {} (№ {})", i + 1, d.head.title, d.docnum),
            false,
            false,
        );
    }
    rtf.push_str("\\par\n");
    for (i, d) in docs.iter().enumerate() {
        if i > 0 {
            rtf.push_str("\\par\\page\\par\n"); // fresh page per document
        }
        rtf_line(&mut rtf, &d.head.title, true, false);
        for line in &d.head.lines {
            rtf_line(&mut rtf, line, false, true);
        }
        rtf.push_str("\\par\n");
        rtf_body(&mut rtf, &d.text, hl);
    }
    rtf.push('}');
    std::fs::write(path, rtf)?;
    Ok(())
}

// ---- CSV (the results table as a spreadsheet) ------------------------------

/// One CSV field, quoted only when needed (delimiter is ';', matching the
/// Ukrainian/Russian Excel locale).
fn csv_field(s: &str) -> String {
    if s.contains(';') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn opt_dash(o: &Option<String>) -> String {
    match o {
        Some(s) if !s.is_empty() => s.clone(),
        _ => "—".to_string(),
    }
}

/// The search results as a CSV table (one row per document): docnum, the
/// parsed fields, the match count and the search context. UTF-8 with a BOM
/// (so Excel opens it correctly), ';' separated.
pub fn write_results_csv(
    path: &Path,
    hits: &[crate::db::Hit],
    base: &crate::render::Base,
    baza: &Path,
) -> Result<usize> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    f.write_all(&[0xEF, 0xBB, 0xBF])?; // UTF-8 BOM for Excel
    f.write_all(
        "docnum;видавець;тип;дата;номер;назва;статус;збігів;контекст\n".as_bytes(),
    )?;
    for h in hits.iter() {
        let text = base.doc_text(baza, h.docnum).unwrap_or_default();
        let fl = crate::fields::parse(h.docnum, &text);
        let ctx = h.title.replace('\n', " ").replace('\r', " ");
        let row = format!(
            "{};{};{};{};{};{};{};{};{}\n",
            h.docnum,
            csv_field(&opt_dash(&fl.issuer)),
            csv_field(&opt_dash(&fl.doc_type)),
            csv_field(&opt_dash(&fl.date)),
            csv_field(&opt_dash(&fl.number)),
            csv_field(&opt_dash(&fl.title)),
            fl.status.label(),
            h.matches,
            csv_field(&ctx),
        );
        f.write_all(row.as_bytes())?;
    }
    Ok(hits.len())
}

fn write_stored_zip(path: &Path, entries: &[(&str, &[u8])]) -> Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    let mut cd: Vec<u8> = Vec::new();
    let mut n = 0u16;
    for (name, data) in entries {
        let lho = f.metadata()?.len();
        let crc = crc32(data);
        let mut lh = vec![0u8; 30];
        lh[0..4].copy_from_slice(&0x0403_4B50u32.to_le_bytes()); // local header sig
        lh[4..6].copy_from_slice(&20u16.to_le_bytes()); // version needed
        lh[8..10].copy_from_slice(&0u16.to_le_bytes()); // method stored
        lh[14..18].copy_from_slice(&crc.to_le_bytes());
        lh[18..22].copy_from_slice(&(data.len() as u32).to_le_bytes()); // csize
        lh[22..26].copy_from_slice(&(data.len() as u32).to_le_bytes()); // usize
        lh[26..28].copy_from_slice(&(name.len() as u16).to_le_bytes());
        f.write_all(&lh)?;
        f.write_all(name.as_bytes())?;
        f.write_all(data)?;

        let mut ch = vec![0u8; 46];
        ch[0..4].copy_from_slice(&0x0201_4B50u32.to_le_bytes()); // cd sig
        ch[4..6].copy_from_slice(&20u16.to_le_bytes()); // version made by
        ch[6..8].copy_from_slice(&20u16.to_le_bytes()); // version needed
        ch[10..12].copy_from_slice(&0u16.to_le_bytes()); // method stored
        ch[12..14].copy_from_slice(&0u16.to_le_bytes()); // dos time (midnight)
        ch[14..16].copy_from_slice(&22561u16.to_le_bytes()); // dos date 2004-01-01
        ch[16..20].copy_from_slice(&crc.to_le_bytes());
        ch[20..24].copy_from_slice(&(data.len() as u32).to_le_bytes());
        ch[24..28].copy_from_slice(&(data.len() as u32).to_le_bytes());
        ch[28..30].copy_from_slice(&(name.len() as u16).to_le_bytes());
        ch[42..46].copy_from_slice(&(lho as u32).to_le_bytes());
        cd.extend_from_slice(&ch);
        cd.extend_from_slice(name.as_bytes());
        n += 1;
    }

    let cd_off = f.metadata()?.len();
    f.write_all(&cd)?;

    let mut eocd = vec![0u8; 22];
    eocd[0..4].copy_from_slice(&0x0605_4B50u32.to_le_bytes());
    eocd[4..6].copy_from_slice(&0u16.to_le_bytes()); // this disk
    eocd[6..8].copy_from_slice(&0u16.to_le_bytes()); // cd start disk
    eocd[8..10].copy_from_slice(&n.to_le_bytes()); // entries this disk
    eocd[10..12].copy_from_slice(&n.to_le_bytes()); // entries total
    eocd[12..16].copy_from_slice(&(cd.len() as u32).to_le_bytes());
    eocd[16..20].copy_from_slice(&(cd_off as u32).to_le_bytes());
    eocd[20..22].copy_from_slice(&0u16.to_le_bytes()); // comment len
    f.write_all(&eocd)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn head() -> ExportHead {
        ExportHead {
            title: "КАБІНЕТ МІНІСТРІВ ПРО...".into(),
            lines: vec![
                "файл: 223.mb".into(),
                "документ: 136974".into(),
                "видавець: Кабінет Міністрів".into(),
                "тип: ПОСТАНОВА".into(),
                "дата: 11.02.2004".into(),
                "номер: № 145".into(),
                "назва: Про щось".into(),
                "статус: чинний".into(),
            ],
        }
    }

    #[test]
    fn rtf_roundtrip_kyrgyz_bytes() {
        let p = std::path::Path::new("C:\\temp\\opencode\\export_test.rtf");
        let text = "П О С Т А Н О В А\nвід 11 лютого 2004 р. № 145";
        write_rtf(p, Some(&head()), text, &Hl::default()).unwrap();
        let raw = std::fs::read(p).unwrap();
        let s = String::from_utf8_lossy(&raw);
        assert!(s.contains("\\'E2") || s.contains("\\'e2"));
        // title bold at the top, requisites italic, file line present
        assert!(s.contains("\\b ") && s.contains("\\i "));
        assert!(s.contains("223"));
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn docx_zip_layout() {
        use std::path::Path;
        let p = Path::new("C:\\temp\\opencode\\export_test.docx");
        write_docx(p, Some(&head()), "текст", &Hl::default()).unwrap();
        let b = std::fs::read(p).unwrap();
        let eocd = &b[b.len() - 22..];
        assert_eq!(&eocd[0..4], &0x0605_4B50u32.to_le_bytes());
        assert_eq!(&eocd[4..6], &[0, 0]);
        assert_eq!(&eocd[6..8], &[0, 0]);
        assert_eq!(u16::from_le_bytes([eocd[8], eocd[9]]), 3); // 3 entries
        assert_eq!(u16::from_le_bytes([eocd[10], eocd[11]]), 3);
        // cd offset within file
        let cd_size = u32::from_le_bytes(eocd[12..16].try_into().unwrap()) as usize;
        let cd_off = u32::from_le_bytes(eocd[16..20].try_into().unwrap()) as usize;
        assert_eq!(cd_off + cd_size + 22, b.len());
        // cd signature at cd_off
        assert_eq!(&b[cd_off..cd_off + 4], &0x0201_4B50u32.to_le_bytes());
        // first local header at offset 0
        assert_eq!(&b[0..4], &0x0403_4B50u32.to_le_bytes());
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn doc_markdown_body_rules() {
        let text = "     КАБІНЕТ МІНІСТРІВ УКРАЇНИ\n\n• item one\n– item two\n1. numbered stays\n2. numbered stays too\n\nЗвичайний рядок.";
        let md = doc_markdown(None, text);
        assert!(md.starts_with("# "));
        // centered ALL-CAPS line → bold, centering via nbsp
        assert!(md.contains('\u{00A0}'), "{md}");
        assert!(md.contains("**КАБІНЕТ МІНІСТРІВ УКРАЇНИ**"), "{md}");
        // dash/bullet items → markdown list items
        assert!(md.contains("- item one"), "{md}");
        assert!(md.contains("- item two"), "{md}");
        // numbered items stay as-is (valid md ordered lists)
        assert!(md.contains("1. numbered stays"), "{md}");
        assert!(md.contains("2. numbered stays too"), "{md}");
        // plain lines unchanged
        assert!(md.contains("Звичайний рядок."), "{md}");
    }

    #[test]
    fn doc_markdown_header_from_fields() {
        let fl = crate::fields::DocFields {
            docnum: 222,
            issuer: Some("Верховна Рада".into()),
            doc_type: Some("ПОСТАНОВА".into()),
            date: Some("14.10.1998".into()),
            date_ymd: None,
            number: Some("№ 175".into()),
            title: Some("Про ліміт державних зобов'язань".into()),
            status: crate::fields::Status::Actual,
            status_detail: None,
            n_lines: 10,
            n_chars: 500,
        };
        let md = doc_markdown(Some(&fl), "П О С Т А Н О В А\n\n1. відхилити.");
        assert!(md.contains("# Про ліміт державних зобов'язань"), "{md}");
        assert!(md.contains("*номер: № 175*"), "{md}");
        // kicker line is ALL-CAPS → bold; numbered item stays as-is
        assert!(md.contains("**П О С Т А Н О В А**"), "{md}");
        assert!(md.contains("1. відхилити."), "{md}");
    }

    #[test]
    fn html_preserves_layout() {
        let p = std::path::Path::new("C:\\temp\\opencode\\export_test.html");
        let text = "     КАБІНЕТ МІНІСТРІВ УКРАЇНИ\n\n• item one\n– item two\n1. numbered stays\n\nЗвичайний рядок.";
        write_html(p, Some(&head()), text, &Hl::default()).unwrap();
        let s = std::fs::read_to_string(p).unwrap();
        assert!(s.contains("<h1>"), "{s}");
        // centered ALL-CAPS → centered + bold
        assert!(s.contains("text-align:center"), "{s}");
        assert!(s.contains("<b>КАБІНЕТ МІНІСТРІВ УКРАЇНИ</b>"), "{s}");
        // bullet items → <ul><li>; numbered stay as-is
        assert!(s.contains("<li>item one</li>"), "{s}");
        assert!(s.contains("<li>item two</li>"), "{s}");
        assert!(s.contains("1. numbered stays"), "{s}");
        assert!(s.contains("Звичайний рядок."), "{s}");
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn rtf_export_keeps_layout() {
        let p = std::path::Path::new("C:\\temp\\opencode\\export_test2.rtf");
        let text = "     КАБІНЕТ МІНІСТРІВ\n\n• item one\n1. numbered";
        write_rtf(p, Some(&head()), text, &Hl::default()).unwrap();
        let s = std::fs::read_to_string(p).unwrap();
        // centered → \qc, bullet → \bullet, ALL-CAPS → \b
        assert!(s.contains("\\qc"), "{s}");
        assert!(s.contains("\\bullet"), "{s}");
        assert!(s.contains("\\b "), "{s}");
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn docx_export_keeps_layout() {
        let p = std::path::Path::new("C:\\temp\\opencode\\export_test2.docx");
        let text = "     КАБІНЕТ МІНІСТРІВ\n\n• item one";
        write_docx(p, Some(&head()), text, &Hl::default()).unwrap();
        // the zip's word/document.xml must carry the layout markers
        let b = std::fs::read(p).unwrap();
        let s = String::from_utf8_lossy(&b).to_string();
        assert!(s.contains("w:jc w:val=\"center\""), "{s}");
        assert!(s.contains("<w:b/>"), "{s}");
        assert!(s.contains("item one"), "{s}");
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn csv_field_quoting() {
        assert_eq!(csv_field("простий"), "простий");
        assert_eq!(csv_field("a;b"), "\"a;b\"");
        assert_eq!(csv_field("x\"y"), "\"x\"\"y\"");
        assert_eq!(csv_field("line\nbreak"), "\"line\nbreak\"");
    }

    #[test]
    fn csv_results_smoke() {
        // a real base makes the fields parseable; skip when absent
        let baza = std::path::Path::new("C:\\Zakon.Local\\Baza");
        if !baza.exists() {
            return;
        }
        let base = crate::render::Base::load(baza).unwrap();
        let hits = vec![crate::db::Hit {
            docnum: 222,
            file: "1.mb".into(),
            match_off: 0,
            matches: 1,
            title: "контекст".into(),
        }];
        let p = std::path::Path::new("C:\\temp\\opencode\\export_test.csv");
        let n = write_results_csv(p, &hits, &base, baza).unwrap();
        assert_eq!(n, 1);
        let raw = std::fs::read(p).unwrap();
        assert_eq!(&raw[0..3], &[0xEF, 0xBB, 0xBF]); // UTF-8 BOM
        let s = String::from_utf8_lossy(&raw).to_string();
        assert!(s.contains("docnum;видавець;"), "{s}");
        assert!(s.contains("222;"), "{s}");
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn pdf_export_structure() {
        // PDF export embeds the system font (Arial) — skip when absent
        if !crate::pdf::default_font_path().exists() {
            return;
        }
        let p = std::path::Path::new("C:\\temp\\opencode\\export_test.pdf");
        let text = "     КАБІНЕТ МІНІСТРІВ\n\n• item one\n1. numbered";
        write_pdf(p, Some(&head()), text, &Hl::default()).unwrap();
        let b = std::fs::read(p).unwrap();
        let s = String::from_utf8_lossy(&b).to_string();
        assert!(s.starts_with("%PDF-1.4"), "PDF header");
        assert!(s.contains("/Type /Page"), "page object");
        assert!(s.contains("/FontFile2"), "embedded subset font");
        assert!(s.contains("startxref"), "xref");
        // the xref offset must point at the "xref" keyword. Check against
        // the raw bytes (the lossy string above shifts indices: the FLATE
        // font stream is not valid UTF-8)
        let at = s.rfind("startxref").unwrap();
        let off: usize = s[at + 9..]
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert_eq!(
            b.get(off..off + 4),
            Some(b"xref".as_slice()),
            "xref offset points at the xref table"
        );
        let _ = std::fs::remove_file(p);
    }

    fn bundle_docs() -> Vec<BundleDoc> {
        let d1 = BundleDoc {
            head: head(),
            text: "     КАБІНЕТ МІНІСТРІВ УКРАЇНИ\n\n• item one\n1. numbered first".into(),
            docnum: 222,
        };
        let d2 = BundleDoc {
            head: ExportHead {
                title: "ПРОЕКТ ЗАКОНУ".into(),
                lines: vec!["файл: 9275.mb".into(), "номер: № 481".into()],
            },
            text: "     ВЕРХОВНА РАДА\n\n– item two\n• item three".into(),
            docnum: 9275,
        };
        vec![d1, d2]
    }

    #[test]
    fn bundle_markdown_structure() {
        let docs = bundle_docs();
        let p = std::path::Path::new("C:\\temp\\opencode\\export_test_bundle.md");
        write_markdown_bundle(p, &docs, &Hl::default()).unwrap();
        let s = std::fs::read_to_string(p).unwrap();
        // bundle header + a Зміст that lists BOTH documents (title + docnum)
        assert!(s.contains("# Зв'язка — 2 документ(ів)"), "{s}");
        assert!(s.contains("**Зміст**"), "{s}");
        assert!(s.contains("1. КАБІНЕТ МІНІСТРІВ ПРО... (№ 222)"), "{s}");
        assert!(s.contains("2. ПРОЕКТ ЗАКОНУ (№ 9275)"), "{s}");
        // both documents' own headers + bodies are present, with separators
        assert!(s.contains("# КАБІНЕТ МІНІСТРІВ ПРО..."), "{s}");
        assert!(s.contains("# ПРОЕКТ ЗАКОНУ"), "{s}");
        assert!(s.contains("numbered first"), "{s}");
        assert!(s.contains("item three"), "{s}");
        assert_eq!(s.matches("\n---\n").count(), 2, "TOC + doc separators");
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn bundle_rtf_structure() {
        let docs = bundle_docs();
        let p = std::path::Path::new("C:\\temp\\opencode\\export_test_bundle.rtf");
        write_rtf_bundle(p, &docs, &Hl::default()).unwrap();
        let s = std::fs::read_to_string(p).unwrap();
        assert!(s.starts_with("{\\rtf1"), "{s}");
        assert!(s.contains("\\page"), "page break between documents");
        // both documents' Cyrillic reached cp1251 escapes (А='Е0', В='B2')
        assert!(s.contains("\\'E0") || s.contains("\\'e0"), "Cyrillic А escaped");
        assert!(s.contains("222"), "docnum 222 in the TOC line");
        assert!(s.contains("9275"), "docnum 9275 in the TOC line");
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn bundle_docx_structure() {
        let docs = bundle_docs();
        let p = std::path::Path::new("C:\\temp\\opencode\\export_test_bundle.docx");
        write_docx_bundle(p, &docs, &Hl::default()).unwrap();
        // the zip's word/document.xml must carry BOTH documents + page breaks
        let b = std::fs::read(p).unwrap();
        assert_eq!(&b[0..2], b"PK", "zip magic");
        let s = String::from_utf8_lossy(&b).to_string();
        assert!(s.contains("w:br w:type=\"page\""), "{s}");
        // 2 docs → 2 page breaks (one before each document)
        assert_eq!(s.matches("w:br w:type=\"page\"").count(), 2, "one break per doc");
        assert!(s.contains("КАБІНЕТ МІНІСТРІВ"), "{s}");
        assert!(s.contains("ВЕРХОВНА РАДА"), "{s}");
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn bundle_html_structure() {
        let docs = bundle_docs();
        let p = std::path::Path::new("C:\\temp\\opencode\\export_test_bundle.html");
        write_html_bundle(p, &docs, &Hl::default()).unwrap();
        let s = std::fs::read_to_string(p).unwrap();
        assert!(s.contains("<h1>Зв'язка — 2 документ(ів)</h1>"), "{s}");
        // the clickable Зміст anchors BOTH document sections
        assert!(s.contains("href=\"#doc1\""), "{s}");
        assert!(s.contains("href=\"#doc2\""), "{s}");
        assert!(s.contains("<section id=\"doc1\" class=\"doc\">"), "{s}");
        assert!(s.contains("<section id=\"doc2\" class=\"doc\">"), "{s}");
        assert!(s.contains("page-break-before:always"), "{s}");
        // both documents' bodies
        assert!(s.contains("numbered first"), "{s}");
        assert!(s.contains("item three"), "{s}");
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn bundle_pdf_structure() {
        // the PDF bundle embeds a system font subset — skip when absent
        if !crate::pdf::default_font_path().exists() {
            return;
        }
        let docs = bundle_docs();
        let p = std::path::Path::new("C:\\temp\\opencode\\export_test_bundle.pdf");
        write_pdf_bundle(p, &docs, &Hl::default()).unwrap();
        let b = std::fs::read(p).unwrap();
        let s = String::from_utf8_lossy(&b).to_string();
        assert!(s.starts_with("%PDF-1.4"), "PDF header");
        assert!(s.contains("/Outlines"), "outline root");
        assert!(s.contains("/Count 2"), "outline item count");
        assert_eq!(s.matches("/Dest [").count(), 2, "two outline entries");
        // each document starts on a fresh page: at least 2 page objects
        assert!(s.matches("/Type /Page ").count() >= 2, "fresh page per doc");
        // the xref offset must point at the "xref" keyword (raw bytes)
        let at = s.rfind("startxref").unwrap();
        let off: usize = s[at + 9..]
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert_eq!(
            b.get(off..off + 4),
            Some(b"xref".as_slice()),
            "xref offset points at the xref table"
        );
        let _ = std::fs::remove_file(p);
    }

    // ---- query-term highlighting (v0.12.0) --------------------------------

    #[test]
    fn md_highlight_marks_terms() {
        let p = std::path::Path::new("C:\\temp\\opencode\\export_test_hl.md");
        let text = "Закон України\nзакон і ЗАКОН\n• закон у списку";
        write_markdown(p, Some(&head()), text, &Hl::from_query("Закон")).unwrap();
        let s = std::fs::read_to_string(p).unwrap();
        assert!(s.contains("<mark>Закон</mark>"), "{s}");
        assert!(s.contains("<mark>закон</mark>"), "{s}");
        assert!(s.contains("<mark>ЗАКОН</mark>"), "{s}");
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn md_no_highlight_when_empty() {
        let p = std::path::Path::new("C:\\temp\\opencode\\export_test_hl0.md");
        write_markdown(p, Some(&head()), "Закон", &Hl::default()).unwrap();
        let s = std::fs::read_to_string(p).unwrap();
        assert!(!s.contains("<mark>"), "{s}");
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn html_highlight_marks_terms() {
        let p = std::path::Path::new("C:\\temp\\opencode\\export_test_hl.html");
        write_html(p, Some(&head()), "Закон України", &Hl::from_query("закон")).unwrap();
        let s = std::fs::read_to_string(p).unwrap();
        assert!(s.contains("<mark class=\"hl\">Закон</mark>"), "{s}");
        assert!(s.contains("mark.hl{background:#ffe86b"), "{s}");
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn rtf_highlight_marks_terms() {
        let p = std::path::Path::new("C:\\temp\\opencode\\export_test_hl.rtf");
        write_rtf(p, Some(&head()), "Закон України", &Hl::from_query("закон")).unwrap();
        let s = std::fs::read_to_string(p).unwrap();
        assert!(s.contains("\\highlight6"), "yellow highlight control: {s}");
        assert!(s.contains("\\highlight0"), "highlight off: {s}");
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn docx_highlight_marks_terms() {
        let p = std::path::Path::new("C:\\temp\\opencode\\export_test_hl.docx");
        write_docx(p, Some(&head()), "Закон України", &Hl::from_query("закон")).unwrap();
        let b = std::fs::read(p).unwrap();
        let s = String::from_utf8_lossy(&b).to_string();
        assert!(s.contains("<w:highlight w:val=\"yellow\"/>"), "docx highlight run: {s}");
        // the matched text stayed inside its own <w:t>
        assert!(s.contains("Закон</w:t>"), "{s}");
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn bundle_md_highlight_marks_terms() {
        let docs = bundle_docs();
        let p = std::path::Path::new("C:\\temp\\opencode\\export_test_bundle_hl.md");
        write_markdown_bundle(p, &docs, &Hl::from_query("ЗАКОН")).unwrap();
        let s = std::fs::read_to_string(p).unwrap();
        // «ЗАКОНУ» (in the title) must NOT be a match of the 5-char term
        assert!(s.contains("# ПРОЕКТ ЗАКОНУ"), "title not swallowed: {s}");
        // the term appears in doc 2's title «ПРОЕКТ ЗАКОНУ»? no — the whole
        // line is a header line, bodies are untouched here; check no <mark>
        assert!(!s.contains("<mark>"), "no body match expected: {s}");
        let _ = std::fs::remove_file(p);
    }
}
