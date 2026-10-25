//! GDI printing of documents (Win32 FFI, pure Rust, no new dependencies):
//! system print dialog (comdlg32 `PrintDlgW` — comdlg32 is linked in
//! build.rs) + GDI text output. The layout keeps the document's own
//! formatting (the same rules as the format-preserving writers): ALL-CAPS
//! lines print bold (a second bold face), dash/bullet items get a "•  "
//! marker, centered lines (`writers::is_centered`) are centered.

use std::ffi::c_void;
use std::os::raw::{c_int, c_long, c_ushort};
use std::ptr;

/// What happened after a `print_text` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintOutcome {
    /// the job was sent to the selected printer
    Sent,
    /// the user pressed «Отмена» in the print dialog (not an error)
    Cancelled,
}

// ---------------------------------------------------------------------------
// Win32 FFI (comdlg32 / gdi32 / user32 / kernel32 — all already linked,
// comdlg32 added by build.rs)
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy)]
struct Size32 {
    cx: c_long,
    cy: c_long,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct PrintDlg2W {
    l_struct_size: u32,
    hwnd_owner: *mut c_void,
    h_dc: *mut c_void,
    h_document: *mut c_void,
    h_instance: *mut c_void,
    h_template: *mut c_void,
    flags: u32,
    n_page: u16,
    /// union { LPWORD vFromPage; LPWORD vToPage; }
    v_from_page: *mut c_void,
    v_to_page: *mut c_void,
    n_max_page_range: u16,
    p_points: *mut c_void,
    reserved: u32,
    psp_mode: *mut c_void,
    h_printer: *mut c_void,
    p_result: *mut c_void,
    flags2: u32,
    n_max_filename: u32,
    pch_jobs: *mut u16,
    c_jobs: u32,
    p_ui_info: *mut c_void,
    ui_version: u32,
}

#[repr(C)]
struct DocInfoW {
    cb_size: u32,
    fw_type: u32,
    psz_doc_name: *const u16,
    psz_output: *const u16,
    h_instance: *mut c_void,
    h_bitmap: *mut c_void,
}

const PD_RETURNDC: u32 = 0x0000_0100;
const PD_NOPROOF: u32 = 0x0000_0200;
const HORZRES: c_int = 8;
const VERTRES: c_int = 10;

unsafe extern "system" {
    // comdlg32
    fn PrintDlgW(p: *mut PrintDlg2W) -> i32;
    // user32
    fn GetDCW(h: *mut c_void) -> *mut c_void;
    fn ReleaseDCW(h: *mut c_void, dc: *mut c_void) -> c_int;
    // gdi32
    fn DeleteDC(dc: *mut c_void) -> i32;
    fn GetDeviceCaps(dc: *mut c_void, index: c_int) -> c_int;
    fn CreateFontW(
        c_height: c_long,
        c_width: c_long,
        c_escapement: c_long,
        c_orientation: c_long,
        dw_weight: c_long,
        b_italic: c_ushort,
        b_underline: c_ushort,
        c_char_set: u8,
        c_out_precision: u8,
        c_clip_precision: u8,
        c_quality: u8,
        c_pitch_and_family: c_ushort,
        lpsz_face: *const u16,
    ) -> *mut c_void;
    fn SelectObject(dc: *mut c_void, o: *mut c_void) -> *mut c_void;
    fn DeleteObject(o: *mut c_void) -> i32;
    fn GetTextExtentPoint32W(
        dc: *mut c_void,
        s: *const u16,
        n: c_int,
        sz: *mut Size32,
    ) -> i32;
    fn TextOutW(
        dc: *mut c_void,
        x: c_int,
        y: c_int,
        s: *const u16,
        n: c_int,
    ) -> i32;
    fn StartDocW(dc: *mut c_void, info: *const DocInfoW) -> c_int;
    fn StartPage(dc: *mut c_void) -> i32;
    fn EndPage(dc: *mut c_void) -> i32;
    fn EndDocW(dc: *mut c_void) -> c_int;
    // kernel32
    fn GetLastError() -> u32;
}

// ---------------------------------------------------------------------------
// printing
// ---------------------------------------------------------------------------

/// A logical print line with its style (bold / centered / bullet marker).
struct StyledLine {
    text: String,
    bold: bool,
    centered: bool,
}

/// Classify the document text into styled print lines (the same rules as
/// the format-preserving writers: `line_traits` + `is_centered`).
fn styled_lines(text: &str) -> Vec<StyledLine> {
    let mut out = Vec::new();
    for line in text.lines() {
        let (lead, t, caps, bullet) = crate::writers::line_traits(line);
        if t.is_empty() {
            out.push(StyledLine {
                text: String::new(),
                bold: false,
                centered: false,
            });
            continue;
        }
        if bullet {
            // GDI does not expand tabs — bullet + two spaces
            out.push(StyledLine {
                text: format!("•  {}", crate::writers::bullet_body(t)),
                bold: false,
                centered: false,
            });
            continue;
        }
        out.push(StyledLine {
            text: t.to_string(),
            bold: caps,
            centered: crate::writers::is_centered(lead, t, caps),
        });
    }
    out
}

fn create_font(weight: c_long) -> *mut c_void {
    // Times New Roman 12 pt (cHeight = -16 px at 96 dpi); the face is
    // always present on Windows, bold + regular.
    let mut face = [0u16; 32];
    for (i, ch) in "Times New Roman".encode_utf16().enumerate() {
        face[i] = ch;
    }
    unsafe {
        CreateFontW(
            -16, 0, 0, 0,
            weight, 0, 0,
            1 /*DEFAULT_CHARSET*/, 0, 0, 0,
            20 /*VARIABLE_PITCH | FF_ROMAN*/,
            face.as_ptr(),
        )
    }
}

fn measure(hdc: *mut c_void, text: &str) -> c_long {
    if text.is_empty() {
        return 0;
    }
    let u16: Vec<u16> = text.encode_utf16().collect();
    let mut sz = Size32 { cx: 0, cy: 0 };
    unsafe {
        GetTextExtentPoint32W(hdc, u16.as_ptr(), u16.len() as c_int, &mut sz);
    }
    sz.cx
}

/// Word-wrap a line by measured width; a single word wider than the whole
/// line is hard-split (character-count estimate).
fn wrap_line(
    hdc: *mut c_void,
    font: *mut c_void,
    bold_font: *mut c_void,
    text: &str,
    bold: bool,
    avail_w: c_long,
) -> Vec<String> {
    unsafe {
        let _ = SelectObject(hdc, if bold { bold_font } else { font });
    }
    if measure(hdc, text) <= avail_w {
        return vec![text.to_string()];
    }
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in text.split(' ') {
        if word.is_empty() {
            continue; // runs of spaces collapse (no need to print them)
        }
        let cand = if cur.is_empty() {
            word.to_string()
        } else {
            format!("{cur} {word}")
        };
        if measure(hdc, &cand) <= avail_w {
            cur = cand;
            continue;
        }
        if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        // the word itself is too wide: hard-split it
        let w_full = measure(hdc, word);
        let n_chars = word.chars().count().max(1);
        let per_char = (w_full / n_chars as c_long).max(1);
        let per_line = (avail_w / per_char).max(1);
        let mut rem = word;
        loop {
            let mut take = std::cmp::min(per_line as usize, rem.len());
            while take > 0 && !rem.is_char_boundary(take) {
                take -= 1;
            }
            let (head, tail) = rem.split_at(take);
            out.push(head.to_string());
            rem = tail;
            if rem.is_empty() {
                break;
            }
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Print a document via the system print dialog + GDI.
///
/// `doc_label` becomes the document name in the dialog / print queue;
/// `text` is printed with the document's own layout (bold ALL-CAPS lines,
/// "•  " list markers, centered lines). Cancelling the dialog is not an
/// error (`Ok(Cancelled)`).
pub fn print_text(doc_label: &str, text: &str) -> anyhow::Result<PrintOutcome> {
    // 1) system print dialog: the caller's hDC must be non-null (a dummy
    // screen DC is enough); PD_RETURNDC returns the chosen printer's DC.
    let dummy = unsafe { GetDCW(ptr::null_mut()) };
    let mut dlg = PrintDlg2W::default(); // zeroed
    dlg.l_struct_size = std::mem::size_of::<PrintDlg2W>() as u32;
    dlg.h_dc = dummy;
    dlg.flags = PD_RETURNDC | PD_NOPROOF;
    let accepted = unsafe { PrintDlgW(&mut dlg) };
    unsafe {
        let _ = ReleaseDCW(ptr::null_mut(), dummy);
    }
    if accepted == 0 {
        return Ok(PrintOutcome::Cancelled);
    }
    let hdc = dlg.h_dc; // printer DC (PD_RETURNDC)

    // 2) fonts + page geometry (device units at the printer's resolution)
    let font = create_font(400 /*FW_NORMAL*/);
    let bold_font = create_font(700 /*FW_BOLD*/);
    if font.is_null() || bold_font.is_null() {
        unsafe {
            let _ = DeleteDC(hdc);
        }
        anyhow::bail!("не створено шрифт для друку (CreateFont)");
    }
    let mut line_h: c_long = 20;
    let (page_h, margin, avail_w) = unsafe {
        let _ = SelectObject(hdc, font);
        let ag: Vec<u16> = "Ag\0".encode_utf16().collect();
        let mut sz = Size32 { cx: 0, cy: 0 };
        GetTextExtentPoint32W(hdc, ag.as_ptr(), 2, &mut sz);
        if sz.cy > 4 {
            line_h = sz.cy;
        }
        let pw = GetDeviceCaps(hdc, HORZRES) as c_long;
        let ph = GetDeviceCaps(hdc, VERTRES) as c_long;
        // ~10 % of the page width (≈0.5 inch on A4), clamped to a sane range
        let m = (pw / 10).clamp(24, 150);
        (ph, m, pw - 2 * m)
    };

    // 3) start the document, walk the pages
    let label: Vec<u16> = {
        let mut v: Vec<u16> = doc_label.encode_utf16().collect();
        v.push(0);
        v
    };
    let result: anyhow::Result<()> = (|| {
        unsafe {
            let di = DocInfoW {
                cb_size: std::mem::size_of::<DocInfoW>() as u32,
                fw_type: 0,
                psz_doc_name: label.as_ptr(),
                psz_output: ptr::null(),
                h_instance: ptr::null_mut(),
                h_bitmap: ptr::null_mut(),
            };
            if StartDocW(hdc, &di) == 0 {
                let e = GetLastError();
                anyhow::bail!("StartDoc: принтер недоступний (помилка {e})");
            }
            let mut in_page = false;
            let mut y = margin;
            for sl in &styled_lines(text) {
                let wrapped = if sl.text.is_empty() {
                    vec![String::new()]
                } else {
                    wrap_line(hdc, font, bold_font, &sl.text, sl.bold, avail_w)
                };
                for wl in &wrapped {
                    if !in_page {
                        if StartPage(hdc) == 0 {
                            let e = GetLastError();
                            let _ = EndDocW(hdc);
                            anyhow::bail!("StartPage: помилка {e}");
                        }
                        in_page = true;
                        y = margin;
                    }
                    if y > page_h - margin {
                        let _ = EndPage(hdc);
                        if StartPage(hdc) == 0 {
                            let e = GetLastError();
                            let _ = EndDocW(hdc);
                            anyhow::bail!("StartPage: помилка {e}");
                        }
                        y = margin;
                    }
                    if !wl.is_empty() {
                        let _ = SelectObject(hdc, if sl.bold { bold_font } else { font });
                        let u16: Vec<u16> = wl.encode_utf16().collect();
                        let mut sz = Size32 { cx: 0, cy: 0 };
                        GetTextExtentPoint32W(hdc, u16.as_ptr(), u16.len() as c_int, &mut sz);
                        let x0 = if sl.centered {
                            (margin + (avail_w - sz.cx) / 2).max(0)
                        } else {
                            margin
                        };
                        TextOutW(hdc, x0, y, u16.as_ptr(), u16.len() as c_int);
                    }
                    y += line_h;
                }
            }
            if in_page {
                let _ = EndPage(hdc);
            }
            if EndDocW(hdc) == 0 {
                let e = GetLastError();
                anyhow::bail!("EndDoc: помилка {e}");
            }
            Ok(())
        }
    })();
    unsafe {
        let _ = DeleteObject(font);
        let _ = DeleteObject(bold_font);
        let _ = DeleteDC(hdc);
    }
    result.map(|()| PrintOutcome::Sent)
}
