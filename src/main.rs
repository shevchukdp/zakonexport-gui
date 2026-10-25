//! zakonexport-cli — console executable of the Zakon DB exporter.
//! The engine (search / render / fields / writers / GUI) lives in the lib
//! target; the whole CLI verb handling is in `cli`.

fn main() {
    zakon_exporter::cli::main_cli();
}
