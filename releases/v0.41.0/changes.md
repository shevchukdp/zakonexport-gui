# v0.41.0 - Changes / Зміни (2026-10-30)

## UK / Українська

- **Rust `LMaster_*` driver (roadmap v0.42.0): FFI-facade для
  `lmsup32.dll`.** `lmsup32.dll` — 32-бітний PE32 (machine 0x14c I386;
  imports лише KERNEL32(73)/USER32(3)/ADVAPI32(6) — самодостатній, НЕ
  залежить від 16-бітного `lmsup.dll` (NE 0x446)). 64-бітний процес не
  може його `LoadLibraryW` (ERROR_BAD_FORMAT / err 126) → рушій працює
  через **32-бітний helper** `ffi32/lmsup32-ffi32.exe` (i686,
  commited як 4-й release-асет `lmsup32-ffi32-0.41.0.exe`):
  `LoadLibraryW` + `GetProcAddress` (FFI-facade) + лінійний
  stdin/stdout-протокол (перший рядок = папка DLL;
  `EXPORTS`/`BEGIN <enginedir>`/`LANG <str>`/`TRAN <size> <mode>`/
  `END`/`DIAG`/`BYE`).
- **Новий CLI sub-verb `trident lmaster [--corpus <file>] [--ffi32 <path>]`.**
  - **Facade (досвідчено)**: `EXPORTS OK 4/4` (`LMaster_{Begin,End,
    Lang,Tran}`); `LMaster_Lang` drivable (`"ne"` → 69, `"zakon"` → 69 —
    морфо-класифікатор).
  - **`LMaster_Begin`/`LMaster_Tran` (DLL-join) — blocked, задокументовано**:
    `BEGIN FAIL LMaster_Begin returned null` → `lm.log` рушія =
    `LmOpen Error`. Корінь: `FUN_10010b30` відкриває статичну глобальну
    `DAT_1002a30c` = `"lm.a"` (off=172812, imageBase 0x10000000,
    перевірлено по PE data-директоріям); цю глобальну виставляє лише
    **не-експортована** init legacy-застосунку (єдине xref — читання);
    4 експортів Lm-шлях БД не налаштовують. CWD=SOFT коректний,
    `lm.a`/`windows.kod`/`lmu.voc`/`lmur.vt` читаються (helper `DIAG`
    = true) → це внутрішній file/DB-шар рушія, а не проблема шляхів.
  - **Ground-truth join (Rust-side cross-check, шипить як референс
    цієї версії)**: `corpus_crosscheck()` через
    `crate::trident::TridentIndex` (власний рушій-зчитувач) на 100-
    запитному корпусі `corpus/trident_oracle_100.txt` (ukr):
    **100 запитів · 13 з joined docs · 8176 distinct native u16 docs**.
    `LMaster_Tran` (коли Lm-шлях-глобальна unblocked) має збігатися з
    цим референсом.
- **Rust surface** (`src/lmaster.rs`): type-aliases
  `LMaster{Begin,End,Tran,Lang}` (C-ABI, decompiled shape з
  `lmsup32_all.c` lines 16002/16140/16154/16389); `LmLine` +
  `parse_lm_line` (чистий parser ліній протоколу, total — без panic);
  `LmasterReport` + `parse_report` (fold сесії); `LmasterStatus`
  (`NoHelper{tried}`/`Ok`/`SpawnError`); `LmasterDriver::locate`
  (`--ffi32` → env `ZAKON_FFI32` → біля exe → cwd →
  `ffi32/target/i686-pc-windows-gnu/release/` in-repo; zero-panic) +
  `run` (spawn + protocol + EOF на stdin); `CorpusJoin`/
  `corpus_crosscheck`/`corpus_summary`.
- **i686 toolchain**: rustup не бандлить i686 self-contained linker →
  self-contained winlibs mingw (gcc 10.5.0) у
  `C:\temp\opencode\mingwi686ok`; збірка:
  `CC=…\i686-w64-mingw32-gcc.exe CARGO_TARGET_I686_PC_WINDOWS_GNU_LINKER=…
  cargo build --release --target i686-pc-windows-gnu`. Helper = i686 PE
  (0x14c), ~983 KB, 0 зовнішніх dependency.
- **210 юніт-тестів** (нові 8: `parse_exports_ok`, `parse_exports_fail`,
  `parse_begin_ok_and_fail`, `parse_lang_tran_end`, `parse_raw_and_bye`,
  `parse_report_folds_the_full_observed_session`,
  `locate_reports_no_helper_with_a_trail`, `corpus_summary_counts`).
  Guitest baseline `rows=430` без змін. Версія 0.40.0 → 0.41.0.
