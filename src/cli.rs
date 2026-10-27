//! Zakon DB Exporter — console CLI implementation (verbs, flags, filters).
//! Lives in the lib target so `zakonexport-cli` and `zakonexport-gui` share
//! the same engine. Invoked by the thin binary in src/main.rs.

use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

// sibling engine modules (same names as in the original main.rs scope)
use crate::{db, fields, gui, naming, render, writers};

fn usage() {
    let v = crate::VERSION;
    println!(
        "zakonexport-cli {v} — search, filter, pick and export Zakon BAZA documents
(expanded token stream, Cyrillic-safe; two bases supported)

Usage:
  zakonexport-cli gui [--guitest] [--base general|audit|<path>] [--baza <path>]
  zakonexport-cli search <query> [--number] [--max n] [--fields] [search flags]
  zakonexport-cli export <query> [search flags] [--number] [filters] [--format md|rtf|docx|html|csv|pdf]
        [--out dir] [--docs n1,n2-n5 | --pick] [--single] [--hl]
  zakonexport-cli export --docs 136974,222-225 [--format docx] [--out dir]
         (md/rtf/docx/html/pdf = one file per document, format-preserving layout;
          pdf embeds a subset of the auto-picked system font (Cyrillic fallback);
          csv = a single results-table file for the whole hit set)
  zakonexport-cli fields <docnum>
  zakonexport-cli extract <docnum>
  zakonexport-cli signal <docnum>   locate the doc's signal attachment (if any)
  zakonexport-cli print <docnum> [docnum ...]    GDI print (system print dialog, no Word);
          several docnums (space- or comma-separated, ranges) print one after another
  zakonexport-cli trident          report the base's native FTS index status
          (SOFT\\ inventory — 16 files incl. windows.kod + lm.a, plus the
          decoded parts: kod codes, fold pairs, dict records, vt stream;
          search via tsearch — experimental)
  zakonexport-cli tsearch <query>  experimental: search the base's native
          Trident index (SOFT\\lmu.voc + lmur.vt) — the legacy main app's
          engine. The document numbers are the native 2005-era Trident ids
          (the join to the current base docnums is in development)
  zakonexport-cli ftsearch <query>  full-text scan of the base's `.mb`
          document files (case-insensitive exact word — the legacy app's
          oracle mechanism); reports (file @ offset) hits like the oracle,
          no B-tree decode needed
  zakonexport-cli bulk [--format md|rtf|docx|html|csv|pdf] [--out dir] [--max n] [--single]
  zakonexport-cli stats
  zakonexport-cli about   (= --version: name, version, copyright)

GUI: zakonexport-gui.exe (double-click — opens the window directly,
same engines; console-free in release builds)

Search flags:
  --and        every term must occur (default)
  --or         any term may occur
  --phrase     whole query must occur as one phrase
  --number     document-number search: «№ 249», «249», «N 249», «номер 249»
               find documents whose text contains «№ 249» (the requisites
               block is part of the text; N / н markers are accepted too)
  --queryfile <path>  take the query from a UTF-8 file (avoids cp1251
               console mangling of command-line arguments)
  --list <path>       write the full numbered result list (with fields)
               to a UTF-8 file instead of the console

Filters (search/export; over per-doc fields):
  --status actual|canceled|notpub|all
  --issuer <text>       contains, case-insensitive
  --type <text>         ПОСТАНОВА/ЗАКОН/УКАЗ... contains
  --date-from YYYY-MM-DD   --date-to YYYY-MM-DD
  --title <text>        contains, case-insensitive

Selection:
  --docs 130656,222-260   export exactly these docnums (works without query)
  --pick                  print listed results, type indices to export
                          (e.g. 1,3,7-9; empty line cancels)

Signal docs (export only):
  --attach                  also copy each doc's signal attachment (додаток /
                Word-вложение) from <baza>\\signal\\<docnum>.<ext> into the
                export folder and note it in the report (md/rtf/docx)
  --single (зв'язка)      all exported documents into ONE file (md/rtf/docx/
                 html/pdf; csv is already one file). The bundle is named
                 bundle-<count>-<first docnum>.<ext>; pdf gets a /Outlines
                 table of contents and each document starts on a fresh page
  --hl (підсвітити)      mark the query terms in the exported text (md:
                 <mark>; html: <mark class=hl>; rtf: yellow highlight;
                 docx: highlighted runs). The GUI has a matching checkbox

--base general  = C:\\Zakon.Local\\Baza (загальне законодавство)
--base audit    = C:\\Zakon.Local\\Baza.GK (Держаудитслужба / ГоловКРУ)
--baza <path>   override any other base flag
If the base folder is missing (moved install), the GUI asks for the folder
at startup and remembers the choice in
%APPDATA%\\zakonexport\\zakonexport.toml; CLI verbs then use that path too
unless --base/--baza is given.
Default out: C:\\temp\\opencode\\export"
    );
}

struct Options {
    verb: String,
    positional: Vec<String>,
    format: String,
    out: Option<PathBuf>,
    max: usize,
    baza: PathBuf,
    /// true when --baza/--base explicitly set the base (skip config fallback)
    baza_explicit: bool,
    guitest: bool,
    version: bool,
    mode: db::MatchMode,
    /// document-number search (№ …) instead of a text search
    number: bool,
    status: Option<String>,
    issuer: Option<String>,
    doctype: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
    title: Option<String>,
    docs: Option<String>,
    pick: bool,
    show_fields: bool,
    queryfile: Option<PathBuf>,
    listfile: Option<PathBuf>,
    /// include signal attachments (додатки / Word-вложения) in export
    attach: bool,
    /// "зв'язка": all the documents into ONE file (md/rtf/docx/html/pdf)
    single: bool,
    /// mark the query terms in the exported text (md/html/rtf/docx)
    hl: bool,
}

