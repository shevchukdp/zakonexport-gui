//! Native folder selection via the Common Item Dialog
//! (IFileDialog + FOS_PICKFOLDERS). native-windows-gui's `nwg::init()` has
//! already called CoInitialize (STA) on the UI thread — this module must be
//! called only from a GUI event handler there, so no COM init bookkeeping of
//! its own. Object lifetime: dialog / shell items are always Released on
//! every path (single-inheritance vtables let us cast to IUnknown*).

use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use winapi::ctypes::c_void;
use winapi::shared::windef::HWND;
use winapi::shared::winerror::S_OK;
use winapi::shared::wtypesbase::CLSCTX_INPROC_SERVER;
use winapi::um::combaseapi::{CoCreateInstance, CoTaskMemFree};
use winapi::um::shobjidl::{
    FOS_FORCEFILESYSTEM, FOS_NOCHANGEDIR, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, IFileDialog,
};
use winapi::um::shobjidl_core::{
    CLSID_FileOpenDialog, IShellItem, SHCreateItemFromParsingName, SIGDN_FILESYSPATH,
};
use winapi::Interface;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn wide_path(p: &Path) -> Vec<u16> {
    p.as_os_str().encode_wide().chain(Some(0)).collect()
}

/// Release a COM object through the flat vtable prefix (IUnknown).
unsafe fn release(ptr: *mut c_void) {
    if !ptr.is_null() {
        let unk = ptr as *mut winapi::um::unknwnbase::IUnknown;
        (*unk).Release();
    }
}

/// Show the standard «choose folder» dialog. `start` hints the initial
/// directory (best effort). `None` on cancel / failure — callers fall back to
/// the path typed in the text box.
pub fn pick_folder(parent: HWND, start: Option<&Path>) -> Option<PathBuf> {
    unsafe {
        let mut dialog: *mut IFileDialog = std::ptr::null_mut();
        let hr = CoCreateInstance(
            &CLSID_FileOpenDialog,
            std::ptr::null_mut(),
            CLSCTX_INPROC_SERVER,
            &IFileDialog::uuidof(),
            &mut dialog as *mut *mut IFileDialog as *mut *mut c_void,
        );
        if hr != S_OK || dialog.is_null() {
            return None;
        }
        // release the dialog on every exit path
        let finish = |dlg: *mut IFileDialog, res: Option<PathBuf>| {
            release(dlg as *mut c_void);
            res
        };

        let dlg: &IFileDialog = &*dialog;
        if dlg.SetOptions(FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_NOCHANGEDIR | FOS_PATHMUSTEXIST)
            != S_OK
        {
            return finish(dialog, None);
        }

        // best-effort start directory
        if let Some(p) = start {
            let wstart = wide_path(p);
            let mut item: *mut IShellItem = std::ptr::null_mut();
            let hr = SHCreateItemFromParsingName(
                wstart.as_ptr(),
                std::ptr::null_mut(),
                &IShellItem::uuidof(),
                &mut item as *mut *mut IShellItem as *mut *mut c_void,
            );
            if hr == S_OK && !item.is_null() {
                let _ = dlg.SetFolder(item);
                release(item as *mut c_void);
            }
        }

        let title = wide("Оберіть папку для збереження експорту");
        let _ = dlg.SetTitle(title.as_ptr());

        // blocks on the modal dialog; S_OK only when the user confirmed
        if dlg.Show(parent) != S_OK {
            return finish(dialog, None);
        }

        let mut item: *mut IShellItem = std::ptr::null_mut();
        if dlg.GetResult(&mut item) != S_OK || item.is_null() {
            return finish(dialog, None);
        }
        let item_ref: &IShellItem = &*item;

        let mut pwsz: *mut u16 = std::ptr::null_mut();
        if item_ref.GetDisplayName(SIGDN_FILESYSPATH, &mut pwsz) != S_OK || pwsz.is_null() {
            release(item as *mut c_void);
            return finish(dialog, None);
        }

        let mut len = 0usize;
        while *pwsz.add(len) != 0 {
            len += 1;
        }
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(pwsz, len));
        CoTaskMemFree(pwsz as *mut c_void);
        release(item as *mut c_void);
        finish(dialog, if s.is_empty() { None } else { Some(PathBuf::from(s)) })
    }
}
