//! Export writers (markdown / RTF / DOCX). Text comes from the render engine
//! (dictionary token expansion), one document per hit.

use anyhow::Result;
use std::path::Path;

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
    let mut exported = 0usize;
    let mut attached = 0usize;
    let total = hits.len();
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
        let head = head_from(&f, &h.file, &title, sig_name.as_deref());
        match format {
            "md" => write_markdown(&fpath, Some(&head), &text)?,
            "rtf" => write_rtf(&fpath, Some(&head), &text)?,
            "docx" => write_docx(&fpath, Some(&head), &text)?,
            "html" => write_html(&fpath, Some(&head), &text)?,
            "pdf" => write_pdf(&fpath, Some(&head), &text)?,
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
/// are left as-is (already valid markdown ordered lists).
fn md_body_fmt(text: &str) -> String {
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
            out.push_str(bullet_body(t));
        } else if caps {
            out.push_str(&format!("**{t}**"));
        } else {
            out.push_str(t);
        }
        out.push('\n');
    }
    out
}

/// Format-preserving Markdown view of one document: `# title` heading,
/// italic requisites lines, then the formatted body.
pub fn doc_markdown_head(title: &str, meta: &[String], text: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {title}\n\n"));
    for line in meta {
        out.push_str(&format!("*{line}*\n"));
    }
    if !meta.is_empty() {
        out.push('\n');
    }
    out.push_str(&md_body_fmt(text));
    out
}

/// Format-preserving Markdown view of one document (fields-based header).
pub fn doc_markdown(f: Option<&crate::fields::DocFields>, text: &str) -> String {
    match f {
        Some(fl) => {
            let title = fl.title.clone().unwrap_or_else(|| first_line(text, 80));
            let meta: Vec<String> = fl.display().lines().map(String::from).collect();
            doc_markdown_head(&title, &meta, text)
        }
        None => doc_markdown_head(&first_line(text, 80), &[], text),
    }
}

/// Markdown export: the same format-preserving layout as doc_markdown_head
/// (`# title`, `*requisites*`, centered / bold / list-aware body).
pub fn write_markdown(path: &Path, head: Option<&ExportHead>, text: &str) -> Result<()> {
    let (title, meta) = match head {
        Some(h) => (h.title.clone(), h.lines.clone()),
        None => (first_line(text, 80).to_string(), Vec::new()),
    };
    std::fs::write(path, doc_markdown_head(&title, &meta, text))?;
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
/// stay as-is.
pub fn write_rtf(path: &Path, head: Option<&ExportHead>, text: &str) -> Result<()> {
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
            rtf_run(&mut rtf, t);
            rtf.push_str("\\b0 ");
        } else if bullet {
            rtf_run(&mut rtf, bullet_body(t));
        } else {
            rtf_run(&mut rtf, t);
        }
        rtf.push_str("\\par\n}");
    }
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

pub fn write_docx(path: &Path, head: Option<&ExportHead>, text: &str) -> Result<()> {
    // bold title paragraph + italic requisites lines
    let mut paras = String::new();
    if let Some(h) = head {
        paras.push_str(&format!(
            "<w:p><w:pPr><w:rPr><w:b/></w:rPr></w:pPr><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
            xml_escape(&h.title)
        ));
        for line in &h.lines {
            paras.push_str(&format!(
                "<w:p><w:pPr><w:rPr><w:i/></w:rPr></w:pPr><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
                xml_escape(line)
            ));
        }
        paras.push_str("<w:p/>");
    }
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
        let content = if bullet {
            format!("- {}", xml_escape(bullet_body(t)))
        } else {
            xml_escape(t)
        };
        paras.push_str(&format!(
            "<w:p>{ppr_block}<w:r><w:t xml:space=\"preserve\">{content}</w:t></w:r></w:p>"
        ));
    }
    let doc = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
<w:body>{paras}</w:body></w:document>"
    );
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
    write_stored_zip(path, &entries)?;
    Ok(())
}

// ---- HTML (format-preserving; the browser's print-to-PDF path) ------------

const HTML_STYLE: &str = "\
body{font-family:'Times New Roman',serif;font-size:14px;line-height:1.45;\
max-width:900px;margin:24px auto;padding:0 16px;color:#111}\
h1{font-size:20px;text-align:center;margin:4px 0 12px}\
p.meta{color:#444;font-style:italic;font-size:13px;text-align:center;margin:0 0 18px}\
ul{margin:6px 0;padding-left:28px}\
li{margin:2px 0}";

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// HTML view of one document: the same layout rules as doc_markdown —
/// centered lines are centered, ALL-CAPS lines are bold, dash items become
/// <ul><li>, numbered items stay as-is. Opens in any browser; Ctrl+P gives
/// a PDF (the browser's print-to-PDF), so this is also the print path.
pub fn write_html(path: &Path, head: Option<&ExportHead>, text: &str) -> Result<()> {
    let mut body = String::new();
    let title = match head {
        Some(h) => h.title.clone(),
        None => first_line(text, 60).to_string(),
    };
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
            body.push_str(&format!("  <li>{}</li>\n", html_escape(bullet_body(t))));
            continue;
        }
        let styled = if caps {
            format!("<b>{}</b>", html_escape(t))
        } else {
            html_escape(t)
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

// ---- PDF (subset-embedded system font, A4 pages) ---------------------------

/// PDF view of one document: the embedded subset of the system font
/// (default C:\Windows\Fonts\arial.ttf) renders A4 pages with the document's
/// own layout. Opens in any PDF viewer; this is the built-in "PDF view"
/// (no external renderer).
pub fn write_pdf(path: &Path, head: Option<&ExportHead>, text: &str) -> Result<()> {
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
        write_rtf(p, Some(&head()), text).unwrap();
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
        write_docx(p, Some(&head()), "текст").unwrap();
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
        write_html(p, Some(&head()), text).unwrap();
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
        write_rtf(p, Some(&head()), text).unwrap();
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
        write_docx(p, Some(&head()), text).unwrap();
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
        write_pdf(p, Some(&head()), text).unwrap();
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
}
