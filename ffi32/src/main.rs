//! 32-bit FFI facade host for `lmsup32.dll` (roadmap v0.42.0 / shipped v0.41.0).
//!
//! `lmsup32.dll` is a 32-bit PE32 (machine 0x14C I386) that imports only
//! KERNEL32 / USER32 / ADVAPI32 (self-contained; it does NOT depend on the
//! 16-bit `lmsup.dll`). A 64-bit process cannot `LoadLibraryW` it, so this
//! i686 helper loads the DLL, resolves the four `LMaster_*` exports
//! (`LoadLibraryW` + `GetProcAddress` — the FFI facade), and drives them
//! over a line-based stdin/stdout protocol that the 64-bit
//! `zakonexport-cli` speaks.
//!
//! Protocol (the first stdin line is the DLL directory, then one command
//! per line, one response per line):
//!   EXPORTS            -> `EXPORTS OK 4/4` or `EXPORTS FAIL <reason>`
//!   BEGIN <dir>        -> `BEGIN OK handle=<hex>` / `BEGIN FAIL <reason>`
//!   LANG <str>         -> `LANG <u32>`
//!   TRAN <size> <mode> -> `TRAN <ret> <outlen>` + `HEX <space-hex>`
//!   END                -> `END <ret>`
//!   BYE                -> exit 0
//!
//! All FFI is plain `extern "system"` (cdecl on i686); no external crates,
//! so the i686 build stays light.

use std::ffi::c_void;
use std::io::{self, BufRead, Write};

// ---------------------------------------------------------------------------
// WinAPI (the minimum needed; declared manually to avoid a heavy windows
// crate on the i686 target).
// ---------------------------------------------------------------------------
type HMODULE = *mut c_void;
type FARPROC = usize;

extern "system" {
    fn LoadLibraryW(name: *const u16) -> HMODULE;
    fn FreeLibrary(h: HMODULE) -> i32;
    fn GetProcAddress(hmod: HMODULE, name: *const u8) -> FARPROC;
    fn SetCurrentDirectoryW(path: *const u16) -> i32;
    fn GetLastError() -> u32;
    fn GetCurrentDirectoryW(size: u32, buf: *mut u16) -> u32;
    fn CreateFileW(
        name: *const u16,
        access: u32,
        share: u32,
        sa: *const c_void,
        disp: u32,
        flags: u32,
        template: *const c_void,
    ) -> *mut c_void;
}

// The four LMaster_* exports (the C ABI of lmsup32.dll). Signatures are the
// decompiled shapes (lmsup32_all.c): Begin returns the handle block pointer
// (whose first word is the engine handle); Tran takes a pointer to the
// handle value and writes the result into outbuf; Lang classifies a string.
type LMasterBegin =
    unsafe extern "system" fn(dir: *mut u8, tranopen: *mut u8) -> HMODULE;
type LMasterEnd = unsafe extern "system" fn(handle: HMODULE) -> i32;
type LMasterTran =
    unsafe extern "system" fn(handle: *mut i32, out: *mut u8, size: i32, mode: i32) -> i32;
type LMasterLang = unsafe extern "system" fn(s: *mut u8) -> u32;

struct State {
    hmod: HMODULE,
    p_begin: usize,
    p_end: usize,
    p_tran: usize,
    p_lang: usize,
    exports: u32,
    handle: HMODULE, // the block Begin returned (first word == the engine handle)
    begun: bool,
}

fn wide(s: &str) -> Vec<u16> {
    let mut v: Vec<u16> = s.encode_utf16().collect();
    v.push(0);
    v
}

fn cstr(s: &str) -> Vec<u8> {
    let mut v = s.as_bytes().to_vec();
    v.push(0);
    v
}