fn parse_args() -> Options {
    let mut o = Options {
        verb: String::new(),
        positional: Vec::new(),
        format: String::new(),
        out: None,
        max: 0,
        baza: PathBuf::from(crate::DEFAULT_BAZA),
        baza_explicit: false,
        guitest: false,
        version: false,
        mode: db::MatchMode::And,
        number: false,
        status: None,
        issuer: None,
        doctype: None,
        date_from: None,
        date_to: None,
        title: None,
        docs: None,
        pick: false,
        show_fields: false,
        queryfile: None,
        listfile: None,
        attach: false,
        single: false,
        hl: false,
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut pos = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        let next = |i: usize| args.get(i + 1).cloned();
        match a.as_str() {
            "--baza" if next(i).is_some() => {
                o.baza = PathBuf::from(next(i).unwrap());
                o.baza_explicit = true;
                i += 2;
            }
            "--base" if next(i).is_some() => {
                let sel = next(i).unwrap();
                let low = sel.to_lowercase();
                o.baza = match low.as_str() {
                    "general" | "основна" | "1" => PathBuf::from(crate::DEFAULT_BAZA),
                    "audit" | "гк" | "gk" | "2" => PathBuf::from(crate::BASE_AUDIT),
                    other => PathBuf::from(other),
                };
                o.baza_explicit = true;
                i += 2;
            }
            "--format" if next(i).is_some() => {
                o.format = next(i).unwrap().to_lowercase();
                i += 2;
            }
            "--out" if next(i).is_some() => {
                o.out = Some(PathBuf::from(next(i).unwrap()));
                i += 2;
            }
            "--max" if next(i).is_some() => {
                o.max = next(i).unwrap().parse().unwrap_or(0);
                i += 2;
            }
            "--guitest" => {
                o.guitest = true;
                i += 1;
            }
            "--version" | "-V" => {
                o.version = true;
                i += 1;
            }
            "--and" => {
                o.mode = db::MatchMode::And;
                i += 1;
            }
            "--or" => {
                o.mode = db::MatchMode::Or;
                i += 1;
            }
            "--phrase" => {
                o.mode = db::MatchMode::Phrase;
                i += 1;
            }
            "--number" => {
                o.number = true;
                i += 1;
            }
            "--status" if next(i).is_some() => {
                o.status = Some(next(i).unwrap());
                i += 2;
            }
            "--issuer" if next(i).is_some() => {
                o.issuer = Some(next(i).unwrap());
                i += 2;
            }
            "--type" if next(i).is_some() => {
                o.doctype = Some(next(i).unwrap());
                i += 2;
            }
            "--date-from" if next(i).is_some() => {
                o.date_from = Some(next(i).unwrap());
                i += 2;
            }
            "--date-to" if next(i).is_some() => {
                o.date_to = Some(next(i).unwrap());
                i += 2;
            }
            "--title" if next(i).is_some() => {
                o.title = Some(next(i).unwrap());
                i += 2;
            }
            "--docs" if next(i).is_some() => {
                o.docs = Some(next(i).unwrap());
                i += 2;
            }
            "--pick" => {
                o.pick = true;
                i += 1;
            }
            "--fields" => {
                o.show_fields = true;
                i += 1;
            }
            "--attach" | "--signal" | "--dodatk" => {
                o.attach = true;
                i += 1;
            }
            "--single" | "--zvizka" => {
                o.single = true;
                i += 1;
            }
            "--hl" | "--highlight" => {
                o.hl = true;
                i += 1;
            }
            "--queryfile" if next(i).is_some() => {
                o.queryfile = Some(PathBuf::from(next(i).unwrap()));
                i += 2;
            }
            "--list" if next(i).is_some() => {
                o.listfile = Some(PathBuf::from(next(i).unwrap()));
                i += 2;
            }
            other => {
                pos.push(other.to_string());
                i += 1;
            }
        }
    }
    o.verb = pos.first().cloned().unwrap_or_default();
    o.positional = pos.into_iter().skip(1).collect();
    o
}

fn stderr_progress(done: usize, total: usize, found: usize) -> bool {
    if done % 4 == 0 || done == total {
        eprint!("\rsearching {done}/{total} files, {found} docs ");
        let _ = std::io::stderr().flush();
    }
    true
}

// ============================================================
// helpers: base / filters / selection
// ============================================================

fn require_baza(o: &Options) -> Result<()> {
    if !o.baza.is_dir() {
        bail!("базу не знайдено: {}", o.baza.display());
    }
    Ok(())
}

/// query from --queryfile (UTF-8, immune to cp1251 console arg mangling)
/// or from the positional arguments
fn get_query(o: &Options) -> Result<String> {
    if let Some(p) = o.queryfile.as_ref() {
        let s = std::fs::read_to_string(p)
            .with_context(|| format!("read query file {}", p.display()))?;
        let s = s.trim_start_matches('\u{FEFF}').trim();
        if s.is_empty() {
            bail!("--queryfile: файл порожній");
        }
        Ok(s.to_string())
    } else {
        Ok(o.positional.join(" "))
    }
}

fn resolve_naming(o: &Options) -> naming::Naming {
    let naming = naming::Naming::load(&o.baza);
    match o.out {
        Some(ref p) => naming::Naming { out_dir: p.to_string_lossy().to_string(), ..naming },
        None => naming,
    }
}

