//! GUI (native-windows-gui, Unicode end-to-end).
//!
//! Step 1: search (multi-term AND / OR / PHRASE) on either base.
//! Step 2: per-doc fields (видавець/тип/дата/номер/статус) parsed in the
//!         worker, shown in columns, usable as filters.
//! Step 3: multi-select of results (✓ column; click on the first cell or
//!         double-click a row) and export of the selected subset or of all
//!         filtered results as MD / RTF / DOCX.
//!
//! Search/fields/filtering and export run in worker threads; the UI thread
//! only polls a channel from a timer, so the window never blocks. Stale
//! messages are dropped by the generation counter.

use crate::db;
use crate::fields;
use crate::naming::Naming;
use crate::render;
use crate::writers;
use native_windows_gui as nwg;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

/// rows shown in the list view (tail collapsed)
const DISPLAY_CAP: usize = 2500;
/// preview text cap (chars) — a few large statutes exceed every edit buffer
const PREVIEW_CAP: usize = 400_000;
/// hits parsed for fields/filtering in the worker (unparsed tail keeps
/// exporting, but field filters can not verify it)
const FIELD_PARSE_CAP: usize = 20_000;

/// result list columns (0 is the ✓ check column); назва and the search
/// context are kept in two separate columns
const COL_FILE: usize = 1;
const COL_DOC: usize = 2;
const COL_DATE: usize = 3;
const COL_NUMBER: usize = 4;
const COL_ISSUER: usize = 5;
const COL_STATUS: usize = 6;
const COL_TITLE: usize = 7;
const COL_CONTEXT: usize = 8;
/// default sorting after a search: by date, newest first
const COL_SORT_DEFAULT: usize = COL_DATE;

/// What the search button runs: text search modes or the document-number
/// search.
enum SearchKind {
    And,
    Or,
    Phrase,
    Number,
}

impl SearchKind {
    fn name(&self) -> &'static str {
        match self {
            SearchKind::And => "AND",
            SearchKind::Or => "OR",
            SearchKind::Phrase => "PHRASE",
            SearchKind::Number => "№",
        }
    }
}

/// headless self-test report file (gui --guitest)
const GUI_READY: &str = "C:\\temp\\opencode\\zakon-exporter\\gui_ready.txt";

pub enum GuiMsg {
    SearchProgress { gen: usize, done: usize, total: usize, found: usize },
    FieldsProgress { gen: usize, done: usize, total: usize },
    SearchDone { gen: usize, hits: Vec<db::Hit>, fields: Vec<Option<fields::DocFields>> },
    SearchFail { gen: usize, msg: String },
    ExportProgress { gen: usize, done: usize, total: usize },
    ExportDone { gen: usize, exported: usize, attached: usize, out: String },
    ExportFail { gen: usize, msg: String },
    PreviewDone { gen: usize, docnum: u32, fields: Option<fields::DocFields>, text: String },
    PreviewFail { gen: usize, msg: String },
    /// the previewed doc's embedded annexes were enumerated: (ext, start, end)
    AnnexesDone { gen: usize, docnum: u32, items: Vec<(String, usize, usize)> },
    /// embedded annexes decoded + written (either the export folder — which
    /// also gets a combined `<docnum>_dodatk.md` — or a temp preview folder)
    AnnexFiles { gen: usize, count: usize, dir: String },
    /// raw annex files written to a temp folder and the folder opened in
    /// Explorer (the system viewers open the PDF / Word / Excel files)
    AnnexFilesOpen { gen: usize, count: usize, dir: String },
    /// embedded annexes converted to Markdown for the INTERNAL preview window
    AnnexPreview { gen: usize, count: usize, md: String },
    AnnexFail { gen: usize, msg: String },
    /// update-task worker events (no generation: one update at a time)
    UpdateCmd { idx: usize, total: usize, cmd: String },
    UpdateResult { idx: usize, total: usize, ok: bool, note: String },
    UpdateDone { ok: usize, fail: usize },
}

/// Field filters read from the UI controls; semantics mirror the CLI.
struct GuiFilters {
    status: fields::StatusWant,
    issuer: Option<String>,  // lowercase contains
    doctype: Option<String>, // uppercase contains
    title: Option<String>,   // lowercase contains
    dfrom: Option<(i32, u8, u8)>,
    dto: Option<(i32, u8, u8)>,
}

impl GuiFilters {
    fn neutral() -> Self {
        GuiFilters {
            status: fields::StatusWant::All,
            issuer: None,
            doctype: None,
            title: None,
            dfrom: None,
            dto: None,
        }
    }

    fn is_neutral(&self) -> bool {
        self.status == fields::StatusWant::All
            && self.issuer.is_none()
            && self.doctype.is_none()
            && self.title.is_none()
            && self.dfrom.is_none()
            && self.dto.is_none()
    }
}

/// One document passes the field filters (mirrors main.rs::apply_filters).
/// Unparsed docs are only kept when no field filters were requested.
fn filter_passes(f: &GuiFilters, fl: Option<&fields::DocFields>) -> bool {
    let fl = match (f.is_neutral(), fl) {
        (true, _) => return true,
        (false, None) => return false,
        (false, Some(fl)) => fl,
    };
    if !fields::status_pass(f.status, &fl.status) {
        return false;
    }
    if let Some(ref n) = f.issuer {
        if !fl.issuer.as_deref().map_or(false, |s| s.to_lowercase().contains(n)) {
            return false;
        }
    }
    if let Some(ref n) = f.doctype {
        if !fl.doc_type.as_deref().map_or(false, |s| s.to_uppercase().contains(n)) {
            return false;
        }
    }
    if let Some(ref n) = f.title {
        if !fl.title.as_deref().map_or(false, |s| s.to_lowercase().contains(n)) {
            return false;
        }
    }
    match fl.date_ymd {
        Some(y) => {
            if let Some(a) = f.dfrom {
                if y < a {
                    return false;
                }
            }
            if let Some(b) = f.dto {
                if y > b {
                    return false;
                }
            }
            true
        }
        None => f.dfrom.is_none() && f.dto.is_none(), // no date + date bound set => drop
    }
}

/// YYYY-MM-DD / DD.MM.YYYY / DD/MM/YYYY -> (y, m, d)
fn parse_ymd(s: &str) -> Result<(i32, u8, u8), String> {
    let parts: Vec<&str> = s.split(['-', '.', '/']).collect();    if parts.len() != 3 {
        return Err(format!("очікується дата: '{s}'"));
    }
    let y: i32 = parts[0].parse().map_err(|_| format!("рік: '{s}'"))?;
    let m: u8 = parts[1].parse().map_err(|_| format!("місяць: '{s}'"))?;
    let d: u8 = parts[2].parse().map_err(|_| format!("день: '{s}'"))?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return Err(format!("поза 1-12 міс / 1-31 дня: '{s}'"));
    }
    Ok((y, m, d))
}

// ============================================================
// results sorting (column header clicks)
// ============================================================

/// Sort key materialized once per hit before the comparison pass.
enum RowKey {
    Ymd(Option<(i32, u8, u8)>),
    U64(u64),
    Text(String),
}

/// Natural-ish text comparison: digit runs compare numerically ("249" < "63"
/// < "999"), letters compare case-insensitively; ties keep the input order
/// because the sorts are stable.
fn nat_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let mut ai = a.chars().peekable();
    let mut bi = b.chars().peekable();
    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                if x.is_ascii_digit() && y.is_ascii_digit() {
                    let ra = take_digits(&mut ai);
                    let rb = take_digits(&mut bi);
                    let sa = ra.trim_start_matches('0');
                    let sb = rb.trim_start_matches('0');
                    let ord = sa.len().cmp(&sb.len()).then_with(|| sa.cmp(sb));
                    if ord != Ordering::Equal {
                        return ord;
                    }
                } else {
                    let lx = x.to_lowercase().next().unwrap_or(x);
                    let ly = y.to_lowercase().next().unwrap_or(y);
                    let ord = lx.cmp(&ly);
                    if ord != Ordering::Equal {
                        return ord;
                    }
                    ai.next();
                    bi.next();
                }
            }
        }
    }
}

/// consume a digit run from a peekable char iterator
fn take_digits<F: Iterator<Item = char>>(it: &mut std::iter::Peekable<F>) -> String {
    let mut s = String::new();
    while let Some(c) = it.peek() {
        if !c.is_ascii_digit() {
            break;
        }
        s.push(*c);
        it.next();
    }
    s
}

/// «№ 249» -> "249" (the number column sorts by the core, unparsed last)
fn number_core(fl: Option<&fields::DocFields>) -> String {
    fl.and_then(|x| x.number.as_ref())
        .map(|n| {
            n.trim_start_matches(|c| c == '№' || c == 'N' || c == 'n' || c == ' ')
                .to_lowercase()
        })
        .unwrap_or_default()
}

/// Column order for the hit indices. Unparsed dates / numbers (dash rows)
/// always end up at the bottom, in both directions; when `desc` the normal
/// comparison reverses (date newest-first = the default state).
fn sort_indices(
    idx: &mut Vec<usize>,
    col: usize,
    desc: bool,
    hits: &[db::Hit],
    fields: &[Option<fields::DocFields>],
) {
    if col <= 0 {
        return; // the ✓ column doesn't sort
    }
    let key_of = |i: usize| {
        let h = &hits[i];
        let fl = fields[i].as_ref();
        let dash = String::new();
        match col {
            COL_DATE => RowKey::Ymd(fl.and_then(|x| x.date_ymd)),
            COL_DOC => RowKey::U64(h.docnum as u64),
            COL_FILE => RowKey::Text(h.file.trim_end_matches(".mb").to_string()),
            COL_NUMBER => RowKey::Text(number_core(fl)),
            COL_ISSUER => RowKey::Text(
                fl.and_then(|x| x.issuer.clone())
                    .map(|s| s.to_lowercase())
                    .unwrap_or(dash),
            ),
            COL_STATUS => RowKey::Text(
                fl.map(|x| x.status.label().to_string()).unwrap_or(dash),
            ),
            COL_TITLE => RowKey::Text(
                fl.and_then(|x| x.title.clone())
                    .map(|t| t.to_lowercase())
                    .unwrap_or(dash),
            ),
            _ => RowKey::Text(h.title.to_lowercase()), // COL_CONTEXT
        }
    };
    let mut keyed: Vec<(RowKey, usize)> = idx.iter().map(|&i| (key_of(i), i)).collect();
    keyed.sort_by(|(ka, ia), (kb, ib)| {
        use std::cmp::Ordering;
        // date None rows sink to the bottom regardless of direction
        let ord = match (ka, kb) {
            (RowKey::Ymd(None), RowKey::Ymd(None)) => Ordering::Equal,
            (RowKey::Ymd(None), _) => Ordering::Greater,
            (_, RowKey::Ymd(None)) => Ordering::Less,
            (RowKey::Ymd(Some(x)), RowKey::Ymd(Some(y))) => {
                let o = x.cmp(y);
                if desc {
                    o.reverse()
                } else {
                    o
                }
            }
            _ => {
                let o = key_compare_normal(ka, kb);
                if desc {
                    o.reverse()
                } else {
                    o
                }
            }
        };
        if ord == Ordering::Equal {
            ia.cmp(ib) // stable: keep the previous (search) order
        } else {
            ord
        }
    });
    *idx = keyed.into_iter().map(|(_, i)| i).collect();
}

fn key_compare_normal(a: &RowKey, b: &RowKey) -> std::cmp::Ordering {
    match (a, b) {
        (RowKey::U64(x), RowKey::U64(y)) => x.cmp(y),
        (RowKey::Text(x), RowKey::Text(y)) => nat_cmp(x, y),
        _ => std::cmp::Ordering::Equal, // keys are per-column typed
    }
}