- Далі (v0.42.0 / roadmap v0.43.0): GUI — native-id export button
  (Trident panel → шлях `--trident-docs`).

## EN / English

- **Rust `LMaster_*` driver (roadmap v0.42.0): the FFI facade over
  `lmsup32.dll`.** `lmsup32.dll` is a 32-bit PE32 (machine 0x14c I386;
  it imports only KERNEL32(73)/USER32(3)/ADVAPI32(6) — self-contained,
  it does NOT depend on the 16-bit `lmsup.dll` (NE 0x446)). A 64-bit
  process cannot `LoadLibraryW` it (ERROR_BAD_FORMAT / err 126), so the
  driver runs through the **32-bit helper**
  `ffi32/lmsup32-ffi32.exe` (i686, committed as the 4th release asset
  `lmsup32-ffi32-0.41.0.exe`): `LoadLibraryW` + `GetProcAddress` (the
  FFI facade) + a line-based stdin/stdout protocol (first line = the DLL
  dir; `EXPORTS`/`BEGIN <enginedir>`/`LANG <str>`/`TRAN <size> <mode>`/
  `END`/`DIAG`/`BYE`).
- **New CLI sub-verb `trident lmaster [--corpus <file>] [--ffi32 <path>]`.**
  - **Facade (observed)**: `EXPORTS OK 4/4`
    (`LMaster_{Begin,End,Lang,Tran}`); `LMaster_Lang` is drivable
    (`"ne"` → 69, `"zakon"` → 69 — the morphology classifier).
  - **`LMaster_Begin`/`LMaster_Tran` (the DLL join) — blocked,
    documented**: `BEGIN FAIL LMaster_Begin returned null` → the
    engine's `lm.log` = `LmOpen Error`. Root: `FUN_10010b30` opens the
    static global `DAT_1002a30c` = `"lm.a"` (off=172812, imageBase
    0x10000000, verified against the PE data directories); that global
    is set only by the **non-exported** init of the legacy app (a single
    xref — a read); the four exports do not configure the Lm database
    path. CWD=SOFT is correct and `lm.a`/`windows.kod`/`lmu.voc`/
    `lmur.vt` are readable (helper `DIAG` = true) → it is the engine's
    internal file/DB layer, not a path problem.
  - **Ground-truth join (the Rust-side cross-check, shipped as this
    version's reference)**: `corpus_crosscheck()` through
    `crate::trident::TridentIndex` (the exporter's own reader) over the
    100-query corpus `corpus/trident_oracle_100.txt` (ukr): **100
    queries · 13 with joined docs · 8176 distinct native u16 docs**.
    `LMaster_Tran` (once the Lm path global is unblocked) must agree
    with this reference.
- **Rust surface** (`src/lmaster.rs`): the `LMaster{Begin,End,Tran,
  Lang}` type aliases (the C ABI, the decompiled shapes from
  `lmsup32_all.c` lines 16002/16140/16154/16389); `LmLine` +
  `parse_lm_line` (the pure protocol-line parser, total — no panics);
  `LmasterReport` + `parse_report` (the session fold); `LmasterStatus`
  (`NoHelper{tried}`/`Ok`/`SpawnError`); `LmasterDriver::locate`
  (`--ffi32` → the `ZAKON_FFI32` env → next to the exe → cwd → the
  in-repo `ffi32/target/i686-pc-windows-gnu/release/`; zero-panic) +
  `run` (spawn + protocol + EOF on stdin); `CorpusJoin`/
  `corpus_crosscheck`/`corpus_summary`.
- **i686 toolchain**: rustup does not bundle the i686 self-contained
  linker → a self-contained winlibs mingw (gcc 10.5.0) at
  `C:\temp\opencode\mingwi686ok`; build:
  `CC=…\i686-w64-mingw32-gcc.exe CARGO_TARGET_I686_PC_WINDOWS_GNU_
  LINKER=… cargo build --release --target i686-pc-windows-gnu`. The
  helper is an i686 PE (0x14c), ~983 KB, zero external dependencies.
- **210 unit tests** (8 new: `parse_exports_ok`, `parse_exports_fail`,
  `parse_begin_ok_and_fail`, `parse_lang_tran_end`, `parse_raw_and_bye`,
  `parse_report_folds_the_full_observed_session`,
  `locate_reports_no_helper_with_a_trail`, `corpus_summary_counts`).
  The guitest baseline `rows=430` is unchanged. Version 0.40.0 → 0.41.0.
- Next (v0.42.0 / roadmap v0.43.0): the GUI — the native-id export
  button (the Trident panel → the `--trident-docs` path).