fn any_field_filters(o: &Options) -> bool {
    o.status.is_some()
        || o.issuer.is_some()
        || o.doctype.is_some()
        || o.date_from.is_some()
        || o.date_to.is_some()
        || o.title.is_some()
}

fn parse_ymd(s: &str) -> Result<(i32, u8, u8)> {
    let parts: Vec<&str> = s.split(['-', '.', '/']).collect();
    if parts.len() != 3 {
        bail!("дата має бути YYYY-MM-DD: '{s}'");
    }
    let y: i32 = parts[0].parse().with_context(|| format!("рік: '{s}'"))?;
    let m: u8 = parts[1].parse().with_context(|| format!("місяць: '{s}'"))?;
    let d: u8 = parts[2].parse().with_context(|| format!("день: '{s}'"))?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        bail!("дата поза межами 1-12 міс. / 1-31 дня: '{s}'");
    }
    Ok((y, m, d))
}

/// Parse a comma-separated list of u64 numbers and inclusive ranges.
fn parse_num_list(s: &str) -> Result<Vec<u64>> {
    let mut out = Vec::new();
    for part in s.split(',') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        if let Some((a, b)) = p.split_once('-') {
            let a: u64 = a.trim().parse().with_context(|| format!("діапазон: '{p}'"))?;
            let b: u64 = b.trim().parse().with_context(|| format!("діапазон: '{p}'"))?;
            if b < a {
                bail!("діапазон {a}-{b}: початок > кінець");
            }
            if b - a > 1_000_000 {
                bail!("діапазон {a}-{b}: задовгий (>1 млн)");
            }
            for v in a..=b {
                out.push(v);
            }
        } else {
            out.push(p.parse().with_context(|| format!("число: '{p}'"))?);
        }
    }
    Ok(out)
}

/// Re-expand each hit's document and keep only those whose fields match
/// the requested filters. No-op when no filters are given.
fn apply_filters(o: &Options, hits: Vec<db::Hit>) -> Result<Vec<db::Hit>> {
    if !any_field_filters(o) {
        return Ok(hits);
    }
    let want = fields::parse_status_want(o.status.as_deref())
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let issuer = o.issuer.as_deref().map(|s| s.to_lowercase());
    let doctype = o.doctype.as_deref().map(|s| s.to_uppercase());
    let titleln = o.title.as_deref().map(|s| s.to_lowercase());
    let dfrom = match o.date_from.as_deref() {
        Some(s) => Some(parse_ymd(s)?),
        None => None,
    };
    let dto = match o.date_to.as_deref() {
        Some(s) => Some(parse_ymd(s)?),
        None => None,
    };
    let total = hits.len();
    let base = render::Base::load(&o.baza)?;
    let mut kept = Vec::new();
    for h in hits {
        let text = base.doc_text(&o.baza, h.docnum).unwrap_or_default();
        let f = fields::parse(h.docnum, &text);
        let mut ok = true;
        if !fields::status_pass(want, &f.status) {
            ok = false;
        }
        if ok {
            if let Some(ref n) = issuer {
                ok = f.issuer.as_deref().map_or(false, |s| s.to_lowercase().contains(n));
            }
        }
        if ok {
            if let Some(ref n) = doctype {
                ok = f.doc_type.as_deref().map_or(false, |s| s.to_uppercase().contains(n));
            }
        }
        if ok {
            if let Some(ref n) = titleln {
                ok = f.title.as_deref().map_or(false, |s| s.to_lowercase().contains(n));
            }
        }
        if ok {
            if let Some(y) = f.date_ymd {
                if let Some(a) = dfrom {
                    if y < a {
                        ok = false;
                    }
                }
                if let Some(b) = dto {
                    if y > b {
                        ok = false;
                    }
                }
            } else if dfrom.is_some() || dto.is_some() {
                ok = false; // no date -> filtered out when a date bound given
            }
        }
        if ok {
            kept.push(h);
        }
    }
    eprintln!("filtered by fields: {total} -> {}", kept.len());
    Ok(kept)
}

/// Hits for the export verb: search by query, or a direct docnum list.
fn collect_hits(o: &Options, q: &str) -> Result<Vec<db::Hit>> {
    if !q.is_empty() {
        let hits = if o.number {
            eprintln!("searching by number '{}'...", q);
            db::search_number_all(&o.baza, q, &stderr_progress)?
        } else {
            eprintln!("searching '{}' ({})...", q, o.mode.name());
            db::search_all(&o.baza, q, o.mode, &stderr_progress)?
        };
        eprintln!("\r{}", " ".repeat(48));
        eprintln!("{} docs found", hits.len());
        apply_filters(o, hits)
    } else if let Some(docs) = o.docs.clone() {
        let list = parse_num_list(&docs)?;
        let base = render::Base::load(&o.baza)?;
        let mut hits = Vec::new();
        let mut missing = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for dn in list {
            if dn > u32::MAX as u64 || !seen.insert(dn) {
                continue;
            }
            let dn = dn as u32;
            match base.span(dn) {
                Ok((_, _, file)) => hits.push(db::Hit {
                    docnum: dn,
                    file: format!("{file}.mb"),
                    match_off: 0,
                    matches: 1,
                    title: String::new(),
                }),
                Err(_) => missing.push(dn),
            }
        }
        if !missing.is_empty() {
            let shown: Vec<String> = missing.iter().take(12).map(|x| x.to_string()).collect();
            let more = if missing.len() > 12 { " …" } else { "" };
            eprintln!("недоступні/відсутні docnum: {}{more}", shown.join(", "));
        }
        apply_filters(o, hits)
    } else if o.pick {
        bail!("export: --pick потрібен запит (або --docs <docnum>)");
    } else {
        usage();
        bail!("export: вкажіть <query> або --docs <docnum,...>");
    }
}