#[derive(Default, native_windows_derive::NwgUi)]
pub struct App {
    #[nwg_control(size: (1150, 804), position: (80, 40),
        title: "Zakon DB Exporter",
        flags: "WINDOW|VISIBLE|MINIMIZE_BOX|MAXIMIZE_BOX|SYS_MENU")]
    #[nwg_events(OnWindowClose: [nwg::stop_thread_dispatch()])]
    window: nwg::Window,

    // ---- row 1: query + base + search button -------------------------------
    #[nwg_control(parent: window, size: (56, 24), position: (10, 17), text: "Запит:")]
    label_q: nwg::Label,

    #[nwg_control(parent: window, size: (430, 24), position: (68, 14))]
    query_edit: nwg::TextInput,

    #[nwg_control(parent: window, size: (44, 24), position: (508, 17), text: "База:")]
    label_base: nwg::Label,

    #[nwg_control(parent: window, size: (230, 300), position: (554, 14))]
    combo_base: nwg::ComboBox<String>,

    #[nwg_control(parent: window, size: (120, 26), position: (792, 12), text: "Пошук")]
    #[nwg_events(OnButtonClick: [App::on_search])]
    btn_search: nwg::Button,

    #[nwg_control(parent: window, size: (96, 26), position: (920, 12), text: "База…")]
    #[nwg_events(OnButtonClick: [App::on_pick_base])]
    btn_base: nwg::Button,

    // ---- row 2: mode + status + dates + issuer + type ----------------------
    #[nwg_control(parent: window, size: (46, 24), position: (10, 53), text: "Режим:")]
    label_mode: nwg::Label,

    #[nwg_control(parent: window, size: (160, 300), position: (56, 50))]
    combo_mode: nwg::ComboBox<String>,

    #[nwg_control(parent: window, size: (56, 24), position: (226, 53), text: "Статус:")]
    label_status: nwg::Label,

    #[nwg_control(parent: window, size: (190, 300), position: (286, 50))]
    combo_status: nwg::ComboBox<String>,

    #[nwg_control(parent: window, size: (26, 24), position: (482, 53), text: "від")]
    label_dfrom: nwg::Label,

    #[nwg_control(parent: window, size: (84, 24), position: (510, 50))]
    date_from: nwg::TextInput,

    #[nwg_control(parent: window, size: (24, 24), position: (598, 53), text: "до")]
    label_dto: nwg::Label,

    #[nwg_control(parent: window, size: (84, 24), position: (626, 50))]
    date_to: nwg::TextInput,

    #[nwg_control(parent: window, size: (72, 24), position: (718, 53), text: "Видавець:")]
    label_iss: nwg::Label,

    #[nwg_control(parent: window, size: (146, 24), position: (792, 50))]
    issuer_edit: nwg::TextInput,

    #[nwg_control(parent: window, size: (36, 24), position: (944, 53), text: "Тип:")]
    label_type: nwg::Label,

    #[nwg_control(parent: window, size: (108, 24), position: (982, 50))]
    type_edit: nwg::TextInput,

    // ---- row 3: title filter + filter buttons ------------------------------
    #[nwg_control(parent: window, size: (50, 24), position: (10, 89), text: "Назва:")]
    label_title: nwg::Label,

    #[nwg_control(parent: window, size: (280, 24), position: (62, 86))]
    title_edit: nwg::TextInput,

    #[nwg_control(parent: window, size: (160, 26), position: (350, 84), text: "Застосувати фільтр")]
    #[nwg_events(OnButtonClick: [App::on_apply_filter])]
    btn_apply_filter: nwg::Button,

    #[nwg_control(parent: window, size: (150, 26), position: (520, 84), text: "Скинути фільтри")]
    #[nwg_events(OnButtonClick: [App::on_reset_filters])]
    btn_reset: nwg::Button,

    #[nwg_control(parent: window, size: (170, 26), position: (686, 84), text: "Про програму")]
    #[nwg_events(OnButtonClick: [App::on_about])]
    btn_about: nwg::Button,

    #[nwg_control(parent: window, size: (160, 26), position: (866, 84), text: "Оновлення…")]
    #[nwg_events(OnButtonClick: [App::on_update])]
    btn_update: nwg::Button,

    // ---- results -----------------------------------------------------------
    #[nwg_control(parent: window, position: (10, 120), size: (1120, 500),
        flags: "VISIBLE|TAB_STOP|ALWAYS_SHOW_SELECTION",
        list_style: nwg::ListViewStyle::Detailed,
        ex_flags: nwg::ListViewExFlags::GRID | nwg::ListViewExFlags::FULL_ROW_SELECT)]
    // header clicks sort the results table (see on_column_click)
    #[nwg_events(OnListViewClick: [App::on_list_click(SELF, EVT_DATA)],
        OnListViewDoubleClick: [App::on_list_dclick(SELF, EVT_DATA)],
        OnListViewColumnClick: [App::on_column_click(SELF, EVT_DATA)])]
    list: nwg::ListView,

    // ---- row 4: export folder + preview trigger ----------------------------
    #[nwg_control(parent: window, size: (56, 24), position: (10, 628), text: "Папка:")]
    label_out: nwg::Label,

    #[nwg_control(parent: window, size: (452, 26), position: (68, 624))]
    out_edit: nwg::TextInput,

    #[nwg_control(parent: window, size: (110, 26), position: (526, 624), text: "Огляд…")]
    #[nwg_events(OnButtonClick: [App::on_browse_out])]
    btn_browse: nwg::Button,

    #[nwg_control(parent: window, size: (140, 26), position: (646, 624), text: "Перегляд")]
    #[nwg_events(OnButtonClick: [App::on_preview])]
    btn_preview: nwg::Button,

    #[nwg_control(parent: window, size: (340, 24), position: (796, 628),
        text: "клік на ✓ чи «Вибрати все» — вибір; подвійний клік — перегляд")]
    label_hint: nwg::Label,

    // ---- bottom: format + export + selection -------------------------------
    #[nwg_control(parent: window, size: (48, 24), position: (10, 673), text: "Формат:")]
    label_fmt: nwg::Label,

    #[nwg_control(parent: window, size: (70, 300), position: (58, 670))]
    combo_fmt: nwg::ComboBox<String>,

    #[nwg_control(parent: window, size: (200, 28), position: (138, 668),
        text: "Експорт обраних (0)")]
    #[nwg_events(OnButtonClick: [App::on_export_selected])]
    btn_export_sel: nwg::Button,

    #[nwg_control(parent: window, size: (140, 28), position: (348, 668),
        text: "Експорт усіх")]
    #[nwg_events(OnButtonClick: [App::on_export_all])]
    btn_export_all: nwg::Button,

    #[nwg_control(parent: window, size: (80, 28), position: (498, 668),
        text: "✓ всі")]
    #[nwg_events(OnButtonClick: [App::on_select_all])]
    btn_sel_all: nwg::Button,

    #[nwg_control(parent: window, size: (110, 28), position: (586, 668),
        text: "✓ зняти")]
    #[nwg_events(OnButtonClick: [App::on_select_none])]
    btn_sel_none: nwg::Button,

    #[nwg_control(parent: window, size: (410, 24), position: (712, 673), text: "")]
    label_count: nwg::Label,

    // include signal attachments (додатки / Word-вложения) in the export
    #[nwg_control(parent: window, size: (280, 24), position: (712, 700),
        text: "додатки (сигнальні документи)")]
    chk_attach: nwg::CheckBox,

    // "зв'язка (1 файл)": all exported documents go into ONE file
    // (md/rtf/docx/html/pdf; pdf gets a /Outlines table of contents)
    #[nwg_control(parent: window, size: (280, 24), position: (712, 724),
        text: "зв'язка (1 файл на всі документи)")]
    chk_bundle: nwg::CheckBox,

    // mark the search query terms in the exported text (md/html/rtf/docx)
    #[nwg_control(parent: window, size: (280, 24), position: (712, 748),
        text: "підсвітити слова запиту")]
    chk_hl: nwg::CheckBox,

    // experimental: search the base's native Trident index (SOFT\) instead of
    // the built-in full-text engine — doc numbers are the native 2005-era
    // ids (the join to the current base is v0.14.0)
    #[nwg_control(parent: window, size: (280, 24), position: (712, 772),
        text: "Trident-рушій (нативний індекс, експеримент)")]
    chk_trident: nwg::CheckBox,

    #[nwg_control(parent: window, text: "Готово")]
    status: nwg::StatusBar,

    #[nwg_control(parent: window, interval: 150, stopped: false)]
    #[nwg_events(OnTimerTick: [App::on_tick])]
    timer: nwg::Timer,

    // ---- preview window (hidden until the first use) ------------------------
    // SYS_MENU gives the X button; OnWindowClose is intercepted below so the
    // window is hidden instead of destroyed and can be reopened.
    #[nwg_control(size: (940, 732), position: (100, 60),
        title: "Перегляд документа",
        flags: "WINDOW|MINIMIZE_BOX|SYS_MENU")]
    #[nwg_events(OnWindowClose: [App::on_preview_close(SELF, EVT_DATA)])]
    preview_window: nwg::Window,

    // no HSCROLL/AUTOHSCROLL → the edit control word-wraps long lines; the
    // document's own line structure (see doc_text) stays readable as-is
    #[nwg_control(parent: preview_window, size: (916, 570), position: (8, 8),
        flags: "VISIBLE|VSCROLL|AUTOVSCROLL|TAB_STOP")]
    preview_text: nwg::TextBox,

    #[nwg_control(parent: preview_window, size: (170, 28), position: (608, 586),
        text: "Експортувати")]
    #[nwg_events(OnButtonClick: [App::on_preview_export])]
    btn_preview_export: nwg::Button,

    #[nwg_control(parent: preview_window, size: (130, 28), position: (794, 586), text: "Закрити")]
    #[nwg_events(OnButtonClick: [App::on_preview_hide])]
    btn_preview_close: nwg::Button,

    #[nwg_control(parent: preview_window, size: (590, 24), position: (8, 590))]
    preview_status: nwg::Label,

    // signal attachment (додаток) status for the previewed document
    #[nwg_control(parent: preview_window, size: (590, 24), position: (8, 618),
        text: "")]
    preview_signal: nwg::Label,

    #[nwg_control(parent: preview_window, size: (170, 28), position: (608, 616),
        text: "Відкрити додаток")]
    #[nwg_events(OnButtonClick: [App::on_preview_open_signal])]
    btn_preview_open_signal: nwg::Button,

    // embedded annexes (BZh9 BZip2 streams inside the document's text\*.mb
    // chunk): list + «Вивантажити» (extract the files + a combined
    // `<docnum>_dodatk.md` to the export folder) + «Прев'ю» (convert to
    // Markdown and show it INTERNALLY in the annex window — no Explorer)
    #[nwg_control(parent: preview_window, size: (590, 24), position: (8, 646),
        text: "")]
    annex_status: nwg::Label,

    #[nwg_control(parent: preview_window, size: (170, 28), position: (608, 644),
        text: "Вивантажити")]
    #[nwg_events(OnButtonClick: [App::on_annex_save])]
    btn_annex_save: nwg::Button,

    #[nwg_control(parent: preview_window, size: (130, 28), position: (794, 644),
        text: "Прев'ю")]
    #[nwg_events(OnButtonClick: [App::on_annex_open])]
    btn_annex_open: nwg::Button,

    // document preview mode: plain text (default, the base's own line
    // structure) vs format-preserving Markdown (writers::doc_markdown:
    // # title, *requisites*, **ALL-CAPS headers**, - lists)
    #[nwg_control(parent: preview_window, size: (100, 28), position: (8, 676),
        text: "Текст")]
    #[nwg_events(OnButtonClick: [App::on_preview_mode_text])]
    btn_preview_mode_text: nwg::Button,

    #[nwg_control(parent: preview_window, size: (100, 28), position: (114, 676),
        text: "Формат (md)")]
    #[nwg_events(OnButtonClick: [App::on_preview_mode_fmt])]
    btn_preview_mode_fmt: nwg::Button,

    #[nwg_control(parent: preview_window, size: (420, 24), position: (220, 680),
        text: "режим: текст")]
    preview_mode_label: nwg::Label,

    // «Друк» in the preview window: GDI print of the cached document (the
    // system print dialog + a printer DC; ALL-CAPS lines bold, list items
    // with "•  " markers, centered lines centered — see printdoc.rs). The
    // dialog is modal and blocks the UI while it is open, like any native
    // print dialog; cancelling reports «Друк: відмінено».
    #[nwg_control(parent: preview_window, size: (110, 28), position: (648, 676),
        text: "Друк")]
    #[nwg_events(OnButtonClick: [App::on_preview_print])]
    btn_preview_print: nwg::Button,

    // ---- internal annex (додатки) preview window ---------------------------
    // Shows the Markdown-converted content of the previewed document's
    // embedded annexes inside the app (no external Explorer / Word / PDF
    // viewer). Hidden until «Прев'ю» is used; hide-not-destroy, like the
    // preview window.
    #[nwg_control(size: (780, 620), position: (140, 80),
        title: "Додатки — прев'ю",
        flags: "WINDOW|MINIMIZE_BOX|SYS_MENU")]
    #[nwg_events(OnWindowClose: [App::on_annex_win_close(SELF, EVT_DATA)])]
    annex_window: nwg::Window,

    // no HSCROLL/AUTOHSCROLL → word-wraps long lines (same as preview_text)
    #[nwg_control(parent: annex_window, size: (764, 560), position: (8, 8),
        flags: "VISIBLE|VSCROLL|AUTOVSCROLL|TAB_STOP")]
    annex_text: nwg::TextBox,

    // preview mode: plain text (default, what «Вивантажити» exports) vs
    // format-preserving Markdown (headings, **bold**, lists, tables)
    #[nwg_control(parent: annex_window, size: (100, 28), position: (10, 580),
        text: "Текст")]
    #[nwg_events(OnButtonClick: [App::on_annex_mode_text])]
    btn_annex_mode_text: nwg::Button,

    #[nwg_control(parent: annex_window, size: (100, 28), position: (116, 580),
        text: "Формат (md)")]
    #[nwg_events(OnButtonClick: [App::on_annex_mode_fmt])]
    btn_annex_mode_fmt: nwg::Button,

    #[nwg_control(parent: annex_window, size: (196, 24), position: (222, 584),
        text: "режим: текст (як експорт .md)")]
    annex_mode_label: nwg::Label,

    // raw annex files → temp folder → Explorer (system viewers: PDF / Word /
    // Excel open the files themselves)
    #[nwg_control(parent: annex_window, size: (114, 28), position: (424, 580),
        text: "Файли")]
    #[nwg_events(OnButtonClick: [App::on_annex_files])]
    btn_annex_files: nwg::Button,

    #[nwg_control(parent: annex_window, size: (130, 28), position: (642, 580),
        text: "Закрити")]
    #[nwg_events(OnButtonClick: [App::on_annex_hide])]
    btn_annex_hide: nwg::Button,

    // ---- update-task dialog (hidden; opened by «Оновлення…») ---------------
    // Same hide-not-destroy pattern as the preview window: the X close is
    // intercepted so the dialog can be reopened later.
    #[nwg_control(size: (700, 350), position: (170, 110),
        title: "Поновлення Законодавства",
        flags: "WINDOW|MINIMIZE_BOX|SYS_MENU")]
    #[nwg_events(OnWindowClose: [App::on_update_close(SELF, EVT_DATA)])]
    update_window: nwg::Window,

    #[nwg_control(parent: update_window, size: (96, 24), position: (10, 12), text: "Назва:")]
    upd_title_label: nwg::Label,

    #[nwg_control(parent: update_window, size: (574, 24), position: (110, 10))]
    upd_title_edit: nwg::TextInput,

    #[nwg_control(parent: update_window, size: (400, 24), position: (10, 42),
        text: "Команди (одна команда — один рядок):")]
    upd_cmds_label: nwg::Label,

    #[nwg_control(parent: update_window, size: (674, 210), position: (10, 66),
        flags: "VISIBLE|HSCROLL|VSCROLL|AUTOVSCROLL|AUTOHSCROLL|TAB_STOP")]
    upd_text: nwg::TextBox,

    #[nwg_control(parent: update_window, size: (120, 28), position: (10, 288), text: "Запустити")]
    #[nwg_events(OnButtonClick: [App::on_update_run])]
    btn_update_run: nwg::Button,

    #[nwg_control(parent: update_window, size: (120, 28), position: (140, 288), text: "Зберегти")]
    #[nwg_events(OnButtonClick: [App::on_update_save])]
    btn_update_save: nwg::Button,

    #[nwg_control(parent: update_window, size: (286, 24), position: (272, 292), text: "")]
    upd_status: nwg::Label,

    #[nwg_control(parent: update_window, size: (118, 28), position: (566, 288), text: "Закрити")]
    #[nwg_events(OnButtonClick: [App::on_update_click_close])]
    btn_update_close: nwg::Button,

    // plain state (no attributes — derive leaves them untouched)
    baza: PathBuf,
    /// (label, dir) list; grows when the user adds a folder via «База…»
    bases: RefCell<Vec<(String, PathBuf)>>,
    naming: Rc<Naming>,
    rx: RefCell<Option<Receiver<GuiMsg>>>,
    results: RefCell<(Vec<db::Hit>, Vec<Option<fields::DocFields>>)>,
    /// hit indices behind the currently displayed rows
    row_hits: RefCell<Vec<usize>>,
    /// per displayed row check state
    sel: RefCell<Vec<bool>>,
    /// the row the user last clicked in the results table (any column);
    /// «Перегляд» opens this row's document first of all
    last_click: RefCell<Option<usize>>,
    /// docnum currently loaded in the preview window (for the «Експортувати»
    /// button)
    preview_doc: RefCell<Option<u32>>,
    /// the previewed doc's embedded annexes: (ext, start, end) within its
    /// chunk, captured when it was previewed (for «Вивантажити» / «Прев'ю»)
    preview_annexes: RefCell<Vec<(String, usize, usize)>>,
    /// annex preview mode: false = plain text (default, matches the .md
    /// export), true = format-preserving Markdown (headings, bold, lists)
    annex_fmt: RefCell<bool>,
    /// document preview mode: false = plain text (default), true =
    /// format-preserving Markdown (writers::doc_markdown)
    preview_fmt: RefCell<bool>,
    /// the last previewed document (docnum, fields, plain text) — cached so
    /// a preview-mode switch re-renders in place without re-reading the base
    preview_cache: RefCell<Option<(u32, Option<fields::DocFields>, String)>>,
    /// current results sort: (column, descending); a new search resets it
    /// to the date column, newest first
    sort: RefCell<(usize, bool)>,
    gen: RefCell<usize>,
    guitest: bool,
    ticks: RefCell<usize>,
    /// self-test stage: 0 idle, 1 searching, 2 rows ready, 3 exporting,
    /// 4 preview running, 5 done
    gt_stage: RefCell<u8>,
    /// (files written, actual output dir) captured from ExportDone
    gt_export: RefCell<Option<(usize, String)>>,
    /// PreviewDone processed?
    gt_preview_done: RefCell<bool>,
    /// an update task is running (worker thread active)
    update_running: RefCell<bool>,
}

