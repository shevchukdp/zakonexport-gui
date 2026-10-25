//! Per-document field extraction from the rendered (tidy) text:
//! видавець, тип, дата, номер, назва, статус (чинний / втрачено / не опубл.).
//!
//! Intentionally derived from what the app displays rather than from the
//! binary metadata DBs (see REVERSE.md). Headers are often letter-spaced
//! ("П О С Т А Н О В А"), so matching compares space-stripped keys, and the
//! unsquash helper restores word boundaries for spaced lines.

// ============================================================
// types
// ============================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// текст є, скасування ненайдено
    Actual,
    /// текст є, есть помітки про внесення змін
    Amended,
    /// не діє: втрачено чинність / знято / скасовано
    Canceled,
    /// опублікуванню не підлягає
    NotPublished,
    /// порожній/недоступний текст
    NoText,
}

impl Status {
    pub fn label(&self) -> &'static str {
        match self {
            Status::Actual => "чинний",
            Status::Amended => "чинний (зі змінами)",
            Status::Canceled => "не діє",
            Status::NotPublished => "не опубліковано",
            Status::NoText => "—",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DocFields {
    pub docnum: u32,
    pub issuer: Option<String>,
    /// kicker: ПОСТАНОВА / ЗАКОН / УКАЗ / НАКАЗ …
    pub doc_type: Option<String>,
    /// DD.MM.YYYY
    pub date: Option<String>,
    pub date_ymd: Option<(i32, u8, u8)>,
    /// "№ 145"
    pub number: Option<String>,
    /// "Про …" line
    pub title: Option<String>,
    pub status: Status,
    pub status_detail: Option<String>,
    pub n_lines: usize,
    pub n_chars: usize,
}

impl DocFields {
    pub fn display(&self) -> String {
        let mut s = format!(
            "документ: {}\nвидавець: {}\nтип: {}\nдата: {}\nномер: {}\nназва: {}\nстатус: {}",
            self.docnum,
            dash2(self.issuer.as_deref()),
            dash2(self.doc_type.as_deref()),
            dash2(self.date.as_deref()),
            dash2(self.number.as_deref()),
            dash2(self.title.as_deref()),
            self.status.label(),
        );
        if let Some(d) = self.status_detail.as_deref() {
            s.push_str(&format!("\nстатус від: {d}"));
        }
        s.push_str(&format!("\nобсяг: {} ряд., {} симв.", self.n_lines, self.n_chars));
        s
    }
}

// ============================================================
// helpers
// ============================================================

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Restore word boundaries in a letter-spaced line:
/// "К А Б І Н Е Т   М І Н І С Т Р І В" -> "КАБІНЕТ МІНІСТРІВ".
/// One space continues a word; 2+ spaces separate words.
fn unsquash(line: &str) -> String {
    let t = line.trim();
    if t.is_empty() {
        return String::new();
    }
    let toks: Vec<&str> = t.split_whitespace().collect();
    let single = toks.iter().filter(|x| x.chars().count() == 1).count();
    if toks.len() < 4 || single * 2 < toks.len() - 1 {
        return collapse_ws(t);
    }
    let mut words: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut gap = 0usize;
    for ch in t.chars() {
        if ch == ' ' || ch == '\t' {
            gap += 1;
            continue;
        }
        if gap >= 2 && !cur.is_empty() {
            words.push(std::mem::take(&mut cur));
        }
        cur.push(ch);
        gap = 0;
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    words.join(" ")
}

/// space-stripped, uppercase key for fuzzy startswith/equals matching
fn upkey(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_uppercase)
        .collect()
}

/// trim a value line to a printable bound
fn clip(s: &str, max: usize) -> String {
    let c: String = s.chars().take(max).collect();
    c.trim_end().to_string()
}

/// option -> "—" for display
fn dash2(o: Option<&str>) -> &str {
    o.unwrap_or("—")
}

// ============================================================
// issuer / type kickers
// ============================================================

/// (prefix-key, canonical name) — checked in order against unspaced keys of the
/// unsquashed first-line candidates.
const ISSUER_CANON: &[(&str, &str)] = &[
    ("КАБІНЕТМІНІСТРІВУКРАЇНИ", "Кабінет Міністрів України"),
    ("ВЕРХОВНАРАДАУКРАЇНИ", "Верховна Рада України"),
    ("ПРЕЗИДЕНТУКРАЇНИ", "Президент України"),
    ("НАЦІОНАЛЬНИЙБАНКУКРАЇНИ", "Національний банк України"),
    (
        "ВЕРХОВНАРАДААВТОНОМНОЇРЕСПУБЛІКИКРИМ",
        "Верховна Рада АР Крим",
    ),
    ("РАДАМІНІСТРІВАВТОНОМНОЇРЕСПУБЛІКИКРИМ", "Рада міністрів АР Крим"),
    ("ФОНДДЕРЖАВНОГОМАЙНАУКРАЇНИ", "Фонд державного майна України"),
    ("АНТИМОНОПОЛЬНИЙКОМІТЕТУКРАЇНИ", "Антимонопольний комітет України"),
];

/// prefixes whose full name continues on the line (emit the line itself)
const ISSUER_PREFIX: &[&str] = &[
    "МІНІСТЕРСТВО",
    "ДЕРЖАВНИЙКОМІТЕТ",
    "ДЕРЖАВНАКОМІСІЯ",
    "ДЕРЖАВНЕАГЕНТСТВО",
    "ДЕРЖАВНАСУДОВААДМІНІСТРАЦІЯ",
    "ДЕРЖАВНАКАЗНАЧЕЙСЬКА",
    "ДЕРЖАВНАФІСКАЛЬНА",
    "ДЕРЖАВНАПОДАТКОВА",
    "ДЕРЖАВНААУДИТСЛУЖБА",
    "ДЕРЖАВНЕКАЗНАЧЕЙСТВО",
    "НАЦІОНАЛЬНАКОМІСІЯ",
    "НАЦІОНАЛЬНЕАГЕНТСТВО",
    "НАЦІОНАЛЬНАСЛУЖБА",
    "ВЕРХОВНИЙСУД",
    "ТИМЧАСОВАСПЕЦІАЛЬНА",
];

const KICKERS: &[&str] = &[
    "ПОСТАНОВА",
    "РОЗПОРЯДЖЕННЯ",
    "ЗАКОН",
    "УКАЗ",
    "НАКАЗ",
    "РІШЕННЯ",
    "ПОЛОЖЕННЯ",
    "ІНСТРУКЦІЯ",
    "ПЕРЕЛІК",
    "ЛИСТ",
    "ЛІСТ",
    "ПОВІДОМЛЕННЯ",
    "ДОВІДКА",
    "КОДЕКС",
    "КОНВЕНЦІЯ",
    "ЗАЯВА",
    "ВЕРХОВНИЙСУД",
    "ПОЯСНЕННЯ",
    "ЗВІТ",
    "ПРОТОКОЛ",
];

fn detect_issuer(head: &[&str]) -> Option<String> {
    for line in head {
        let s = unsquash(line);
        let k = upkey(&s);
        if k.is_empty() {
            continue;
        }
        for (pfx, canon) in ISSUER_CANON {
            if k.starts_with(pfx) {
                return Some(clip(canon, 96));
            }
        }
        for pfx in ISSUER_PREFIX {
            if k.starts_with(pfx) {
                return Some(clip(&s, 120));
            }
        }
    }
    None
}

fn detect_kicker(head: &[&str]) -> Option<String> {
    for line in head {
        let k = upkey(line);
        for kk in KICKERS {
            if k == *kk {
                return Some((*kk).to_string());
            }
        }
    }
    None
}

// ============================================================
// date / number
// ============================================================

fn month_num(w: &str) -> Option<u8> {
    let m = match w {
        "січня" => 1,
        "лютого" => 2,
        "березня" => 3,
        "квітня" => 4,
        "травня" => 5,
        "червня" => 6,
        "липня" => 7,
        "серпня" => 8,
        "вересня" => 9,
        "жовтня" => 10,
        "листопада" => 11,
        "грудня" => 12,
        _ => return None,
    };
    Some(m)
}

fn numeric_date(tok: &str) -> Option<(i32, u8, u8)> {
    let parts: Vec<&str> = tok.split(['.', '-', '/']).collect();
    if parts.len() != 3 {
        return None;
    }
    let d: u32 = parts[0].parse().ok()?;
    let m: u32 = parts[1].parse().ok()?;
    let y: i32 = parts[2].parse().ok()?;
    if (1..=31).contains(&d) && (1..=12).contains(&m) && (1000..2200).contains(&y) {
        // проти помилок виду 990.02.2004 у "датах" реєстрів
        if d as i32 > 31 {
            return None;
        }
        Some((y, m as u8, d as u8))
    } else {
        None
    }
}

/// find the first date in the header: "від 11 лютого 2004 р." pattern first,
/// numeric "19.12.2000" fallback
fn detect_date(head: &[&str]) -> Option<(String, (i32, u8, u8))> {
    for line in head {
        let low = line.to_lowercase();
        let toks: Vec<String> = low.split_whitespace().map(|s| s.to_string()).collect();
        for (i, tok) in toks.iter().enumerate() {
            let clean = tok
                .trim_matches(|c: char| ".,;:«»()".contains(c))
                .to_string();
            if let Some(m) = month_num(&clean) {
                // day = previous token, year = next token
                if i > 0 {
                    if let Ok(d) = toks[i - 1]
                        .trim_matches(|c: char| ",.:;".contains(c))
                        .parse::<u32>()
                    {
                        if (1..=31).contains(&d) {
                            for ytok in toks.iter().skip(i + 1).take(2) {
                                let y = ytok.trim_matches(|c: char| ".,;".contains(c));
                                if let Some(y) = y.parse::<i32>().ok().filter(|y| (1000..2200).contains(y)) {
                                    let ds = format!("{:02}.{:02}.{:04}", d, m, y);
                                    return Some((ds, (y, m, d as u8)));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    // numeric fallback
    for line in head {
        for tok in line.split_whitespace() {
            let clean = tok.trim_matches(|c: char| ".,;:«»()".contains(c));
            if let Some(ymd) = numeric_date(clean) {
                return Some((
                    format!("{:02}.{:02}.{:04}", ymd.2, ymd.1, ymd.0),
                    ymd,
                ));
            }
        }
    }
    None
}

fn detect_number(head: &[&str]) -> Option<String> {
    // pass 1: "№ 145" / "№145" / "№№ 63" (double-sign variant used in some bases)
    for line in head {
        let c = collapse_ws(line);
        if let Some(i) = c.find('№') {
            // skip additional № signs, then spaces, before the token
            let bytes: Vec<char> = c.chars().collect();
            let mut k = c[..i].chars().count() + 1;
            while k < bytes.len() && (bytes[k] == '№' || bytes[k] == ' ') {
                k += 1;
            }
            if let Some(tok) = num_token_start(&bytes[k..]) {
                return Some(format!("№ {tok}"));
            }
        }
    }
    // pass 2: "… р. N 386" (Latin N followed by digits, not inside a word)
    for line in head {
        let c = collapse_ws(line);
        let chars: Vec<char> = c.chars().collect();
        for i in 0..chars.len() {
            if chars[i] == 'N' || chars[i] == 'n' {
                let prev_is_word = i > 0 && chars[i - 1].is_alphanumeric();
                if prev_is_word {
                    continue;
                }
                if let Some(tok) = num_token_start(&chars[i + 1..]) {
                    return Some(format!("№ {tok}"));
                }
            }
        }
    }
    None
}

/// Parse a number token from the start of `rest` (spaces skipped first):
/// digits, optionally extended with "-XIV" / "/2" segments.
fn num_token_start(rest: &[char]) -> Option<String> {
    let n = rest.len();
    let mut i = 0usize;
    let mut out = String::new();
    while i < n && rest[i] == ' ' {
        i += 1;
    }
    let mut have_digit = false;
    while i < n && rest[i].is_ascii_digit() {
        out.push(rest[i]);
        i += 1;
        have_digit = true;
    }
    if !have_digit {
        return None;
    }
    loop {
        if i < n && (rest[i] == '-' || rest[i] == '/') {
            let mut j = i + 1;
            while j < n && rest[j].is_alphanumeric() {
                j += 1;
            }
            if j > i + 1 {
                for k in i..j {
                    out.push(rest[k]);
                }
                i = j;
                continue;
            }
        }
        break;
    }
    Some(out)
}

// ============================================================
// title / status
// ============================================================

fn detect_title(head: &[&str]) -> Option<String> {
    for line in head {
        let t = collapse_ws(line);
        if t.chars().count() > 6 && t.to_lowercase().starts_with("про ") {
            return Some(clip(&t, 200));
        }
    }
    None
}

const STATUS_NOTPUB: &str = "опублікуванню не підлягає";

const STATUS_CANCELED: &[&str] = &[
    "втратив чинність",
    "втратила чинність",
    "втратили чинність",
    "відмінено",
    "скасовано доручення", // preserving narrower needles for body context
    "визнано недійсним",
    "не набрав чинності",
];

const CANCELED_ANCHOR: &[&str] = &[
    "постанов",
    "наказ",
    "указ",
    "розпорядж",
    "рішенням",
    "рішення",
    "законом",
    "листом",
];

const STATUS_AMENDED: &[&str] = &["внесено зміни", "змінено", "у редакції"];

/// scan from the end: the keyword/status card of the app is appended last
fn detect_status(text: &str) -> (Status, Option<String>) {
    if text.is_empty() {
        return (Status::NoText, None);
    }
    for line in text.lines().rev() {
        let lc = line.to_lowercase();
        if lc.contains(STATUS_NOTPUB) {
            return (Status::NotPublished, Some(clip(&collapse_ws(line), 160)));
        }
        if lc.contains("знято")
            && CANCELED_ANCHOR.iter().any(|a| lc.contains(a))
        {
            return (Status::Canceled, Some(clip(&collapse_ws(line), 160)));
        }
        if STATUS_CANCELED.iter().any(|n| lc.contains(n)) {
            return (Status::Canceled, Some(clip(&collapse_ws(line), 160)));
        }
        if STATUS_AMENDED.iter().any(|n| lc.contains(n)) {
            return (Status::Amended, Some(clip(&collapse_ws(line), 160)));
        }
    }
    (Status::Actual, None)
}

// ============================================================
// main entry
// ============================================================

/// Parse fields from the rendered text of one document.
pub fn parse(docnum: u32, text: &str) -> DocFields {
    let lines: Vec<&str> = text.lines().collect();
    let head_n = lines.len().min(14);
    let head = &lines[..head_n];
    let (status, detail) = if text.is_empty() {
        (Status::NoText, None)
    } else {
        detect_status(text)
    };
    let (date, date_ymd) = match detect_date(head) {
        Some((s, ymd)) => (Some(s), Some(ymd)),
        None => (None, None),
    };
    DocFields {
        docnum,
        issuer: detect_issuer(&lines[.. head_n.min(8)]),
        doc_type: detect_kicker(head),
        date,
        date_ymd,
        number: detect_number(head),
        title: detect_title(head),
        status,
        status_detail: detail,
        n_lines: lines.len(),
        n_chars: text.chars().count(),
    }
}

// ============================================================
// status filter vocabulary (shared with the CLI)
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusWant {
    All,
    /// чинний / зі змінами
    InForce,
    Canceled,
    NotPublished,
}

pub fn parse_status_want(s: Option<&str>) -> Result<StatusWant, String> {
    let v = s.unwrap_or("").to_lowercase();
    match v.as_str() {
        "" | "all" => Ok(StatusWant::All),
        "actual" | "inforce" | "чинний" => Ok(StatusWant::InForce),
        "canceled" | "не діє" | "скасовано" => Ok(StatusWant::Canceled),
        "notpub" | "не опубліковано" => Ok(StatusWant::NotPublished),
        other => Err(format!(
            "--status: unknown value '{other}' (actual|canceled|notpub|all)"
        )),
    }
}

pub fn status_pass(w: StatusWant, s: &Status) -> bool {
    match w {
        StatusWant::All => true,
        StatusWant::InForce => matches!(s, Status::Actual | Status::Amended),
        StatusWant::Canceled => *s == Status::Canceled,
        StatusWant::NotPublished => *s == Status::NotPublished,
    }
}

// ============================================================
// tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    const KMU_TEXT: &str = "К А Б І Н Е Т   М І Н І С Т Р І В   У К Р А Ї Н И\n\
П О С Т А Н О В А\n\
від 11 лютого 2004 р. № 145\n\
Про продовження строку дії постанови Кабінету Міністрів України від 26 березня 2003 р. N 386\n\
\n\
Кабінет Міністрів України постановляє:\n\
\n\
1. Продовжити строк дії ...\n\
\n\
Знято постановою Кабінету Міністрів України від 28 лютого 2006 р. № 214";

    #[test]
    fn kmu_postanova_full_match() {
        let f = parse(136974, KMU_TEXT);
        assert_eq!(f.issuer.as_deref(), Some("Кабінет Міністрів України"));
        assert_eq!(f.doc_type.as_deref(), Some("ПОСТАНОВА"));
        assert_eq!(f.date.as_deref(), Some("11.02.2004"));
        assert_eq!(f.date_ymd, Some((2004, 2, 11)));
        assert_eq!(f.number.as_deref(), Some("№ 145"));
        assert!(f.title.as_deref().unwrap_or("").starts_with("Про продовження"));
        assert_eq!(f.status, Status::Canceled);
        assert!(f.status_detail.as_deref().unwrap_or("").contains("Знято постановою"));
    }

    #[test]
    fn president_not_published_and_numeric_date() {
        let text = "ПРЕЗИДЕНТ УКРАЇНИ\nУКАЗ\n19.12.2000  № 13-18/1-502\nПро надання роз'яснення\n\nтекст\n\nОпублікуванню не підлягає";
        let f = parse(1, text);
        assert_eq!(f.issuer.as_deref(), Some("Президент України"));
        assert_eq!(f.doc_type.as_deref(), Some("УКАЗ"));
        assert_eq!(f.date.as_deref(), Some("19.12.2000"));
        assert_eq!(f.status, Status::NotPublished);
        assert_eq!(f.number.as_deref(), Some("№ 13-18/1-502"));
    }

    #[test]
    fn latin_n_fallback_number() {
        let text = "ВЕРХОВНА РАДА УКРАЇНИ\nЗАКОН\nвід 16.07.1999 № 996-XIV\nПро бухгалтерський облік\n\nтекст\n\nТекст документ від 26 березня 2003 р. N 3992";
        let f = parse(2, text);
        assert_eq!(f.number.as_deref(), Some("№ 996-XIV"));
        assert_eq!(f.issuer.as_deref(), Some("Верховна Рада України"));
        assert_eq!(f.status, Status::Actual);
    }

    #[test]
    fn ministry_issuer_prefix() {
        let f = parse(3, "МІНІСТЕРСТВО ФІНАНСІВ УКРАЇНИ\nЛИСТ\nвід 05.04.2006 № 7-2-10-958\n");
        assert_eq!(
            f.issuer.as_deref(),
            Some("МІНІСТЕРСТВО ФІНАНСІВ УКРАЇНИ")
        );
        assert_eq!(f.doc_type.as_deref(), Some("ЛИСТ"));
        assert_eq!(f.number.as_deref(), Some("№ 7-2-10-958"));
    }

    #[test]
    fn empty_text_is_notext() {
        let f = parse(4, "");
        assert_eq!(f.status, Status::NoText);
        assert_eq!(f.n_lines, 0);
    }

    #[test]
    fn double_number_sign_is_collapsed() {
        let f = parse(7, "ДЕРЖАВНА ПОДАТКОВА АДМІНІСТРАЦІЯ\nЛИСТ\n28.02.2002 №№ 63\n");
        assert_eq!(f.number.as_deref(), Some("№ 63"));
    }
}