/// Print the numbered list used by --pick selection.
fn pick_from_hits(hits: &[db::Hit]) -> Result<Vec<db::Hit>> {
    if hits.is_empty() {
        bail!("нема результатів для вибору");
    }
    println!("\nоберіть документи для експорту — введіть номери через кому (напр. 1,3,7-9);");
    println!("порожній рядок = скасувати:");
    let shown = hits.len().min(500);
    for (i, h) in hits.iter().enumerate().take(shown) {
        println!(
            "#{} [{}, doc {}] {}",
            i + 1,
            h.file,
            h.docnum,
            if h.title.is_empty() { "(без назви)" } else { h.title.as_str() }
        );
    }
    if hits.len() > shown {
        println!("… та ще {} (вводьте номери понад {shown})", hits.len() - shown);
    }
    print!("вибір: ");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    let s = line.trim();
    if s.is_empty() {
        bail!("вибір порожній — експорт скасовано");
    }
    let idxs = parse_num_list(s)?;
    let mut sel = Vec::new();
    let mut bad = 0usize;
    for i in idxs {
        if i == 0 || i as usize > hits.len() {
            bad += 1;
            continue;
        }
        sel.push(hits[i as usize - 1].clone());
    }
    if bad > 0 {
        eprintln!("поза діапазоном номерів: {bad}");
    }
    if sel.is_empty() {
        bail!("жодного валідного номеру з вибору");
    }
    Ok(sel)
}

// ============================================================
// printing
// ============================================================

fn print_hit(idx: usize, hit: &db::Hit, f: Option<&fields::DocFields>) {
    println!(
        "#{} [{}, doc {}] {} @ {}",
        idx, hit.file, hit.docnum, hit.matches, hit.match_off
    );
    if let Some(f) = f {
        let d = f.date.as_deref().unwrap_or("—");
        let n = f.number.as_deref().unwrap_or("—");
        let i = f.issuer.as_deref().unwrap_or("—");
        println!("     {d} | {} | {i} | {n}", f.status.label());
    }
    println!("     {}", hit.title);
}

fn fields_of(base: &render::Base, baza: &Path, docnum: u32) -> fields::DocFields {
    let text = base.doc_text(baza, docnum).unwrap_or_default();
    fields::parse(docnum, &text)
}

/// Full numbered result list with per-doc fields, written as UTF-8
/// (bypasses cp1251 console). Pair with export --pick: indices are the
/// 1-based positions in this list.
fn write_list_file(o: &Options, q: &str, hits: &[db::Hit], path: &Path) -> Result<()> {
    let base = render::Base::load(&o.baza)?;
    let mut s = String::new();
    use std::fmt::Write as _;
    writeln!(
        s,
        "# {} results | query: {q} | mode: {} | база: {}",
        hits.len(),
        o.mode.name(),
        o.baza.display()
    )
    .unwrap();
    for (i, h) in hits.iter().enumerate() {
        let f = fields_of(&base, &o.baza, h.docnum);
        writeln!(
            s,
            "#{} [{}, doc {}] {} @ {}",
            i + 1,
            h.file,
            h.docnum,
            h.matches,
            h.match_off
        )
        .unwrap();
        writeln!(
            s,
            "     дата: {} | {} | видавець: {} | статус: {}",
            f.date.as_deref().unwrap_or("—"),
            f.number.as_deref().unwrap_or("№ —"),
            f.issuer.as_deref().unwrap_or("—"),
            f.status.label(),
        )
        .unwrap();
        if let Some(d) = f.status_detail.as_deref() {
            writeln!(s, "     від: {d}").unwrap();
        }
        writeln!(s, "     {}", h.title).unwrap();
    }
    std::fs::write(path, s)?;
    eprintln!("list -> {}", path.display());
    Ok(())
}

// ============================================================
// main_cli
// ============================================================