impl App {
    // ------------------------------------------------------------------
    // search
    // ------------------------------------------------------------------

    pub fn on_search(&self) {
        let query = self.query_edit.text().trim().to_string();
        if query.is_empty() {
            self.status.set_text(0, "Введіть запит для пошуку");
            return;
        }
        let kind = match self.combo_mode.selection() {
            Some(1) => SearchKind::Or,
            Some(2) => SearchKind::Phrase,
            Some(3) => SearchKind::Number,
            _ => SearchKind::And,
        };
        if self.local_filters().is_none() {
            return; // bad date bound — message already shown
        }
        let (label, baza) = {
            let bs = self.bases.borrow();
            match self.combo_base.selection() {
                Some(i) if i < bs.len() => bs[i].clone(),
                _ => ("поточна".to_string(), self.baza.clone()),
            }
        };
        let (tx, rx) = channel();
        *self.rx.borrow_mut() = Some(rx);
        let gen = {
            let mut g = self.gen.borrow_mut();
            *g += 1;
            *g
        };
        {
            let (hits, fields) = &mut *self.results.borrow_mut();
            hits.clear();
            fields.clear();
        }
        self.clear_list();
        let trident = self.chk_trident.check_state() == nwg::CheckBoxState::Checked;
        self.status.set_text(
            0,
            &if trident {
                format!("Trident-пошук «{query}» (база: {label})…")
            } else {
                format!("Пошук «{query}» ({}, база: {label})…", kind.name())
            },
        );
        if trident {
            thread::spawn(move || trident_search_worker(tx, baza, query, gen));
        } else {
            thread::spawn(move || search_worker(tx, baza, query, kind, gen));
        }
    }

    /// Read the filter controls. `None` => a date is malformed (reported).
    fn local_filters(&self) -> Option<GuiFilters> {
        let status = match self.combo_status.selection() {
            Some(1) => fields::StatusWant::InForce,
            Some(2) => fields::StatusWant::Canceled,
            Some(3) => fields::StatusWant::NotPublished,
            _ => fields::StatusWant::All,
        };
        let edit = |e: &nwg::TextInput| {
            let s = e.text().trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        };
        let issuer = edit(&self.issuer_edit).map(|s| s.to_lowercase());
        let doctype = edit(&self.type_edit).map(|s| s.to_uppercase());
        let title = edit(&self.title_edit).map(|s| s.to_lowercase());
        let mut dfrom = None;
        let mut dto = None;
        let mut bad: Option<String> = None;
        if let Some(s) = edit(&self.date_from) {
            match parse_ymd(&s) {
                Ok(v) => dfrom = Some(v),
                Err(e) => bad = Some(e),
            }
        }
        if let Some(s) = edit(&self.date_to) {
            if bad.is_none() {
                match parse_ymd(&s) {
                    Ok(v) => dto = Some(v),
                    Err(e) => bad = Some(e),
                }
            }
        }
        if let Some(e) = bad {
            self.status.set_text(0, &format!("Помилка у даті (YYYY-MM-DD): {e}"));
            return None;
        }
        Some(GuiFilters { status, issuer, doctype, title, dfrom, dto })
    }

    /// (Re)build the displayed rows from the current field filters.
    pub fn on_apply_filter(&self) {
        let f = match self.local_filters() {
            Some(f) => f,
            None => return,
        };
        let total = self.results.borrow().0.len();
        if total == 0 {
            self.status.set_text(0, "Спочатку виконайте пошук");
            return;
        }
        self.show_filtered(&f);
        self.update_counts();
        let shown = self.sel.borrow().len();
        self.status.set_text(0, &format!("Фільтр: {total} знайдених → {shown} показано"));
    }

    pub fn on_reset_filters(&self) {
        self.combo_status.set_selection(Some(0));
        self.issuer_edit.set_text("");
        self.type_edit.set_text("");
        self.title_edit.set_text("");
        self.date_from.set_text("");
        self.date_to.set_text("");
        if !self.results.borrow().0.is_empty() {
            let f = GuiFilters::neutral();
            self.show_filtered(&f);
            self.update_counts();
        }
        self.status.set_text(0, "Фільтри скинуто");
    }

    // ------------------------------------------------------------------
    // selection
    // ------------------------------------------------------------------

    pub fn on_list_click(&self, evt: &nwg::EventData) {
        if let nwg::EventData::OnListViewItemIndex { row_index, column_index } = evt {
            // «Перегляд» follows the row the user actually clicked, whatever
            // column it was (✓ clicks mark + remember the row as well)
            *self.last_click.borrow_mut() = Some(*row_index);
            if *column_index == 0 {
                self.toggle_row(*row_index);
            }
        }
    }

    pub fn on_list_dclick(&self, evt: &nwg::EventData) {
        if let nwg::EventData::OnListViewItemIndex { row_index, column_index } = evt {
            *self.last_click.borrow_mut() = Some(*row_index);
            if *column_index != 0 {
                // a double-click on a row opens its document (the Explorer
                // idiom); marking stays with the single ✓-column click
                self.on_preview();
            }
        }
    }

    /// Column-header click: sort by that column; a second click on the same
    /// header flips the direction. The ✓ column doesn't sort. Checks follow
    /// their documents across resorts (see show_filtered).
    pub fn on_column_click(&self, evt: &nwg::EventData) {
        if let nwg::EventData::OnListViewItemIndex { column_index, .. } = evt {
            let col = *column_index;
            if col == 0 || col > COL_CONTEXT {
                return;
            }
            {
                let mut s = self.sort.borrow_mut();
                if s.0 == col {
                    s.1 = !s.1;
                } else {
                    // the date column starts newest-first, everything else A→Z
                    *s = (col, col == COL_DATE);
                }
            }
            if self.results.borrow().0.is_empty() {
                self.status
                    .set_text(0, "Спочатку виконайте пошук — потім сортувуйте стовпці");
                self.draw_sort_arrow();
            } else {
                let f = self.local_filters().unwrap_or_else(GuiFilters::neutral);
                self.show_filtered(&f);
            }
        }
    }

    fn toggle_row(&self, row: usize) {
        if row >= self.sel.borrow().len() {
            return; // click below any row / on empty space
        }
        let now = {
            let mut s = self.sel.borrow_mut();
            s[row] = !s[row];
            s[row]
        };
        self.list.update_item(
            row,
            nwg::InsertListViewItem {
                index: None,
                column_index: 0,
                text: Some(if now { "✓".to_string() } else { String::new() }),
                ..Default::default()
            },
        );
        self.update_counts();
    }

