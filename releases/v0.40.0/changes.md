# v0.40.0 - Changes / Зміни (2026-10-29)

## UK / Українська

- **Поведення `.uvt` (roadmap v0.41.0): write-mode per-language update
  cache.** Факти з `lmsup32.dll` (`FUN_10015340`, задокументовано в
  v0.20.0/v0.21.0): рушій відкриває `lm{lang}.m` ('R') + `lm{lang}.uvt`
  ('W') + `lm{lang}.vt` ('R') — `.uvt` це inkrementальний update-стрім
  для `.vt`-postings, відкривається лише на запис. У цій read-only базі
  файл відсутній (SOFT\ має лише 204-байтові `lmu.use`/`lmr.use`; рушій
  створив би `.uvt` на вимагу, але ніколи не писав).
- **Новий CLI sub-verb `trident uvt [--synth [dir]]`.**
  - Статус: per-language стан `.uvt` (lmur.uvt ukr / lmru.uvt rus) —
    ABSENT/PRESENT + розмір + parse (204B FREE-container form tail=0 →
    0 records; або .vt-grammar stream → кількість record-ів).
  - **--synth [dir]** — синтез відсутніх `.uvt` НА КОПІЇ БАЗИ (16-
    файловий inventory → `<dir>\SOFT\`; dir за замовчуванням
    C:\temp\opencode\uvt_synth\<pid>), ніколи не торкаючись read-only
    бази:
    - синтез у pinned 204B FREE-container form (64B header: vendor з
      реального .vt, name → lmUR.uvt/lmRU.uvt, FREE, tail=0 + 140B
      нульовий body — форма FUN_10011e10 з v0.21.0); точний
      початковий розмір .uvt не зафіксовано дизасемблеєм → синтезовано
      єдину pinned empty-cache form;
    - verify: parse (0 update records) + TridentIndex::open на копії +
      search parity: tsearch «не» (ukr) оригінал == копія
      (967 u16 == 967 u16, ідентично — update-cache прозора для
      експортера).
- **Rust surface** (trident_vt.rs): UVT_LANGS template (uvt-ім'я, lang,
  vt-ім'я, header-name) + UvtState/uvt_states(soft) (per-language стан,
  обидві pinned форми) + synth_uvt_from_vt(vt, name) (чистий
  синтезатор 204B container; None без FREE header).
- **202 юніт-тести** (нові 3: synth_uvt_from_vt_produces_the_204b_container,
  uvt_states_reports_the_real_base_and_a_synthesized_copy,
  uvt_synth_base_copy_search_parity). Guitest baseline rows=430 без змін.
  Версія 0.39.0 → 0.40.0.
- Далі (v0.41.0 / roadmap v0.42.0): Rust LMaster_* driver (FFI-facade
  для lmsup32.dll; крос-чек DLL-join vs Rust-join на 100-запитному
  корпусі).

## EN / English

- **`.uvt` behavior (roadmap v0.41.0): the write-mode per-language
  update cache.** Facts from `lmsup32.dll` (`FUN_10015340`, documented
  in v0.20.0/v0.21.0): the engine opens `lm{lang}.m` ('R') +
  `lm{lang}.uvt` ('W') + `lm{lang}.vt` ('R') — the `.uvt` is the
  incremental update stream for the `.vt` postings, opened for writing
  only. In this read-only base the file is absent (SOFT\ carries only
  the 204B `lmu.use`/`lmr.use`; the engine would create the `.uvt` on
  demand but never wrote it).
- **New CLI sub-verb `trident uvt [--synth [dir]]`.**
  - Status: the per-language `.uvt` state (lmur.uvt ukr / lmru.uvt rus)
    — ABSENT/PRESENT + size + parse (the 204B FREE-container form
    tail=0 → 0 records; or a .vt-grammar stream → the record count).
  - **--synth [dir]** — synthesizes the absent `.uvt` caches ON A BASE
    COPY (the 16-file inventory → `<dir>\SOFT\`; dir defaults to
    C:\temp\opencode\uvt_synth\<pid>), never touching the read-only
    base:
    - the synthesis uses the pinned 204B FREE-container form (64B
      header: the vendor from the real .vt, the name → lmUR.uvt/
      lmRU.uvt, FREE, tail=0 + the 140B zero body — the FUN_10011e10
      shape from v0.21.0); the exact initial .uvt size is not pinned
      by the disassembly → the single pinned empty-cache form is
      synthesized;
    - verify: parse (0 update records) + TridentIndex::open on the copy
      + search parity: tsearch «не» (ukr) original == copy (967 u16 ==
      967 u16, identical — the update cache is transparent to the
      exporter).
- **Rust surface** (trident_vt.rs): the UVT_LANGS template (uvt-name,
  lang, vt-name, header-name) + UvtState/uvt_states(soft) (the
  per-language state, both pinned forms) + synth_uvt_from_vt(vt, name)
  (the pure 204B-container synthesizer; None without the FREE header).
- **202 unit tests** (3 new: synth_uvt_from_vt_produces_the_204b_container,
  uvt_states_reports_the_real_base_and_a_synthesized_copy,
  uvt_synth_base_copy_search_parity). The guitest baseline rows=430 is
  unchanged. Version 0.39.0 → 0.40.0.
- Next (v0.41.0 / roadmap v0.42.0): the Rust LMaster_* driver (an FFI
  facade over lmsup32.dll; a DLL-join vs Rust-join cross-check on the
  100-query corpus).