pub fn main_cli() {
    let mut o = parse_args();
    if o.version {
        println!(
            "{} {}\n{}\n© {}",
            "zakonexport-cli",
            crate::VERSION,
            crate::APP_NAME,
            crate::COPYRIGHT
        );
        return;
    }
    // Moved installs: without an explicit --baza/--base the remembered config
    // path wins over the built-in default (same rule as the GUI). Only for
    // non-GUI verbs — the gui verb resolves inside run_gui (config + ask);
    // the headless guitest never touches the config.
    if !o.baza_explicit && !o.guitest && o.verb != "gui" {
        if let Some(saved) = crate::load_saved_baza() {
            o.baza = saved;
        }
    }
    let r: Result<()> = match o.verb.as_str() {
        "gui" => (|| {
            // no require_baza here: run_gui resolves the base itself
            // (remembered config, then an interactive pick when missing)
            gui::run_gui(o.baza, o.guitest, o.baza_explicit)
        })(),
        "about" => (|| {
            println!("{}", crate::about_text());
            Ok(())
        })(),
        "search" => (|| {
            require_baza(&o)?;
            let q = get_query(&o)?;
            if q.is_empty() {
                usage();
                bail!("search needs <query> or --queryfile");
            }
            let hits = if o.number {
                db::search_number_all(&o.baza, &q, &stderr_progress)?
            } else {
                db::search_all(&o.baza, &q, o.mode, &stderr_progress)?
            };
            let hits = apply_filters(&o, hits)?;
            eprintln!("\r{}", " ".repeat(48));
            if let Some(lf) = o.listfile.clone() {
                write_list_file(&o, &q, &hits, &lf)?;
                println!(
                    "{} results ({} mode); лист записано в файл",
                    hits.len(),
                    o.mode.name()
                );
                return Ok(());
            }
            let mode_disp = if o.number { "№" } else { o.mode.name() };
            println!("{} results for '{}' ({})\n", hits.len(), q, mode_disp);
            let want_fields = o.show_fields || any_field_filters(&o);
            let base = if want_fields {
                Some(render::Base::load(&o.baza)?)
            } else {
                None
            };
            let limit = if o.max > 0 { o.max } else { 50 };
            for (i, h) in hits.iter().enumerate().take(limit) {
                let f = base.as_ref().map(|b| fields_of(b, &o.baza, h.docnum));
                print_hit(i + 1, h, f.as_ref());
            }
            if hits.len() > limit {
                println!(
                    "… і ще {} (використайте --max, --list або фільтри)",
                    hits.len() - limit
                );
            }
            Ok(())
        })(),
        "export" => (|| {
            require_baza(&o)?;
            let q = get_query(&o)?;
            let hits = collect_hits(&o, &q)?;
            let sel = if o.docs.is_some() && !o.pick {
                let docs = o.docs.clone().unwrap();
                let keep: std::collections::HashSet<u64> =
                    parse_num_list(&docs)?.into_iter().collect();
                let before = hits.len();
                let sel: Vec<db::Hit> =
                    hits.into_iter().filter(|h| keep.contains(&(h.docnum as u64))).collect();
                eprintln!("selection --docs: {before} -> {}", sel.len());
                sel
            } else if o.pick {
                if let Some(lf) = o.listfile.clone() {
                    write_list_file(&o, &q, &hits, &lf)?;
                }
                pick_from_hits(&hits)?
            } else {
                hits
            };
            if sel.is_empty() {
                bail!("після вибору нічого експортувати");
            }
            let naming = resolve_naming(&o);
            let hl = if o.hl {
                crate::highlight::Hl::from_query(&q)
            } else {
                crate::highlight::Hl::default()
            };
            let (n, a, out) =
                writers::export_hits(&o.baza, &sel, &o.format, &naming, Some(&|d, t| {
                    eprint!("\rexport {d}/{t}  ");
                    let _ = std::io::stderr().flush();
                }), o.attach, o.single, &hl)?;
            eprintln!();
            if o.single {
                if o.attach {
                    println!("зв'язка: {n} документи -> 1 файл, {a} додатків -> {}", out.display());
                } else {
                    println!("зв'язка: {n} документи -> 1 файл: {}", out.display());
                }
            } else if o.attach {
                println!("exported {n} files, {a} signal attachments -> {}", out.display());
            } else {
                println!("exported {n} files to {}", out.display());
            }
            Ok(())
        })(),
        "bulk" => (|| {
            require_baza(&o)?;
            eprintln!("loading base index...");
            let base = render::Base::load(&o.baza)?;
            let total_docs = base.doc_count();
            eprintln!("{total_docs} documents");
            let take = if o.max > 0 { o.max.min(total_docs as usize) } else { total_docs as usize };
            let all: Vec<db::Hit> = (0..take as u32)
                .map(|d| {
                    let file = base.span(d).map(|(_, _, f)| f).unwrap_or(0);
                    db::Hit {
                        docnum: d,
                        file: format!("{file}.mb"),
                        match_off: 0,
                        matches: 1,
                        title: String::new(),
                    }
                })
                .collect();
            let naming = resolve_naming(&o);
            // bulk has no query to highlight — the term set stays empty
            let hl = crate::highlight::Hl::default();
            let (n, a, out) =
                writers::export_hits(&o.baza, &all, &o.format, &naming, Some(&|d, t| {
                    eprint!("\rexport {d}/{t}   ");
                    let _ = std::io::stderr().flush();
                }), o.attach, o.single, &hl)?;
            eprintln!();
            if o.single {
                println!("зв'язка: {n} документи -> 1 файл: {}", out.display());
            } else if o.attach {
                println!("exported {n} files, {a} signal attachments -> {}", out.display());
            } else {
                println!("exported {n} files to {}", out.display());
            }
            Ok(())
        })(),
        "signal" => (|| {
            require_baza(&o)?;
            if o.positional.is_empty() {
                bail!("signal needs <docnum>");
            }
            let docnum: u32 = o.positional[0].parse().with_context(|| "docnum must be a number")?;
            match crate::signal::find_signal(&o.baza, docnum) {
                Some(p) => {
                    let sz = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                    println!("doc {docnum}: сигнал знайдено -> {} ({} Б)", p.display(), sz);
                }
                None => {
                    println!(
                        "doc {docnum}: сигнальний документ відсутній ({}\\signal\\)",
                        o.baza.display()
                    );
                }
            }
            Ok(())
        })(),
        "extract" => (|| {
            require_baza(&o)?;
            if o.positional.is_empty() {
                usage();
                bail!("extract needs <docnum>");
            }
            let docnum: u32 = o.positional[0].parse().with_context(|| "docnum must be a number")?;
            let base = render::Base::load(&o.baza)?;
            let text = base.doc_text(&o.baza, docnum)?;
            println!("{text}");
            Ok(())
        })(),
        "fields" => (|| {
            require_baza(&o)?;
            if o.positional.is_empty() {
                usage();
                bail!("fields needs <docnum>");
            }
            let docnum: u32 = o.positional[0].parse().with_context(|| "docnum must be a number")?;
            let base = render::Base::load(&o.baza)?;
            let text = base.doc_text(&o.baza, docnum).unwrap_or_default();
            if text.is_empty() {
                eprintln!("(текст документа порожній/недоступний)");
            }
            let f = fields::parse(docnum, &text);
            println!("{}", f.display());
            Ok(())
        })(),
        "print" => (|| {
            // GDI print: system print dialog + a printer DC (no Word / no
            // browser). Formatting is preserved: ALL-CAPS lines bold, list
            // items "•  ", centered lines centered. BATCH: several docnums
            // (space- or comma-separated, ranges allowed) print one after
            // another (the print dialog appears per document).
            require_baza(&o)?;
            if o.positional.is_empty() {
                usage();
                bail!("print needs <docnum> [docnum ...]");
            }
            let docnums = parse_num_list(&o.positional.join(","))?;
            let base = render::Base::load(&o.baza)?;
            for d in docnums.iter() {
                let docnum = *d as u32;
                let text = base.doc_text(&o.baza, docnum).unwrap_or_default();
                let f = fields::parse(docnum, &text);
                let full = format!("{}\n\n{}", f.display(), text);
                match crate::printdoc::print_text(&format!("Документ {docnum}"), &full) {
                    Ok(crate::printdoc::PrintOutcome::Sent) => {
                        println!("doc {docnum}: завдання друку надіслано в принтер");
                    }
                    Ok(crate::printdoc::PrintOutcome::Cancelled) => {
                        println!("doc {docnum}: друк відмінено");
                    }
                    Err(e) => {
                        eprintln!("doc {docnum}: помилка друку: {e}");
                    }
                }
            }
            Ok(())
        })(),
        "stats" => (|| {
            // coverage diagnostic: expand every document, bucket failures
            require_baza(&o)?;
            let base = render::Base::load(&o.baza)?;
            let total = base.doc_count();
            let mut ok = 0usize;
            let mut empty = 0usize;
            let mut modes: HashMap<u8, usize> = HashMap::new();
            let mut empty_modes: HashMap<u8, usize> = HashMap::new();
            let mut empty_samples: HashMap<u8, Vec<u32>> = HashMap::new();
            let mut errs: HashMap<String, usize> = HashMap::new();
            let mut cache: HashMap<u16, Vec<u8>> = HashMap::new();
            for d in 0..total {
                let (mode, chunk, file) = base.span(d)?;
                *modes.entry(mode).or_default() += 1;
                let expand = (|| -> anyhow::Result<std::vec::Vec<u8>> {
                    let data = match cache.get(&file) {
                        Some(v) => v,
                        None => {
                            let p = o.baza.join("text").join(format!("{file}.mb"));
                            let v = std::fs::read(&p)
                                .with_context(|| format!("read {}", p.display()))?;
                            cache.insert(file, v);
                            cache.get(&file).unwrap()
                        }
                    };
                    let cb = render::chunk_slice(data, chunk as u32)?;
                    render::expand_chunk(mode, cb, base.words())
                })();
                match expand {
                    Err(e) => {
                        let key = format!("{e}")
                            .split(|c| c == ':' || c == '(')
                            .next()
                            .unwrap_or("")
                            .to_string();
                        *errs.entry(key).or_default() += 1;
                    }
                    Ok(exp) => {
                        if exp.is_empty() {
                            empty += 1;
                            *empty_modes.entry(mode).or_default() += 1;
                            let sv = empty_samples.entry(mode).or_default();
                            if sv.len() < 4 {
                                sv.push(d);
                            }
                        } else {
                            ok += 1;
                        }
                    }
                }
                if d % 200_000 == 0 {
                    eprintln!("d={d}/{total}  ");
                    let _ = std::io::stderr().flush();
                }
            }
            let esum: usize = errs.values().sum();
            println!(
                "docs: {total}  ok: {ok}  empty: {empty}  errs: {esum}  \
                 sum: {}  (мусорных нет, если ok+empty+errs == docs)",
                ok + empty + esum
            );
            let mut ms: Vec<_> = modes.into_iter().collect();
            ms.sort_by_key(|x| x.0);
            for (m, n) in ms {
                let em = *empty_modes.get(&m).unwrap_or(&0);
                println!("  mode {m}: {n} (empty {em}, ok {})", n - em);
                if let Some(sv) = empty_samples.get(&m) {
                    if !sv.is_empty() {
                        println!("    empty sample docs: {}", sv.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", "));
                    }
                }
            }
            let mut es: Vec<_> = errs.into_iter().collect();
            es.sort_by_key(|x| std::cmp::Reverse(x.1));
            for (k, n) in es.iter().take(14) {
                println!("  err [{k}]: {n}");
            }
            Ok(())
        })(),
        "trident" => (|| {
            // The legacy main app's native FTS indexes (SOFT\): the 16-file
            // inventory plus the parts decoded today — the windows.kod code
            // table, the lm.a case-fold tables, the dictionary records and
            // the u16 posting stream (src/trident.rs).
            require_baza(&o)?;
            let soft = o.baza.join("SOFT");
            if !soft.is_dir() {
                println!(
                    "В базі {} немає Trident-індексів (папки SOFT\\) — пошук \
                     виконується власним рушієм експортера.",
                    o.baza.display()
                );
                return Ok(());
            }
            println!("Trident-індекси в {}: {}", o.baza.display(), soft.display());
            for (name, role) in crate::trident::INVENTORY {
                let p = soft.join(name);
                match std::fs::metadata(&p) {
                    Ok(m) => println!("  {name:<14} {:>9} Б  ({role})", m.len()),
                    Err(_) => println!("  {name:<14} —          (відсутній)"),
                }
            }
            println!();
            if let Some(idx) = crate::trident::TridentIndex::open(&o.baza) {
                let vt_min = idx.vt_u16.iter().copied().min();
                let vt_max = idx.vt_u16.iter().copied().max();
                println!(
                    "windows.kod: {} кодів · lm.a: fold-пари R={} U={} · \
                     словник lmu.voc: {} записів · vt-потік: {} u16 \
                     (документи {}..{})",
                    idx.kod.len(),
                    idx.fold.rus.len(),
                    idx.fold.ukr.len(),
                    idx.dict_words.len(),
                    idx.vt_u16.len(),
                    vt_min.map(|x| x.to_string()).unwrap_or("-".into()),
                    vt_max.map(|x| x.to_string()).unwrap_or("-".into()),
                );
                // the exact .voc morphology tables (v0.17.0)
                if let Some(m) = &idx.morphology {
                    let t = &m.table;
                    println!(
                        "lmu.voc секції: root=0x{:x} listA=0x{:x}({}) listB=0x{:x}({}) \
                         f-записи=0x{:x}({}) суфікси=0x{:x}",
                        t.root,
                        t.list_a,
                        m.list_a_count,
                        t.list_b,
                        m.list_b_count,
                        t.per_word_f,
                        m.per_word_f_count,
                        t.suffixes,
                    );
                    if !m.affixes.is_empty() {
                        let affixes: Vec<String> = m
                            .affixes
                            .iter()
                            .map(|w| format!("{}({})", w.text(), w.class))
                            .collect();
                        println!(
                            "  афікси ({}): {}",
                            m.affixes.len(),
                            affixes.join(", ")
                        );
                    }
                    if !m.suffixes.is_empty() {
                        let shown: Vec<String> = m
                            .suffixes
                            .iter()
                            .take(12)
                            .map(|w| crate::trident_voc::lm_word_display(w))
                            .collect();
                        println!(
                            "  суфікси/кінцівки ({}): {}…",
                            m.suffixes.len(),
                            shown.join(", ")
                        );
                    }
                    if !m.digraphs.is_empty() {
                        println!(
                            "  двозначники ({}): {}",
                            m.digraphs.len(),
                            m.digraphs.join(", ")
                        );
                    }
                    // the FUN_10004230 record walks (v0.18.0): per-section
                    // record counts, landing offsets and resync jumps
                    if !m.walks.is_empty() {
                        println!(
                            "  записники (FUN_10004230, блок {}):",
                            crate::trident_voc::WALK_BLOCK
                        );
                        for (slot, w) in &m.walks {
                            let mode = match crate::trident_voc::section_mode(*slot) {
                                Some(0xff) => "ff".to_string(),
                                Some(m) => (m as char).to_string(),
                                None => "?".to_string(),
                            };
                            println!(
                                "    +{slot:02x} '{mode}' @0x{:x}: {} записів → 0x{:x} \
                                 ({} block-resync)",
                                m.table.slot_offset(*slot),
                                w.count,
                                w.landed,
                                w.block_jumps
                            );
                        }
                    }
                }
                // the exact `.vt` record stream + `.m` metadata header (v0.19.0)
                if let Some(s) = &idx.vt_stream {
                    let runs: usize = s
                        .records
                        .iter()
                        .map(|r| r.latin_runs.len())
                        .sum();
                    let u16s: usize = s.records.iter().map(|r| r.u16_words.len()).sum();
                    println!(
                        "lmur.vt: {} — {} record-ів (запис [u8 count][payload][NUL], \
                         0x40..0x{:x}) · {} Latin-run-ів · {} u16-слів",
                        s.header.name,
                        s.records.len(),
                        s.end,
                        runs,
                        u16s,
                    );
                }
                if let Some(m) = &idx.vm_header {
                    let table: Vec<String> =
                        m.table.iter().map(|v| format!("0x{v:x}")).collect();
                    println!(
                        "lmur.m: {} — метадані за словами (0xE-структури \
                         FUN_100177d0) · u32-таблиця @0x40: {} · хвіст @0x{:x}",
                        m.header.name,
                        table.join(" "),
                        m.header.tail,
                    );
                }
                if let Some(p) = idx.morphology.as_ref().and_then(|m| m.params) {
                    println!(
                        "lmu.voc параметри @0x80: stop=0x{:02x} блок=0x{:x} \
                         sec_54={} gw={}",
                        p.stop_marker(),
                        p.page_const(),
                        p.sec_54_count(),
                        p.gw_size()
                    );
                }
                // the `.g` grammar graph (structural decode) + `.gw` weights
                if let Some(g) = &idx.graph {
                    let head: Vec<String> = g
                        .stream_head
                        .iter()
                        .take(8)
                        .map(|v| format!("{v:04x}"))
                        .collect();
                    let (gw_n, gw_flag) = (idx.gw_u16.len(), {
                        idx.gw_u16.iter().filter(|v| *v & 0x8000 != 0).count()
                    });
                    println!(
                        "lmu.g граф: lang={} template={} stream@0x{:x} ({} u16) · \
                         маркери: {} · lmu.gw: {} u16 ({} прап. 0x8000)",
                        g.language,
                        g.template,
                        g.stream_off,
                        g.stream_u16,
                        if g.tail_markers.is_empty() {
                            "—".to_string()
                        } else {
                            g.tail_markers.join(" ")
                        },
                        gw_n,
                        gw_flag
                    );
                    println!("  граф head: {}…", head.join(" "));
                }
            }
            println!();
            println!(
                "Пошук нативним індексом — tsearch (експеримент); морфологія \
                 lmu.voc (афікси, суфікси/кінцівки, двозначники, таблиця секцій) — \
v0.17.0; записники lmu.voc (FUN_10004230 record-walker: кордони record-ів + \
block-resync packed B-tree) — v0.18.0; граф lmu.g — структурно декодовано, \
v0.19.0: точний stem-join word→.vt (posting record stream lmur.vt + header \
                 lmur.m + параметри lmu.voc @0x80); семантика ребер lmu.g + \
                 B-tree-VM хвоста (FUN_100019f0) — далі (v0.20). \
                 Повнопошуковий пошук — search/export (власний fold-codec FTS)."
            );
            Ok(())
        })(),
        "tsearch" => (|| {
            // Experimental native-index search: encode the query through the
            // LmCode character table, look the terms up in the lmu.voc
            // dictionary, read posting slices from lmur.vt. The document
            // numbers are the native Trident ids of the 2005-era index; the
            // join to the current base docnums is a later version.
            require_baza(&o)?;
            let q = get_query(&o)?;
            if q.is_empty() {
                usage();
                bail!("tsearch needs <query> or --queryfile");
            }
            let Some(idx) = crate::trident::TridentIndex::open(&o.baza) else {
                println!(
                    "В базі {} немає робочого Trident-індексу (SOFT\\lmu.voc + \
                     lmur.vt) — пошук: search.",
                    o.baza.display()
                );
                return Ok(());
            };
            let r = idx.search(&q);
            let pct = (r.coverage * 100.0).round() as u32;
            println!("Trident tsearch (експеримент): «{q}»");
            println!(
                "  словник: {} слів · vt-потік: {} u16 · покриття знаків: {}% · \
                 знайдено термінів: {}",
                r.dict_size,
                r.vt_size,
                pct,
                r.matched.len()
            );
            // Show how each term encodes into LmCode (the full cp1251 table):
            //   термін → hex-байти → decode (what those bytes mean).
            for (term, bytes) in idx.encode_query(&q) {
                let hex = bytes
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                let decoded = crate::trident::decode_lm_word(&bytes);
                println!("  код: «{term}» → [{hex}] → «{decoded}»");
            }
            if !r.matched.is_empty() {
                println!("  збіг: {}", r.matched.join(", "));
            }
            // v0.19.0: the exact stem join — the case-folded Latin
            // candidates of the terms matched in the `.vt` record stream
            if !r.joined.is_empty() {
                println!(
                    "  join word→.vt (v0.19.0, точний stem-join): {} record-ів:",
                    r.joined.len()
                );
                for (cand, off, u16s) in r.joined.iter().take(12) {
                    let shown: Vec<String> =
                        u16s.iter().take(8).map(|u| u.to_string()).collect();
                    println!(
                        "    «{cand}» → record @0x{off:x} ({} u16: {}{})",
                        u16s.len(),
                        shown.join(" "),
                        if u16s.len() > 8 { "…" } else { "" }
                    );
                }
                if r.joined.len() > 12 {
                    println!("    … (ще {})", r.joined.len() - 12);
                }
            }
            if r.docs.is_empty() {
                println!(
                    "  результат: 0 документів (точні word→postings кордони та \
                     join native→docnum — далі в roadmap)"
                );
            } else {
                let shown = r
                    .docs
                    .iter()
                    .take(40)
                    .map(|d| d.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                println!(
                    "  результат: {} Trident-документів (native IDs 2005-індексу, \
                     показано {}):",
                    r.docs.len(),
                    shown.len()
                );
                println!("    {shown}");
                if r.docs.len() > 40 {
                    println!("    … (ще {})", r.docs.len() - 40);
                }
            }
            Ok(())
        })(),
        "ftsearch" => (|| {
            // Full-text scan of the base's .mb document files — the legacy
            // app's oracle mechanism. Encode the query through the base
            // codepage, scan every .mb (lower + title case), report the
            // (file @ offset) hits in the oracle's shape. No B-tree needed.
            require_baza(&o)?;
            let q = get_query(&o)?;
            if q.is_empty() {
                usage();
                bail!("ftsearch needs <query> or --queryfile");
            }
            let r = crate::ftsearch::ft_search(&o.baza, &q);
            let pct = (r.coverage() * 100.0).round() as u32;
            println!("FT full-text scan (oracle mechanism): «{q}»");
            println!(
                "  files: {} .mb · coverage: {}% · hits: {}",
                r.files_scanned,
                pct,
                r.hits.len()
            );
            if r.hits.is_empty() {
                println!("  результат: 0 збігів");
            } else {
                let shown = r
                    .hits
                    .iter()
                    .take(40)
                    .map(|h| format!("{} @ {}", h.file.to_string_lossy(), h.offset))
                    .collect::<Vec<_>>()
                    .join(", ");
                println!("  збіги (показано {}):", shown.split(", ").count());
                println!("    {shown}");
                if r.hits.len() > 40 {
                    println!("    … (ще {})", r.hits.len() - 40);
                }
            }
            Ok(())
        })(),
        _ => {
            usage();
            Ok(())
        }
    };
    if let Err(e) = r {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