    pub fn on_select_all(&self) {
        let n = self.sel.borrow().len();
        if n == 0 {
            return;
        }
        self.list.set_redraw(false);
        for r in 0..n {
            self.list.update_item(
                r,
                nwg::InsertListViewItem {
                    index: None,
                    column_index: 0,
                    text: Some("✓".to_string()),
                    ..Default::default()
                },
            );
        }
        self.list.set_redraw(true);
        self.list.invalidate();
        {
            let mut s = self.sel.borrow_mut();
            for v in s.iter_mut() {
                *v = true;
            }
        }
        self.update_counts();
    }

    pub fn on_select_none(&self) {
        let n = self.sel.borrow().len();
        if n == 0 {
            return;
        }
        self.list.set_redraw(false);
        for r in 0..n {
            self.list.update_item(
                r,
                nwg::InsertListViewItem {
                    index: None,
                    column_index: 0,
                    text: Some(String::new()),
                    ..Default::default()
                },
            );
        }
        self.list.set_redraw(true);
        self.list.invalidate();
        self.update_counts();
    }

    fn selected_count(&self) -> usize {
        self.sel.borrow().iter().filter(|&&b| b).count()
    }

    fn update_counts(&self) {
        let sel = self.selected_count();
        let total = self.results.borrow().0.len();
        let shown = self.sel.borrow().len();
        self.btn_export_sel.set_text(&format!("Експорт обраних ({sel})"));
        self.label_count
            .set_text(&format!("Знайдено: {total} | показано: {shown} | обрано: {sel}"));
    }

    // ------------------------------------------------------------------
    // export
    // ------------------------------------------------------------------

    pub fn on_export_selected(&self) {
        let idx: Vec<usize> = {
            let sel = self.sel.borrow();
            (0..sel.len()).filter(|&i| sel[i]).collect()
        };
        if idx.is_empty() {
            self.status
                .set_text(0, "Нічого не вибрано: клік на ✓ у рядку документа");
            return;
        }
        let hits: Vec<db::Hit> = {
            let r = self.results.borrow();
            let (hits, _) = &*r;
            let rows = self.row_hits.borrow();
            idx.iter().map(|&row| hits[rows[row]].clone()).collect()
        };
        let n = hits.len();
        self.spawn_export(hits, &format!("Експорт {n} обраних документів…"));
    }

    pub fn on_export_all(&self) {
        let f = match self.local_filters() {
            Some(f) => f,
            None => return,
        };
        if self.results.borrow().0.is_empty() {
            self.status.set_text(0, "Немає результатів для експорту");
            return;
        }
        let sel: Vec<db::Hit> = {
            let r = self.results.borrow();
            let (hits, fields) = &*r;
            hits.iter()
                .zip(fields.iter())
                .filter(|(_, fl)| filter_passes(&f, fl.as_ref()))
                .map(|(h, _)| h.clone())
                .collect()
        };
        let n = sel.len();
        if n == 0 {
            self.status.set_text(0, "Фільтр не пропустив жодного документа");
            return;
        }
        self.spawn_export(sel, &format!("Експорт {n} документів…"));
    }

    fn spawn_export(&self, hits: Vec<db::Hit>, label: &str) {
        let format = match self.combo_fmt.selection_string() {
            Some(f) => f.trim().to_lowercase(),
            None => {
                self.status.set_text(0, "Виберіть формат (md/rtf/docx/html/csv/pdf)");
                return;
            }
        };
        let (tx, rx) = channel();
        *self.rx.borrow_mut() = Some(rx);
        let gen = {
            let mut g = self.gen.borrow_mut();
            *g += 1;
            *g
        };
        self.status.set_text(0, label);
        let baza = self.bases_current().1;
        let mut naming: Naming = (*self.naming).clone();
        // user-chosen export folder overrides naming.toml (same root; the
        // per-run subfolder policy of naming.toml is kept)
        let dir = self.out_edit.text().trim().to_string();
        if !dir.is_empty() {
            naming.out_dir = dir;
        }
        let attach = self.chk_attach.check_state() == nwg::CheckBoxState::Checked;
        let single = self.chk_bundle.check_state() == nwg::CheckBoxState::Checked;
        let hl_on = self.chk_hl.check_state() == nwg::CheckBoxState::Checked;
        let hl_query = self.query_edit.text();
        thread::spawn(move || {
            export_worker(tx, baza, hits, format, naming, gen, attach, single, hl_on, hl_query)
        });
    }

    fn bases_current(&self) -> (String, PathBuf) {
        let bs = self.bases.borrow();
        match self.combo_base.selection() {
            Some(i) if i < bs.len() => bs[i].clone(),
            _ => ("поточна".to_string(), self.baza.clone()),
        }
    }

    // ------------------------------------------------------------------
    // export folder chooser + document preview
    // ------------------------------------------------------------------