fn hex_of(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn cwd() -> String {
    let mut buf = [0u16; 4096];
    let n = unsafe { GetCurrentDirectoryW(buf.len() as u32, buf.as_mut_ptr()) };
    String::from_utf16_lossy(&buf[..n.max(1) as usize])
}

/// Can a relative path (resolved against the current directory) be opened for
/// read? (The engine's "LmOpen" opens lm.a / windows.kod relative to CWD.)
fn rel_readable(name: &str) -> bool {
    let w = wide(name);
    const GENERIC_READ: u32 = 0x80000000;
    const FILE_SHARE_READ: u32 = 1;
    const OPEN_EXISTING: u32 = 3;
    let h = unsafe {
        CreateFileW(
            w.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null(),
        )
    };
    if h.is_null() || h as usize == 0xFFFFFFFF {
        return false;
    }
    close(h);
    true
}

extern "system" {
    fn CloseHandle(h: *mut c_void) -> i32;
}

fn close(h: *mut c_void) {
    unsafe {
        CloseHandle(h);
    }
}

impl State {
    fn new() -> Self {
        State {
            hmod: std::ptr::null_mut(),
            p_begin: 0,
            p_end: 0,
            p_tran: 0,
            p_lang: 0,
            exports: 0,
            handle: std::ptr::null_mut(),
            begun: false,
        }
    }

    fn load(&mut self, dll_path: &str) -> Result<(), String> {
        let w = wide(dll_path);
        unsafe {
            self.hmod = LoadLibraryW(w.as_ptr());
        }
        if self.hmod.is_null() {
            return Err(format!(
                "LoadLibraryW failed (err {})",
                unsafe { GetLastError() }
            ));
        }
        // resolve the four exports (the FFI facade self-check)
        for (name, slot) in [
            ("LMaster_Begin", &mut self.p_begin),
            ("LMaster_End", &mut self.p_end),
            ("LMaster_Lang", &mut self.p_lang),
            ("LMaster_Tran", &mut self.p_tran),
        ] {
            let a = cstr(name);
            let p = unsafe { GetProcAddress(self.hmod, a.as_ptr()) };
            *slot = p;
            if p != 0 {
                self.exports += 1;
            }
        }
        if self.exports < 4 {
            return Err(format!("only {}/4 LMaster_* exports resolved", self.exports));
        }
        Ok(())
    }

    fn begin(&mut self, engine_dir: &str, dll_dir: &str) -> Result<String, String> {
        // The engine opens "windows.kod" (and the lm*.index files) relative to
        // the current directory, so CWD must be the SOFT dir (where the DLL +
        // windows.kod live). LMaster_Begin's param_1 is the base ROOT dir
        // (the "Lm" document database, e.g. c:\zakon.2\baza), which the engine
        // uses for the .mb data + the lm%s.uvt update stream ('W').
        let w = wide(dll_dir);
        let _ = unsafe { SetCurrentDirectoryW(w.as_ptr()) };
        let mut dc = cstr(engine_dir);
        let b = unsafe {
            let f = std::mem::transmute::<usize, LMasterBegin>(self.p_begin);
            f(dc.as_mut_ptr(), dc.as_mut_ptr())
        };
        if b.is_null() {
            return Err("LMaster_Begin returned null (engine init failed)".into());
        }
        self.handle = b;
        self.begun = true;
        Ok(format!("BEGIN OK handle={:x}", b as usize))
    }

    fn lang(&mut self, s: &str) -> String {
        if self.p_lang == 0 {
            return "LANG FAIL no-export".into();
        }
        let mut sc = cstr(s);
        let v = unsafe {
            let f = std::mem::transmute::<usize, LMasterLang>(self.p_lang);
            f(sc.as_mut_ptr())
        };
        format!("LANG {v}")
    }

    fn tran(&mut self, size: i32, mode: i32) -> Vec<String> {
        if self.p_tran == 0 || !self.begun {
            return vec!["TRAN FAIL not-begun".into()];
        }
        let n = size.max(64) as usize;
        let mut buf = vec![0u8; n];
        let t = unsafe {
            let f = std::mem::transmute::<usize, LMasterTran>(self.p_tran);
            let hptr = self.handle as *mut i32;
            f(hptr, buf.as_mut_ptr(), size, mode)
        };
        // report the NUL-terminated text prefix length (the readable part)
        let outlen = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        let mut out = vec![format!("TRAN {t} {outlen}")];
        out.push(format!("HEX {}", hex_of(&buf)));
        out
    }

    fn end(&mut self) -> String {
        if !self.begun {
            return "END 0 (never begun)".into();
        }
        let r = unsafe {
            let f = std::mem::transmute::<usize, LMasterEnd>(self.p_end);
            f(self.handle)
        };
        self.begun = false;
        format!("END {r}")
    }
}

fn main() {
    let mut st = State::new();
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let flush = || {
        let _ = io::stdout().flush();
    };

    // the first stdin line is the DLL directory (where lmsup32.dll lives);
    // the engine opens lm.a / windows.kod relative to CWD, so set CWD there
    let dir = match lines.next() {
        Some(Ok(d)) => d.trim().to_string(),
        _ => return,
    };
    {
        let w = wide(&dir);
        let _ = unsafe { SetCurrentDirectoryW(w.as_ptr()) };
    }

    loop {
        let line = match lines.next() {
            Some(Ok(l)) => l,
            _ => break,
        };
        let mut parts = line.splitn(2, ' ');
        let cmd = parts.next().unwrap_or("").to_uppercase();
        let arg = parts.next().unwrap_or("").to_string();

        match cmd.as_str() {
            "EXPORTS" => {
                // resolve the exports (idempotent if already loaded)
                if st.hmod.is_null() {
                    let dll = format!("{dir}\\lmsup32.dll");
                    match st.load(&dll) {
                        Ok(()) => println!("EXPORTS OK {}/4", st.exports),
                        Err(e) => {
                            println!("EXPORTS FAIL {e}");
                            flush();
                            break;
                        }
                    }
                } else {
                    println!("EXPORTS OK {}/4", st.exports);
                }
            }
            "BEGIN" => match st.begin(&arg, &dir) {
                Ok(m) => println!("{m}"),
                Err(e) => println!("BEGIN FAIL {e}"),
            },
            "LANG" => {
                let r = st.lang(&arg);
                println!("{r}");
            }
            "TRAN" => {
                let mut p = arg.split_whitespace();
                let size: i32 = p.next().and_then(|s| s.parse().ok()).unwrap_or(1024);
                let mode: i32 = p.next().and_then(|s| s.parse().ok()).unwrap_or(1);
                for l in st.tran(size, mode) {
                    println!("{l}");
                }
            }
            "END" => {
                let r = st.end();
                println!("{r}");
            }
            "DIAG" => {
                for f in ["lm.a", "windows.kod", "lmsup32.dll", "lmu.voc", "lmur.vt"] {
                    println!("DIAG {} {} (cwd={})", f, rel_readable(f), cwd());
                }
            }
            "BYE" => break,
            "" => {}
            other => {
                println!("UNKNOWN {other}");
            }
        }
        flush();
    }

    if !st.hmod.is_null() {
        unsafe {
            FreeLibrary(st.hmod);
        }
    }
}
