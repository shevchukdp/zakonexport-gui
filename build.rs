//! Link settings. Rust's std already links kernel32/user32/gdi32;
//! comdlg32 (PrintDlgW for the GDI print path, src\printdoc.rs) and
//! winspool (StartDocW/EndDocW/StartPage/EndPage) are not in the default
//! link set, so add them here.
//!
//! EndDocW/GetDCW/ReleaseDCW are absent from the self-contained mingw
//! import libraries of the x86_64-pc-windows-gnu toolchain; printdoc.rs
//! resolves them via winapi (GetDC/ReleaseDC) and GetProcAddress
//! (EndDocW) at runtime instead.
fn main() {
    println!("cargo:rustc-link-lib=comdlg32");
    println!("cargo:rustc-link-lib=winspool");
    println!("cargo:rerun-if-changed=build.rs");
}