    pub fn on_browse_out(&self) {
        let parent = match self.window.handle.hwnd() {
            Some(h) => h,
            None => return,
        };
        let current = {
            let s = self.out_edit.text().trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(PathBuf::from(s))
            }
        };
        match crate::picker::pick_folder(parent, current.as_deref()) {
            Some(p) => {
                let shown = p.to_string_lossy().to_string();
                self.out_edit.set_text(&shown);
                self.status.set_text(0, &format!("Папка експорту: {shown}"));
            }
            None => self.status.set_text(0, "Вибір папки скасовано"),
        }
    }

    /// Document to preview — priority order:
    /// 1. the row the user last clicked in the table (any column, even a
    ///    ✓-marking click);
    /// 2. the row currently highlighted (selected) in the list view;
    /// 3. the first ✓-marked row.
    /// Positions are validated against the CURRENT row table, so a stale
    /// click/selection can never resolve to a document the user doesn't see.
    fn preview_target(&self) -> Option<(u32, Option<fields::DocFields>)> {
        let row = {
            let clicked = *self.last_click.borrow();
            match clicked {
                Some(r) if r < self.row_hits.borrow().len() => Some(r),
                _ => None,
            }
        }
        .or_else(|| self.list.selected_item())
        .or_else(|| self.sel.borrow().iter().position(|&b| b));
        let row = row?;
        let hi = {
            let rows = self.row_hits.borrow();
            if row >= rows.len() {
                return None;
            }
            rows[row]
        };
        let r = self.results.borrow();
        let (hits, fields) = &*r;
        if hi >= hits.len() {
            return None;
        }
        Some((hits[hi].docnum, fields[hi].clone()))
    }

    pub fn on_preview(&self) {
        let (docnum, fl) = match self.preview_target() {
            Some(v) => v,
            None => {
                self.status
                    .set_text(0, "Виберіть документ (✓ або клік по рядку) для перегляду");
                return;
            }
        };
        // remember what's being previewed (the «Експортувати» button uses it)
        *self.preview_doc.borrow_mut() = Some(docnum);
        self.preview_status.set_text(&format!(
            "Документ {docnum}. «Експортувати» — вивантажити його у вибраний формат (md/rtf/docx) до папки експорту."
        ));
        // embedded-annex UI: reset; the worker fills it back in (AnnexesDone)
        self.preview_annexes.borrow_mut().clear();
        // a new document → back to the default (plain text) annex preview mode
        *self.annex_fmt.borrow_mut() = false;
        self.annex_mode_label.set_text("режим: текст (як експорт .md)");
        self.annex_status.set_text("Додатки: завантаження…");
        // signal attachment (додаток) status for this document
        let baza_now = self.bases_current().1;
        match crate::signal::find_signal(&baza_now, docnum) {
            Some(p) => {
                let name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                self.preview_signal
                    .set_text(&format!("Додаток (сигнальний документ): {name}"));
            }
            None => {
                self.preview_signal
                    .set_text(&format!("Додаток до док. {docnum}: відсутній у цій базі"));
            }
        }
        let (tx, rx) = channel();
        *self.rx.borrow_mut() = Some(rx);
        let gen = {
            let mut g = self.gen.borrow_mut();
            *g += 1;
            *g
        };
        self.status.set_text(0, &format!("Завантаження документа {docnum}…"));
        let baza = self.bases_current().1;
        thread::spawn(move || preview_worker(tx, baza, docnum, fl, gen));
    }

    /// «Експортувати» in the preview window: export exactly the document
    /// being previewed, in the format and to the folder chosen in the main
    /// window. The document is also marked ✓ in the results table so the
    /// user can see it was handled.
    pub fn on_preview_export(&self) {
        let docnum = match *self.preview_doc.borrow() {
            Some(d) => d,
            None => {
                self.preview_status
                    .set_text("Немає документа: спочатку натисніть «Перегляд»");
                return;
            }
        };
        let hit = {
            let r = self.results.borrow();
            let (hits, _) = &*r;
            hits.iter().find(|h| h.docnum == docnum).cloned()
        };
        let hit = match hit {
            Some(h) => h,
            None => {
                self.preview_status
                    .set_text(&format!("Документ {docnum} відсутній у поточних результатах пошуку"));
                return;
            }
        };
        // mark it in the main table (visual feedback of what got exported)
        let row_pos: Option<usize> = {
            let rows = self.row_hits.borrow();
            let r = self.results.borrow();
            rows.iter().position(|&hi| r.0[hi].docnum == docnum)
        };
        if let Some(row) = row_pos {
            let is_checked = self.sel.borrow()[row];
            if !is_checked {
                self.toggle_row(row);
            }
        }
        self.preview_status
            .set_text(&format!("Експорт документа {docnum}… (формат і папка — з головного вікна)"));
        self.spawn_export(vec![hit], &format!("Експорт документа {docnum} з перегляду…"));
    }

    /// «Відкрити додаток» in the preview window: open the previewed document's
    /// signal attachment (if any) in its associated application (Word etc.).
    pub fn on_preview_open_signal(&self) {
        let docnum = match *self.preview_doc.borrow() {
            Some(d) => d,
            None => {
                self.preview_signal.set_text("Додаток: спочатку відкрийте документ у перегляді");
                return;
            }
        };
        let baza = self.bases_current().1;
        match crate::signal::find_signal(&baza, docnum) {
            Some(p) => {
                let s = p.to_string_lossy().to_string();
                // open in the associated app (Word / OpenOffice / archiver…)
                match std::process::Command::new("cmd").args(["/c", "start", "", &s]).spawn() {
                    Ok(_) => self.preview_status.set_text(&format!("Відкрито: {s}")),
                    Err(e) => self.preview_status.set_text(&format!("Не вдалося відкрити {s}: {e}")),
                }
            }
            None => {
                self.preview_signal
                    .set_text(&format!("Додаток до док. {docnum}: відсутній у цій базі"));
            }
        }
    }

    /// «Вивантажити» in the preview window: decode the previewed document's
    /// embedded annexes (BZh9 streams in its text\*.mb chunk) and write them
    /// to the export folder (the folder chosen in the main window, or the
    /// naming.toml output dir) — each annex as `<docnum>_dod<i>.<ext>` plus
    /// a combined `<docnum>_dodatk.md` (Markdown of all annexes).
    pub fn on_annex_save(&self) {
        self.spawn_annex_files();
    }

    /// «Прев'ю» in the preview window: convert the previewed document's
    /// embedded annexes to Markdown and show them INTERNALLY (the annex
    /// window) — no external Explorer / Word / PDF viewer. Uses the mode
    /// last selected in the annex window (plain text by default).
    pub fn on_annex_open(&self) {
        self.spawn_annex_preview(*self.annex_fmt.borrow());
    }

    /// «Текст» in the annex window: plain-text Markdown mode (the same
    /// content «Вивантажити» exports) and re-spawn the preview.
    pub fn on_annex_mode_text(&self) {
        *self.annex_fmt.borrow_mut() = false;
        self.annex_mode_label
            .set_text("режим: текст (як експорт .md)");
        self.spawn_annex_preview(false);
    }

    /// «Формат (md)» in the annex window: format-preserving Markdown mode
    /// (headings, **bold**, *italic*, lists) and re-spawn the preview.
    pub fn on_annex_mode_fmt(&self) {
        *self.annex_fmt.borrow_mut() = true;
        self.annex_mode_label
            .set_text("режим: формат (збереження оформлення)");
        self.spawn_annex_preview(true);
    }

    /// «Файли» in the annex window: write the raw annexes (PDF / Word /
    /// Excel / images…) to a temp folder and open it in Explorer — the
    /// system viewers open the files themselves (no in-app renderer).
    pub fn on_annex_files(&self) {
        let (docnum, items, baza) = match self.annex_spawn_prelude() {
            Some(t) => t,
            None => {
                if self.preview_doc.borrow().is_none() {
                    self.annex_status
                        .set_text("Додатки: спочатку відкрийте документ у перегляді");
                }
                return;
            }
        };
        let dir = std::env::temp_dir().join(format!("zakon_dod_{docnum}"));
        let _ = std::fs::create_dir_all(&dir);
        let (tx, rx) = channel();
        *self.rx.borrow_mut() = Some(rx);
        let gen = {
            let mut g = self.gen.borrow_mut();
            *g += 1;
            *g
        };
        self.annex_status
            .set_text(&format!("файли додатків… ({} шт.)", items.len()));
        thread::spawn(move || annex_files_open_worker(tx, baza, docnum, items, gen, dir));
    }

    fn annex_spawn_prelude(&self) -> Option<(u32, Vec<(String, usize, usize)>, PathBuf)> {
        let docnum = match *self.preview_doc.borrow() {
            Some(d) => d,
            None => return None,
        };
        let items = self.preview_annexes.borrow().clone();
        if items.is_empty() {
            self.annex_status
                .set_text("У цього документа немає вбудованих додатків");
            return None;
        }
        Some((docnum, items, self.bases_current().1))
    }

    fn spawn_annex_files(&self) {
        let (docnum, items, baza) = match self.annex_spawn_prelude() {
            Some(t) => t,
            None => {
                if self.preview_doc.borrow().is_none() {
                    self.annex_status
                        .set_text("Додатки: спочатку відкрийте документ у перегляді");
                }
                return;
            }
        };
        let naming = (*self.naming).clone();
        let dir = self.out_edit.text().trim().to_string();
        let target = if !dir.is_empty() {
            PathBuf::from(dir)
        } else {
            PathBuf::from(naming.out_dir.clone())
        };
        let _ = std::fs::create_dir_all(&target);
        let (tx, rx) = channel();
        *self.rx.borrow_mut() = Some(rx);
        let gen = {
            let mut g = self.gen.borrow_mut();
            *g += 1;
            *g
        };
        self.annex_status
            .set_text(&format!("вивантаження додатків… ({} шт.)", items.len()));
        thread::spawn(move || annex_files_worker(tx, baza, docnum, items, gen, target));
    }

    fn spawn_annex_preview(&self, fmt: bool) {
        let (docnum, items, baza) = match self.annex_spawn_prelude() {
            Some(t) => t,
            None => {
                if self.preview_doc.borrow().is_none() {
                    self.annex_status
                        .set_text("Додатки: спочатку відкрийте документ у перегляді");
                }
                return;
            }
        };
        let (tx, rx) = channel();
        *self.rx.borrow_mut() = Some(rx);
        let gen = {
            let mut g = self.gen.borrow_mut();
            *g += 1;
            *g
        };
        self.annex_status
            .set_text(&format!("прев'ю додатків… ({} шт.)", items.len()));
        thread::spawn(move || annex_preview_worker(tx, baza, docnum, items, gen, fmt));
    }

    /// System close (X / Alt+F4) on the internal annex window: hide, do NOT
    /// destroy — the window is reopened by «Прев'ю».
    pub fn on_annex_win_close(&self, evt: &nwg::EventData) {
        if let nwg::EventData::OnWindowClose(wcd) = evt {
            wcd.close(false);
            self.annex_window.set_visible(false);
        }
    }

    pub fn on_annex_hide(&self) {
        self.annex_window.set_visible(false);
    }

    /// System close (X / Alt+F4): hide, do NOT destroy — keep reopenable.
    pub fn on_preview_close(&self, evt: &nwg::EventData) {
        if let nwg::EventData::OnWindowClose(wcd) = evt {
            wcd.close(false);
            self.preview_window.set_visible(false);
        }
    }

    pub fn on_preview_hide(&self) {
        self.preview_window.set_visible(false);
    }

    /// Add (or just select) an arbitrary base folder in the«База:» combo.
    pub fn on_pick_base(&self) {
        let parent = match self.window.handle.hwnd() {
            Some(h) => h,
            None => return,
        };
        let current = self.bases_current().1;
        match crate::picker::pick_folder(parent, Some(&current)) {
            Some(p) => {
                // remember real bases (with a text\ subfolder) for future runs
                if p.join("text").is_dir() {
                    let _ = crate::save_saved_baza(&p);
                }
                let existing = self
                    .bases
                    .borrow()
                    .iter()
                    .position(|(_, bp)| *bp == p);
                let idx = match existing {
                    Some(i) => i,
                    None => {
                        let label = base_label_for(&p);
                        self.combo_base.push(label.clone());
                        self.bases.borrow_mut().push((label, p.clone()));
                        self.bases.borrow().len() - 1
                    }
                };
                self.combo_base.set_selection(Some(idx));
                self.status.set_text(0, &format!("База: {}", p.display()));
            }
            None => self.status.set_text(0, "Вибір бази скасовано"),
        }
    }

    /// «Про програму» — name, version, copyright (native message box).
    pub fn on_about(&self) {
        use winapi::um::winuser::{MB_ICONINFORMATION, MB_OK};
        message_box(
            self.window.handle.hwnd().unwrap_or(std::ptr::null_mut()),
            &crate::about_text(),
            "Про програму",
            MB_OK | MB_ICONINFORMATION,
        );
    }

    // ------------------------------------------------------------------
    // update task («Оновлення…» dialog)
    // ------------------------------------------------------------------

    /// Open the update-task dialog: loads (or seeds) update.toml and fills
    /// the title + command box with the current task.
    pub fn on_update(&self) {
        let t = crate::update::load();
        self.upd_title_edit.set_text(&t.title);
        self.upd_text.set_text_unix2dos(&t.commands.join("\n"));
        self.upd_status
            .set_text("Перевірте команди та натисніть «Запустити»");
        self.update_window.set_text(&t.title);
        self.update_window.set_visible(true);
    }

    /// Non-empty trimmed lines of the command box (one command per line).
    fn upd_script_lines(&self) -> Vec<String> {
        self.upd_text
            .text()
            .split(['\r', '\n'])
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect()
    }

    pub fn on_update_save(&self) {
        let t = crate::update::Task {
            title: self.upd_title_edit.text(),
            commands: self.upd_script_lines(),
        };
        match crate::update::save(&t) {
            Ok(()) => self.upd_status.set_text("Збережено в update.toml"),
            Err(e) => self.upd_status.set_text(&format!("Помилка збереження: {e}")),
        }
    }

    pub fn on_update_run(&self) {
        if *self.update_running.borrow() {
            self.upd_status
                .set_text("Виконується — зачекайте завершення");
            return;
        }
        let cmds = self.upd_script_lines();
        if cmds.is_empty() {
            self.upd_status.set_text("Немає команд для виконання");
            return;
        }
        // remember the edits: the running task is exactly what's on screen
        let t = crate::update::Task {
            title: self.upd_title_edit.text(),
            commands: cmds.clone(),
        };
        let _ = crate::update::save(&t);
        *self.gen.borrow_mut() += 1; // drop stale search/export messages
        let (tx, rx) = channel();
        *self.rx.borrow_mut() = Some(rx);
        *self.update_running.borrow_mut() = true;
        self.btn_update_run.set_enabled(false);
        self.upd_status
            .set_text(&format!("Запущено… 0/{}", cmds.len()));
        thread::spawn(move || update_worker(tx, cmds));
    }

    pub fn on_update_click_close(&self) {
        self.update_window.set_visible(false);
    }

    pub fn on_update_close(&self, evt: &nwg::EventData) {
        if let nwg::EventData::OnWindowClose(wcd) = evt {
            wcd.close(false);
        }
        self.update_window.set_visible(false);
    }

    fn show_preview(&self, docnum: u32, fl: &Option<fields::DocFields>, text: &str) {
        // cache for in-place re-render on a preview-mode switch
        *self.preview_cache.borrow_mut() = Some((docnum, fl.clone(), text.to_string()));
        let fmt = *self.preview_fmt.borrow();
        let body = if fmt {
            crate::writers::doc_markdown(fl.as_ref(), text)
        } else {
            let header = match fl {
                Some(f) => f.display(),
                None => "Поля документа не розібрано".to_string(),
            };
            format!("{header}\n\n{text}")
        };
        let shown = if body.chars().count() > PREVIEW_CAP {
            let cut: String = body.chars().take(PREVIEW_CAP).collect();
            format!("{cut}\n\n… текст обрізано (показано перші {PREVIEW_CAP} символів)")
        } else {
            body
        };
        self.preview_text.set_text_unix2dos(&shown);
        // window title + mode label reflect the active mode
        let title = format!(
            "Перегляд — документ {docnum}{}",
            if fmt { " (формат md)" } else { "" }
        );
        self.preview_window.set_text(&title);
        self.preview_mode_label
            .set_text(if fmt { "режим: формат (md)" } else { "режим: текст" });
        self.preview_window.set_visible(true);
    }

    /// «Текст» in the preview window: plain-text mode (the base's own line
    /// structure — the same as the .md export body).
    pub fn on_preview_mode_text(&self) {
        *self.preview_fmt.borrow_mut() = false;
        self.redraw_preview();
    }

    /// «Формат (md)» in the preview window: format-preserving Markdown mode
    /// (# title, *requisites*, **ALL-CAPS headers**, - lists).
    pub fn on_preview_mode_fmt(&self) {
        *self.preview_fmt.borrow_mut() = true;
        self.redraw_preview();
    }

    /// Re-render the cached previewed document with the current mode (no
    /// base re-read). A no-op until the first document is previewed.
    fn redraw_preview(&self) {
        let Some((docnum, fl, text)) = self.preview_cache.borrow().clone() else {
            return;
        };
        self.show_preview(docnum, &fl, &text);
    }

    /// «Друк» in the preview window: GDI print of the cached document (the
    /// system print dialog + a printer DC, no external apps). The PLAIN
    /// document text is printed (GDI cannot render Markdown), but the
    /// layout is preserved: ALL-CAPS lines bold, list items "•  ",
    /// centered lines centered. Modal: the UI is blocked while the dialog
    /// is open.
    pub fn on_preview_print(&self) {
        let (docnum, fl, text) = match self.preview_cache.borrow().clone() {
            Some(c) => c,
            None => {
                self.preview_status
                    .set_text("Друк: спочатку відкрийте документ у перегляді");
                return;
            }
        };
        let header = match &fl {
            Some(f) => f.display(),
            None => format!("Документ {docnum}"),
        };
        let full = format!("{header}\n\n{text}");
        match crate::printdoc::print_text(&format!("Документ {docnum}"), &full) {
            Ok(crate::printdoc::PrintOutcome::Sent) => {
                self.preview_status
                    .set_text("Друк: завдання надіслано в принтер");
            }
            Ok(crate::printdoc::PrintOutcome::Cancelled) => {
                self.preview_status.set_text("Друк: відмінено");
            }
            Err(e) => {
                self.preview_status
                    .set_text(&format!("Помилка друку: {e}"));
            }
        }
    }

    // ------------------------------------------------------------------
    // display
    // ------------------------------------------------------------------

    fn clear_list(&self) {
        self.list.clear();
        self.row_hits.borrow_mut().clear();
        self.sel.borrow_mut().clear();
        self.update_counts();
    }

    /// Redraw the header sort arrows from the current sort state.
    fn draw_sort_arrow(&self) {
        let (col, desc) = *self.sort.borrow();
        for c in 0..=COL_CONTEXT {
            self.list.set_column_sort_arrow(c, None);
        }
        let dir = if desc {
            nwg::ListViewColumnSortArrow::Down
        } else {
            nwg::ListViewColumnSortArrow::Up
        };
        self.list.set_column_sort_arrow(col, Some(dir));
    }

    /// Rebuild rows: current filters + current sort over the last search.
    /// A hit's ✓ mark follows it across resorts and filter re-applies:
    /// the checked hit indices are captured before the rebuild.
    fn show_filtered(&self, f: &GuiFilters) {
        let checked: std::collections::HashSet<usize> = {
            let rows = self.row_hits.borrow();
            let s = self.sel.borrow();
            rows.iter().zip(s.iter()).filter(|(_, &c)| c).map(|(&h, _)| h).collect()
        };
        let (row_hits, rows): (Vec<usize>, Vec<[String; 9]>) = {
            let r = self.results.borrow();
            let (hits, fields) = &*r;
            let mut idx: Vec<usize> = Vec::new();
            for (i, fl) in fields.iter().enumerate() {
                if filter_passes(f, fl.as_ref()) {
                    idx.push(i);
                }
            }
            let (col, desc) = *self.sort.borrow();
            sort_indices(&mut idx, col, desc, hits, fields);
            let shown = idx.len().min(DISPLAY_CAP);
            let mut row_hits: Vec<usize> = Vec::with_capacity(shown);
            let mut rows: Vec<[String; 9]> = Vec::with_capacity(shown);
            for &hi in idx.iter().take(shown) {
                let h = &hits[hi];
                let fl = fields[hi].as_ref();
                let dash = "—";
                // «назва» = the document's own title field (— if unparsed);
                // «контекст» = the search-context snippet around the first
                // match (kept separate so it can't be mistaken for a title)
                let title = match fl.and_then(|x| x.title.clone()) {
                    Some(t) => t,
                    None => dash.to_string(),
                };
                let context = if h.title.is_empty() {
                    dash.to_string()
                } else {
                    h.title.clone()
                };
                rows.push([
                    if checked.contains(&hi) { "✓".to_string() } else { String::new() },
                    h.file.clone(),
                    h.docnum.to_string(),
                    fl.and_then(|x| x.date.clone()).unwrap_or_else(|| dash.to_string()),
                    fl.and_then(|x| x.number.clone()).unwrap_or_else(|| dash.to_string()),
                    fl.and_then(|x| x.issuer.clone()).unwrap_or_else(|| dash.to_string()),
                    fl.map(|x| x.status.label().to_string())
                        .unwrap_or_else(|| "?".to_string()),
                    title,
                    context,
                ]);
                row_hits.push(hi);
            }
            (row_hits, rows)
        };
        let total_pass = row_hits.len();
        let shown = rows.len();

        self.list.clear();
        self.list.set_redraw(false);
        for row in rows {
            self.list.insert_items_row(None, &row);
        }
        self.list.set_redraw(true);
        self.list.invalidate();

        let sel: Vec<bool> = row_hits.iter().map(|h| checked.contains(h)).collect();
        *self.row_hits.borrow_mut() = row_hits;
        *self.sel.borrow_mut() = sel;
        // rows were rebuilt: a ListView selection / a remembered click now
        // points at an arbitrary document — drop both so «Перегляд» can
        // never resolve to something the user is not looking at. Without a
        // fresh click it falls back to the first ✓ mark.
        let n = self.sel.borrow().len();
        for r in 0..n {
            self.list.select_item(r, false);
        }
        *self.last_click.borrow_mut() = None;
        self.draw_sort_arrow();
        self.update_counts();
        let more = if total_pass > shown {
            format!(" (показано перші {shown})")
        } else {
            String::new()
        };
        self.status.set_text(
            0,
            &format!("Знайдено {total_pass}{more}. Виберіть документи (✓) та експортуйте."),
        );
    }

    // ------------------------------------------------------------------
    // message pump
    // ------------------------------------------------------------------

    pub fn on_tick(&self) {
        let msgs: Vec<GuiMsg> = {
            let mut slot = self.rx.borrow_mut();
            match slot.take() {
                Some(rx) => {
                    let mut out = Vec::new();
                    while let Ok(m) = rx.try_recv() {
                        out.push(m);
                    }
                    *slot = Some(rx);
                    out
                }
                None => Vec::new(),
            }
        };
        let cur_gen = *self.gen.borrow();
        for m in msgs {
            match m {
                GuiMsg::SearchProgress { gen, done, total, found } if gen == cur_gen => {
                    self.status
                        .set_text(0, &format!("Пошук… {done}/{total} файлів, знайдено {found}"));
                }
                GuiMsg::FieldsProgress { gen, done, total } if gen == cur_gen => {
                    self.status
                        .set_text(0, &format!("Обробка полів… {done}/{total}"));
                }
                GuiMsg::SearchDone { gen, hits, fields } if gen == cur_gen => {
                    let n = hits.len();
                    *self.results.borrow_mut() = (hits, fields);
                    // fresh results: default sort = by date, newest first
                    *self.sort.borrow_mut() = (COL_SORT_DEFAULT, true);
                    // stale checks from the previous result set must NOT
                    // carry over: the hit indices of two searches point at
                    // unrelated documents
                    self.sel.borrow_mut().clear();
                    self.row_hits.borrow_mut().clear();
                    *self.last_click.borrow_mut() = None;
                    // bad filter controls must never hide a fresh result — fallback neutral
                    let f = self.local_filters().unwrap_or_else(GuiFilters::neutral);
                    self.show_filtered(&f);
                    self.update_counts();
                    if n == 0 {
                        self.status.set_text(
                            0,
                            "Нічого не знайдено — спробуйте інші слова або режим «будь-яке»",
                        );
                    }
                }
                GuiMsg::SearchFail { gen, msg } if gen == cur_gen => {
                    self.status.set_text(0, &msg);
                }
                GuiMsg::ExportProgress { gen, done, total } if gen == cur_gen => {
                    self.status.set_text(0, &format!("Експорт… {done}/{total}"));
                }
                GuiMsg::ExportDone { gen, exported, attached, out } if gen == cur_gen => {
                    let att = if attached > 0 {
                        format!(", {attached} додатк(ів)")
                    } else {
                        String::new()
                    };
                    self.status
                        .set_text(0, &format!("Експортовано {exported} файлів{att} → {out}"));
                    self.preview_status
                        .set_text(&format!("Готово: {exported} файл(ів){att} → {out}"));
                    if self.guitest {
                        *self.gt_export.borrow_mut() = Some((exported, out));
                    }
                }
                GuiMsg::ExportFail { gen, msg } if gen == cur_gen => {
                    self.status.set_text(0, &msg);
                    self.preview_status.set_text(&msg);
                }
                GuiMsg::PreviewDone { gen, docnum, fields, text } if gen == cur_gen => {
                    self.show_preview(docnum, &fields, &text);
                    self.status.set_text(0, &format!("Перегляд документа {docnum}"));
                    if self.guitest {
                        *self.gt_preview_done.borrow_mut() = true;
                    }
                }
                GuiMsg::PreviewFail { gen, msg } if gen == cur_gen => {
                    self.status.set_text(0, &msg);
                }
                GuiMsg::AnnexesDone { gen, docnum, items } if gen == cur_gen => {
                    if *self.preview_doc.borrow() != Some(docnum) {
                        continue; // a newer document superseded this one
                    }
                    *self.preview_annexes.borrow_mut() = items.clone();
                    if items.is_empty() {
                        self.annex_status
                            .set_text("Вбудованих додатків у цьому документі немає");
                    } else {
                        let list: Vec<String> = items
                            .iter()
                            .enumerate()
                            .map(|(i, (e, _, _))| format!("{}) .{e}", i + 1))
                            .collect();
                        self.annex_status.set_text(&format!(
                            "Додатки (вбудовані): {} — {}",
                            items.len(),
                            list.join(", ")
                        ));
                    }
                }
                GuiMsg::AnnexFiles { gen, count, dir } if gen == cur_gen => {
                    let m = format!(
                        "Вивантажено {count} додатк(ів) + .md → {dir}"
                    );
                    self.annex_status.set_text(&m);
                    self.status.set_text(0, &m);
                }
                GuiMsg::AnnexFilesOpen { gen, count, dir } if gen == cur_gen => {
                    let m = format!(
                        "Файли: {count} додатк(ів) → {dir} (Explorer, системні переглядачі)"
                    );
                    self.annex_status.set_text(&m);
                    self.status.set_text(0, &m);
                }
                GuiMsg::AnnexPreview { gen, count, md } if gen == cur_gen => {
                    self.annex_text.set_text_unix2dos(&md);
                    self.annex_window.set_visible(true);
                    // sync the mode label + window title with the mode used
                    // (in nwg 1.0 a window's title is its "text")
                    self.annex_mode_label.set_text(if *self.annex_fmt.borrow() {
                        "режим: формат (збереження оформлення)"
                    } else {
                        "режим: текст (як експорт .md)"
                    });
                    self.annex_window.set_text(if *self.annex_fmt.borrow() {
                        "Додатки — прев'ю (оформлення збережено)"
                    } else {
                        "Додатки — прев'ю (текст, як експорт .md)"
                    });
                    self.annex_status
                        .set_text(&format!("Прев'ю: {count} додатк(ів) відображено (md)"));
                }
                GuiMsg::AnnexFail { gen, msg } if gen == cur_gen => {
                    self.annex_status.set_text(&msg);
                    self.status.set_text(0, &msg);
                }
                // update task — no gen guard: a single update at a time
                GuiMsg::UpdateCmd { idx, total, cmd } => {
                    let short = if cmd.chars().count() > 90 {
                        let cut: String = cmd.chars().take(90).collect();
                        format!("{cut}…")
                    } else {
                        cmd
                    };
                    self.upd_status
                        .set_text(&format!("Команда {idx}/{total}: {short}"));
                    self.status
                        .set_text(0, &format!("Оновлення… команда {idx}/{total}"));
                }
                GuiMsg::UpdateResult { idx, total, ok, note } => {
                    let mark = if ok { "готово" } else { "ПОМИЛКА" };
                    self.status
                        .set_text(0, &format!("Оновлення [{idx}/{total}]: {mark}"));
                    self.upd_status
                        .set_text(&format!("[{idx}/{total}] {mark}: {note}"));
                }
                GuiMsg::UpdateDone { ok, fail } => {
                    *self.update_running.borrow_mut() = false;
                    self.btn_update_run.set_enabled(true);
                    if fail == 0 {
                        self.upd_status
                            .set_text(&format!("Готово ({ok} команд). Журнал: update.log"));
                        self.status.set_text(0, "Оновлення баз завершено");
                    } else {
                        self.upd_status.set_text(&format!(
                            "Помилок: {fail} з {}. Журнал: update.log",
                            ok + fail
                        ));
                        self.status.set_text(0, "Оновлення завершено з помилками");
                    }
                }
                _ => {} // stale generation, drop
            }
        }

        if self.guitest {
            let tick = {
                let mut t = self.ticks.borrow_mut();
                *t += 1;
                *t
            };
            let finished = {
                if tick == 8 {
                    // stage 1: drive a real search on the GK base, headless
                    *self.gt_stage.borrow_mut() = 1;
                    if let Some(i) = self
                        .bases
                        .borrow()
                        .iter()
                        .position(|(l, _)| l.contains("Держаудит"))
                    {
                        self.combo_base.set_selection(Some(i));
                    }
                    // assert the export-path override is honored
                    self.out_edit
                        .set_text(r"C:\temp\opencode\zakon-exporter\gt_export");
                    self.query_edit.set_text("роз'яснення");
                    self.on_search();
                } else {
                    let stage = *self.gt_stage.borrow();
                    match stage {
                        1 => {
                            // SearchDone processed and rows displayed?
                            if !self.row_hits.borrow().is_empty() {
                                self.toggle_row(0);
                                self.toggle_row(1);
                                *self.gt_stage.borrow_mut() = 2;
                            }
                        }
                        2 => {
                            // "зв'язка" mode: all exported documents go into
                            // ONE file (bundle-2-<first>.<fmt> in the run dir)
                            self.chk_bundle.set_check_state(nwg::CheckBoxState::Checked);
                            // v0.12.0: term highlighting — the query terms
                            // must appear as <mark> in the exported md bundle
                            self.chk_hl.set_check_state(nwg::CheckBoxState::Checked);
                            self.on_export_selected();
                            *self.gt_stage.borrow_mut() = 3;
                        }
                        3 => {
                            // export finished → drive the preview window too
                            let exported = self.gt_export.borrow().is_some();
                            if exported {
                                self.on_preview();
                                *self.gt_stage.borrow_mut() = 4;
                            }
                        }
                        4 => {
                            if *self.gt_preview_done.borrow() {
                                let (exported, out) = self
                                    .gt_export
                                    .borrow_mut()
                                    .take()
                                    .unwrap_or((0, "-".to_string()));
                                let rows = self.row_hits.borrow().len();
                                let sel = self.selected_count();
                                let total = self.results.borrow().0.len();
                                let chars = self.preview_text.len() as usize;
                                let title = self.preview_window.text();
                                // files written into the export run dir (a
                                // "зв'язка" bundle = 1 file for N documents)
                                let files = std::fs::read_dir(&out)
                                    .map(|rd| rd.filter_map(|e| e.ok()).count())
                                    .unwrap_or(0);
                                // v0.12.0: highlight check — count the
                                // `<mark>` spans the exported file(s) carry
                                let mut marks = 0usize;
                                if let Ok(rd) = std::fs::read_dir(&out) {
                                    for e in rd.filter_map(|e| e.ok()) {
                                        if let Ok(s) = std::fs::read_to_string(e.path()) {
                                            marks += s.matches("<mark>").count();
                                        }
                                    }
                                }
                                let _ = std::fs::write(
                                    GUI_READY,
                                    format!(
                                        "GUI OK rows={rows} selected={sel} exported={exported} files={files} marks={marks} total={total} out={out} preview_chars={chars} ptitle={title}\n"
                                    ),
                                );
                                *self.gt_stage.borrow_mut() = 5;
                            }
                        }
                        _ => {}
                    }
                }
                if tick >= 240 && *self.gt_stage.borrow() != 5 {
                    let rows = self.row_hits.borrow().len();
                    let stage = *self.gt_stage.borrow();
                    let _ = std::fs::write(
                        GUI_READY,
                        format!("GUI TIMEOUT stage={stage} rows={rows}\n"),
                    );
                    true
                } else {
                    *self.gt_stage.borrow() == 5
                }
            };
            if finished {
                nwg::stop_thread_dispatch();
            }
        }
    }
}

// ============================================================
// workers
// ============================================================

fn search_worker(
    tx: Sender<GuiMsg>,
    baza: PathBuf,
    query: String,
    kind: SearchKind,
    gen: usize,
) {
    let res: anyhow::Result<(Vec<db::Hit>, Vec<Option<fields::DocFields>>)> = (|| {
        let base = render::Base::load(&baza)?;
        let progress = |d: usize, t: usize, fd: usize| {
            let _ = tx.send(GuiMsg::SearchProgress { gen, done: d, total: t, found: fd });
            true
        };
        let hits = match kind {
            SearchKind::And => {
                let terms = db::build_terms(&query, db::MatchMode::And)?;
                db::search_base(&base, &baza, &terms, db::MatchMode::And, &progress)?
            }
            SearchKind::Or => {
                let terms = db::build_terms(&query, db::MatchMode::Or)?;
                db::search_base(&base, &baza, &terms, db::MatchMode::Or, &progress)?
            }
            SearchKind::Phrase => {
                let terms = db::build_terms(&query, db::MatchMode::Phrase)?;
                db::search_base(&base, &baza, &terms, db::MatchMode::Phrase, &progress)?
            }
            // number query parse errors (query doesn't look like a number)
            // surface as a plain status message, not a crash
            SearchKind::Number => {
                let core = db::parse_number_query(&query)?;
                db::search_number_base(&base, &baza, &core, &progress)?
            }
        };
        let total = hits.len();
        let mut fs: Vec<Option<fields::DocFields>> = vec![None; total];
        let cap = total.min(FIELD_PARSE_CAP);
        for (i, h) in hits.iter().enumerate().take(cap) {
            let text = base.doc_text(&baza, h.docnum).unwrap_or_default();
            fs[i] = Some(fields::parse(h.docnum, &text));
            if i % 256 == 255 {
                let _ = tx.send(GuiMsg::FieldsProgress { gen, done: i + 1, total });
            }
        }
        Ok((hits, fs))
    })();

    match res {
        Ok((hits, fields)) => {
            let _ = tx.send(GuiMsg::SearchDone { gen, hits, fields });
        }
        Err(e) => {
            let _ = tx.send(GuiMsg::SearchFail { gen, msg: format!("Помилка пошуку: {e}") });
        }
    }
}

/// Experimental native-index search (Trident, `SOFT\`): the document
/// numbers are the native 2005-era Trident ids, so the field columns stay
/// dashed (`fields = None`) until the doc-number join lands (v0.14.0).
fn trident_search_worker(
    tx: Sender<GuiMsg>,
    baza: PathBuf,
    query: String,
    gen: usize,
) {
    match crate::trident::TridentIndex::open(&baza) {
        Some(idx) => {
            let r = idx.search(&query);
            let pct = (r.coverage * 100.0).round() as u32;
            let hits: Vec<db::Hit> = r
                .docs
                .into_iter()
                .map(|docnum| db::Hit {
                    docnum,
                    file: "trident (native id)".to_string(),
                    match_off: 0,
                    matches: 1,
                    title: String::new(),
                })
                .collect();
            let fields = vec![None; hits.len()];
            if hits.is_empty() {
                let _ = tx.send(GuiMsg::SearchFail {
                    gen,
                    msg: format!(
                        "Trident: «{query}» — 0 документів (покриття знаків {pct}%, \
                         словник: {} слів, vt-потік: {} u16 — експериментальний рушій)",
                        r.dict_size, r.vt_size
                    ),
                });
            } else {
                let _ = tx.send(GuiMsg::SearchDone { gen, hits, fields });
            }
        }
        None => {
            let _ = tx.send(GuiMsg::SearchFail {
                gen,
                msg: "В базі немає робочого Trident-індексу (SOFT\\lmu.voc + lmur.vt) — \
                      зніміть прапорець «Trident-рушій»".to_string(),
            });
        }
    }
}

fn export_worker(
    tx: Sender<GuiMsg>,
    baza: PathBuf,
    hits: Vec<db::Hit>,
    format: String,
    naming: Naming,
    gen: usize,
    attach: bool,
    single: bool,
    hl_on: bool,
    hl_query: String,
) {
    let progress = |done: usize, t: usize| {
        let _ = tx.send(GuiMsg::ExportProgress { gen, done, total: t });
    };
    let hl = if hl_on {
        crate::highlight::Hl::from_query(&hl_query)
    } else {
        crate::highlight::Hl::default()
    };
    match writers::export_hits(&baza, &hits, &format, &naming, Some(&progress), attach, single, &hl) {
        Ok((exported, attached, out)) => {
            let _ = tx.send(GuiMsg::ExportDone {
                gen,
                exported,
                attached,
                out: out.to_string_lossy().to_string(),
            });
        }
        Err(e) => {
            let _ = tx.send(GuiMsg::ExportFail { gen, msg: format!("Помилка експорту: {e}") });
        }
    }
}

fn preview_worker(
    tx: Sender<GuiMsg>,
    baza: PathBuf,
    docnum: u32,
    fields: Option<fields::DocFields>,
    gen: usize,
) {
    let res: anyhow::Result<(String, Vec<(String, usize, usize)>)> = (|| {
        let base = render::Base::load(&baza)?;
        let text = base.doc_text(&baza, docnum)?;
        // embedded annexes: BZh9 streams inside this document's own chunk
        let chunk = base.chunk(&baza, docnum)?;
        let annexes = crate::attach::find_annexes(&chunk);
        let items = annexes
            .iter()
            .map(|a| (a.ext.clone(), a.start, a.end))
            .collect();
        Ok((text, items))
    })();
    match res {
        Ok((text, items)) => {
            let _ = tx.send(GuiMsg::AnnexesDone { gen, docnum, items });
            let _ = tx.send(GuiMsg::PreviewDone { gen, docnum, fields, text });
        }
        Err(e) => {
            let _ = tx.send(GuiMsg::PreviewFail {
                gen,
                msg: format!("Помилка завантаження документа {docnum}: {e}"),
            });
        }
    }
}

/// Decode + write the previewed document's embedded annexes to `target`:
/// each annex as `<docnum>_dod<i>.<ext>` plus a combined Markdown file
/// `<docnum>_dodatk.md` (all annexes, converted by mdconv). Reports via
/// AnnexFiles / AnnexFail.
fn annex_files_worker(
    tx: Sender<GuiMsg>,
    baza: PathBuf,
    docnum: u32,
    items: Vec<(String, usize, usize)>,
    gen: usize,
    target: PathBuf,
) {
    let res: anyhow::Result<(usize, String)> = (|| {
        let base = render::Base::load(&baza)?;
        let chunk = base.chunk(&baza, docnum)?;
        let mut decoded: Vec<(String, Vec<u8>)> = Vec::new();
        for (i, item) in items.iter().enumerate() {
            let a = crate::attach::Annex {
                ext: item.0.clone(),
                start: item.1,
                end: item.2,
            };
            let (ext, bytes) = crate::attach::decode(&chunk, &a);
            if bytes.is_empty() {
                continue; // undecodable stream — skip, keep going
            }
            let name = format!("{docnum}_dod{i}.{ext}");
            std::fs::write(target.join(name), &bytes)?;
            decoded.push((ext, bytes));
        }
        // the combined Markdown export of all annexes (the «як md» part)
        let md = crate::mdconv::annexes_markdown(docnum, &decoded);
        std::fs::write(target.join(format!("{docnum}_dodatk.md")), md.as_bytes())?;
        Ok((decoded.len(), target.to_string_lossy().to_string()))
    })();
    match res {
        Ok((count, dir)) => {
            let _ = tx.send(GuiMsg::AnnexFiles { gen, count, dir });
        }
        Err(e) => {
            let _ = tx.send(GuiMsg::AnnexFail {
                gen,
                msg: format!("Помилка додатків до док. {docnum}: {e}"),
            });
        }
    }
}

/// Write the previewed document's RAW annexes (no conversion) to `dir` and
/// open that folder in Explorer — the system viewers (PDF viewer / Word /
/// Excel) open the files themselves. Reports via AnnexFilesOpen / AnnexFail.
fn annex_files_open_worker(
    tx: Sender<GuiMsg>,
    baza: PathBuf,
    docnum: u32,
    items: Vec<(String, usize, usize)>,
    gen: usize,
    dir: PathBuf,
) {
    let res: anyhow::Result<(usize, String)> = (|| {
        let base = render::Base::load(&baza)?;
        let chunk = base.chunk(&baza, docnum)?;
        let mut n = 0usize;
        for (i, item) in items.iter().enumerate() {
            let a = crate::attach::Annex {
                ext: item.0.clone(),
                start: item.1,
                end: item.2,
            };
            let (ext, bytes) = crate::attach::decode(&chunk, &a);
            if bytes.is_empty() {
                continue; // undecodable stream — skip, keep going
            }
            let name = format!("{docnum}_dod{i}.{ext}");
            std::fs::write(dir.join(name), &bytes)?;
            n += 1;
        }
        // open the folder in Explorer (double-click a file → system viewer)
        let d = dir.to_string_lossy().to_string();
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", "", &d])
            .spawn();
        Ok((n, d))
    })();
    match res {
        Ok((count, dir)) => {
            let _ = tx.send(GuiMsg::AnnexFilesOpen { gen, count, dir });
        }
        Err(e) => {
            let _ = tx.send(GuiMsg::AnnexFail {
                gen,
                msg: format!("Помилка файлів додатків до док. {docnum}: {e}"),
            });
        }
    }
}

/// Decode the previewed document's embedded annexes and convert them to
/// Markdown for the internal annex-preview window (`fmt` = format-preserving
/// Markdown; false = plain text, the same content «Вивантажити» exports).
/// Reports via AnnexPreview / AnnexFail.
fn annex_preview_worker(
    tx: Sender<GuiMsg>,
    baza: PathBuf,
    docnum: u32,
    items: Vec<(String, usize, usize)>,
    gen: usize,
    fmt: bool,
) {
    let res: anyhow::Result<String> = (|| {
        let base = render::Base::load(&baza)?;
        let chunk = base.chunk(&baza, docnum)?;
        let mut decoded: Vec<(String, Vec<u8>)> = Vec::new();
        for item in items.iter() {
            let a = crate::attach::Annex {
                ext: item.0.clone(),
                start: item.1,
                end: item.2,
            };
            let (ext, bytes) = crate::attach::decode(&chunk, &a);
            if !bytes.is_empty() {
                decoded.push((ext, bytes));
            }
        }
        Ok(if fmt {
            crate::mdconv::annexes_markdown_fmt(docnum, &decoded)
        } else {
            crate::mdconv::annexes_markdown(docnum, &decoded)
        })
    })();
    match res {
        Ok(md) => {
            let _ = tx.send(GuiMsg::AnnexPreview {
                gen,
                count: items.len(),
                md,
            });
        }
        Err(e) => {
            let _ = tx.send(GuiMsg::AnnexFail {
                gen,
                msg: format!("Помилка прев'ю додатків до док. {docnum}: {e}"),
            });
        }
    }
}

/// Runs the update-task commands sequentially (hidden cmd.exe via
/// update::run_one), reporting progress into the dialog + status bar.
fn update_worker(tx: Sender<GuiMsg>, cmds: Vec<String>) {    let total = cmds.len();
    let mut ok = 0usize;
    let mut fail = 0usize;
    for (i, c) in cmds.iter().enumerate() {
        let _ = tx.send(GuiMsg::UpdateCmd {
            idx: i + 1,
            total,
            cmd: c.clone(),
        });
        let res = crate::update::run_one(c);
        if res.ok {
            ok += 1;
        } else {
            fail += 1;
        }
        let _ = tx.send(GuiMsg::UpdateResult {
            idx: i + 1,
            total,
            ok: res.ok,
            note: format!("{} [код {}]", res.note, res.code),
        });
    }
    let _ = tx.send(GuiMsg::UpdateDone { ok, fail });
}

// ============================================================
// entry
// ============================================================

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// Set the title-bar icon (big + small) from exe resource #1 (injected by
/// res/inject_icon.ps1). Silent no-op when the resource is missing (debug
/// builds without injection).
unsafe fn set_window_icon(hwnd: winapi::shared::windef::HWND) {
    use winapi::um::libloaderapi::GetModuleHandleW;
    use winapi::um::winuser::{
        GetSystemMetrics, LoadImageW, SendMessageW, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_SHARED,
        SM_CXICON, SM_CXSMICON, SM_CYICON, SM_CYSMICON, WM_SETICON,
    };
    let mod_handle = GetModuleHandleW(std::ptr::null());
    if mod_handle.is_null() || hwnd.is_null() {
        return;
    }
    let name = 1usize as *const u16; // MAKEINTRESOURCEW(1)
    let small = LoadImageW(
        mod_handle,
        name,
        IMAGE_ICON,
        GetSystemMetrics(SM_CXSMICON),
        GetSystemMetrics(SM_CYSMICON),
        LR_SHARED,
    );
    let big = LoadImageW(
        mod_handle,
        name,
        IMAGE_ICON,
        GetSystemMetrics(SM_CXICON),
        GetSystemMetrics(SM_CYICON),
        LR_SHARED,
    );
    if !small.is_null() {
        SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, small as isize);
    }
    if !big.is_null() {
        SendMessageW(hwnd, WM_SETICON, ICON_BIG as usize, big as isize);
    }
}

fn set_window_icon_safe(hwnd: Option<winapi::shared::windef::HWND>) {
    if let Some(h) = hwnd {
        unsafe { set_window_icon(h) };
    }
}

fn base_label_for(p: &Path) -> String {
    if p == Path::new(crate::DEFAULT_BAZA) {
        "Загальне законодавство (основна база)".to_string()
    } else if p == Path::new(crate::BASE_AUDIT) {
        "Держаудитслужба / ГоловКРУ".to_string()
    } else {
        p.to_string_lossy().to_string()
    }
}

/// Native message box (also works before any window is built).
fn message_box(hwnd: winapi::shared::windef::HWND, text: &str, title: &str, flags: u32) {
    unsafe {
        use winapi::um::winuser::MessageBoxW;
        let t = utf16(title);
        let m = utf16(text);
        MessageBoxW(hwnd, m.as_ptr(), t.as_ptr(), flags);
    }
}

/// Startup prompt when no base directory exists (moved install): inform the
/// user, let them pick a folder, auto-resolve `Baza` under it and remember
/// the result. Cancelled pick -> Err (both exes show it as a fatal message).
fn ask_startup_base(baza: &mut PathBuf) -> anyhow::Result<()> {
    use winapi::um::winuser::{MB_ICONWARNING, MB_OK};
    message_box(
        std::ptr::null_mut(),
        &format!(
            "Базу не знайдено: {}\n\
             (програму запущено на іншому комп'ютері або базу переміщено)\n\n\
             Оберіть папку з базою «Закон» — наприклад C:\\Zakon.2\\Baza.\n\
             Шлях буде запам'ятовано для наступних запусків.",
            baza.display()
        ),
        "Zakon DB Exporter — розташування бази",
        MB_ICONWARNING | MB_OK,
    );
    let picked = crate::picker::pick_folder(std::ptr::null_mut(), None)
        .ok_or_else(|| anyhow::anyhow!("базу не обрано — запустіть з параметром --baza <шлях>"))?;
    let resolved = crate::auto_base_under(&picked).unwrap_or_else(|| picked.clone());
    if !resolved.join("text").is_dir() {
        return Err(anyhow::anyhow!(
            "Обрано папку без бази: {} (потрібна папка, що містить підпапку text)",
            resolved.display()
        ));
    }
    *baza = resolved.clone();
    let _ = crate::save_saved_baza(&resolved);
    Ok(())
}

pub fn run_gui(baza: PathBuf, guitest: bool, explicit: bool) -> anyhow::Result<()> {
    use nwg::NativeUi;
    nwg::init().map_err(|e| anyhow::anyhow!("nwg init failed: {}", e))?;

    // Base resolution. Rule (same for GUI and CLI verbs):
    //   explicit --baza/--base  -> used as-is (missing dir is an error)
    //   no explicit flag        -> remembered config path wins over the
    //                              built-in default; if nothing valid exists
    //                              the GUI asks which folder holds the base
    //                              and remembers the choice.
    // The headless guitest must never open dialogs or read the config.
    let mut baza = baza;
    if !guitest && !explicit {
        if let Some(saved) = crate::load_saved_baza() {
            baza = saved;
        }
        if !baza.is_dir() {
            ask_startup_base(&mut baza)?;
        }
    }
    if !baza.is_dir() {
        return Err(anyhow::anyhow!(
            "Базу не знайдено: {} (задайте --baza <шлях> або оберіть папку)",
            baza.display()
        ));
    }

    let naming = Rc::new(Naming::load(&baza));
    // bases list: the invoked base first, then the standard ones, deduped
    let mut bases: Vec<(String, PathBuf)> = Vec::new();
    let mut push_base = |p: &Path| {
        if !bases.iter().any(|(_, pb)| pb == p) {
            bases.push((base_label_for(p), p.to_path_buf()));
        }
    };
    push_base(&baza);
    push_base(Path::new(crate::BASE_AUDIT));
    push_base(Path::new(crate::DEFAULT_BAZA));

    let mut data = App::default();
    data.baza = baza.clone();
    data.bases = RefCell::new(bases.clone());
    data.guitest = guitest;
    data.naming = naming;

    let ui = App::build_ui(data).map_err(|e| anyhow::anyhow!("build gui: {}", e))?;

    for (label, _) in ui.bases.borrow().iter() {
        ui.combo_base.push(label.clone());
    }
    ui.combo_base.set_selection(Some(0));

    ui.combo_mode.push("Усі слова (І)".to_string());
    ui.combo_mode.push("Будь-яке слово (АБО)".to_string());
    ui.combo_mode.push("Фраза".to_string());
    ui.combo_mode.push("За номером (№)".to_string());
    ui.combo_mode.set_selection(Some(0));

    ui.combo_status.push("Статус: будь-який".to_string());
    ui.combo_status.push("чинний".to_string());
    ui.combo_status.push("не діє".to_string());
    ui.combo_status.push("не опубліковано".to_string());
    ui.combo_status.set_selection(Some(0));

    ui.list.set_headers_enabled(true);
    for (text, width, align) in [
        ("✓", 30, nwg::ListViewColumnFlags::CENTER),
        ("файл", 64, nwg::ListViewColumnFlags::LEFT),
        ("док.", 60, nwg::ListViewColumnFlags::RIGHT),
        ("дата", 80, nwg::ListViewColumnFlags::LEFT),
        ("номер", 100, nwg::ListViewColumnFlags::LEFT),
        ("видавець", 150, nwg::ListViewColumnFlags::LEFT),
        ("статус", 100, nwg::ListViewColumnFlags::LEFT),
        ("назва", 250, nwg::ListViewColumnFlags::LEFT),
        ("контекст", 280, nwg::ListViewColumnFlags::LEFT),
    ] {
        ui.list.insert_column(nwg::InsertListViewColumn {
            fmt: Some(align),
            width: Some(width),
            text: Some(text.to_string()),
            ..Default::default()
        });
    }

    ui.out_edit.set_text(&ui.naming.out_dir);

    // default sort: date, newest first (arrow drawn so the user sees it)
    *ui.sort.borrow_mut() = (COL_SORT_DEFAULT, true);
    ui.draw_sort_arrow();

    ui.combo_fmt.push("md".to_string());
    ui.combo_fmt.push("rtf".to_string());
    ui.combo_fmt.push("docx".to_string());
    ui.combo_fmt.push("html".to_string());
    ui.combo_fmt.push("csv".to_string());
    ui.combo_fmt.push("pdf".to_string());
    ui.combo_fmt.set_selection(Some(0));

    // version in the title + title-bar icons from the injected exe resource
    ui.window
        .set_text(&format!("{} {}", crate::APP_NAME, crate::VERSION));
    set_window_icon_safe(ui.window.handle.hwnd());
    set_window_icon_safe(ui.preview_window.handle.hwnd());

    nwg::dispatch_thread_events();
    Ok(())
}

// ============================================================
// tests (no window: pure sort helpers)
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    fn hit(docnum: u32, file: &str) -> db::Hit {
        db::Hit {
            docnum,
            file: file.to_string(),
            match_off: 0,
            matches: 1,
            title: String::new(),
        }
    }

    fn fld(date_ymd: Option<(i32, u8, u8)>, number: Option<&str>) -> fields::DocFields {
        fields::DocFields {
            docnum: 0,
            issuer: None,
            doc_type: None,
            date: None,
            date_ymd,
            number: number.map(String::from),
            title: None,
            status: fields::Status::Actual,
            status_detail: None,
            n_lines: 0,
            n_chars: 0,
        }
    }

    #[test]
    fn nat_cmp_numeric_runs() {
        assert_eq!(nat_cmp("9", "10"), Ordering::Less);
        assert_eq!(nat_cmp("249", "63"), Ordering::Greater);
        assert_eq!(nat_cmp("249/2", "249/10"), Ordering::Less);
        assert_eq!(nat_cmp("a2", "a10"), Ordering::Less);
        assert_eq!(nat_cmp("175-xiv", "249"), Ordering::Less);
        // case-insensitive letters («про …» sorts before «РОЗПОРЯДЖЕННЯ …»)
        assert_eq!(nat_cmp("про А", "про Б"), Ordering::Less);
        assert_eq!(nat_cmp("b", "A"), Ordering::Greater);
    }

    #[test]
    fn number_core_strips_marker() {
        let f = fld(None, Some("№ 249/2"));
        assert_eq!(number_core(Some(&f)), "249/2");
        let f = fld(None, Some("№ 5"));
        assert_eq!(number_core(Some(&f)), "5");
        assert_eq!(number_core(None), "");
    }

    #[test]
    fn sort_indices_date_desc_default() {
        // hits: 0→2018, 1→2024, 2→no date, 3→2018 (tie, later)
        let hits = vec![hit(10, "1.mb"), hit(11, "1.mb"), hit(12, "2.mb"), hit(13, "2.mb")];
        let fields = vec![
            Some(fld(Some((2018, 11, 8)), None)),
            Some(fld(Some((2024, 1, 15)), None)),
            None,
            Some(fld(Some((2018, 11, 8)), None)),
        ];
        let mut idx: Vec<usize> = (0..4).collect();
        sort_indices(&mut idx, COL_DATE, true, &hits, &fields);
        assert_eq!(idx, vec![1, 0, 3, 2]); // newest first; no-date last; ties stable
        sort_indices(&mut idx, COL_DATE, false, &hits, &fields);
        assert_eq!(idx, vec![0, 3, 1, 2]); // oldest first; no-date still last
    }

    #[test]
    fn sort_indices_docnum_and_number() {
        let hits = vec![hit(1200, "9.mb"), hit(63, "10.mb"), hit(249, "2.mb")];
        let fields = vec![
            Some(fld(None, Some("№ 1200"))),
            Some(fld(None, Some("№ 1200"))), // number tie → docnum decides? no: stable order
            Some(fld(None, Some("№ 249"))),
        ];
        let mut idx: Vec<usize> = (0..3).collect();
        sort_indices(&mut idx, COL_DOC, false, &hits, &fields);
        assert_eq!(idx, vec![1, 2, 0]); // docnums 63, 249, 1200
        sort_indices(&mut idx, COL_DOC, true, &hits, &fields);
        assert_eq!(idx, vec![0, 2, 1]);
        sort_indices(&mut idx, COL_NUMBER, false, &hits, &fields);
        // «249» < «1200» numerically; the tie keeps search order (0 before 1)
        assert_eq!(idx, vec![2, 0, 1]);
    }

    #[test]
    fn sort_indices_file_and_title() {
        let hits = vec![hit(1, "9.mb"), hit(2, "10.mb"), hit(3, "223.mb")];
        let fields = vec![None, None, None];
        let mut idx: Vec<usize> = (0..3).collect();
        sort_indices(&mut idx, COL_FILE, false, &hits, &fields);
        assert_eq!(idx, vec![0, 1, 2]); // 9 < 10 < 223 numerically
        sort_indices(&mut idx, COL_TITLE, true, &hits, &fields);
        // empty titles tie → stable search order (ignoring direction flip)
        assert_eq!(idx, vec![0, 1, 2]);
    }
}
