# v0.42.0 - Changes / Зміни (2026-10-30)

## UK / Українська

- **GUI: native-id export button (roadmap v0.43.0): панель «Тріант»
  тепер експортує.** Новий кнопа **«Експорт (native-id)»** у
  Trident-панелі (окреме модальне вікно, v0.36.0) запускає **шлях
  `--trident-docs`** (v0.37.0) з GUI: запит + база з полів панелі →
  tsearch stem-join → openable native u16 docnums → **md-бандл**, де
  **кожен документ несе рядок native-id-довідки**
  (`нативні u16 (Trident): cand@0x…, cand@0x…` — join-кандидати +
  `.vt` record offsets, які дали цей docnum). Експорт пишеться у
  папку з naming.toml (per-run субпапка), формат md, auto-lang
  (ukr+rus).
- **Один pipeline, без дублювання** (`writers.rs`): спільна функція
  `trident_docs_bundle()` (join → openable docnums → бандл з
  evidence-колонкою) тепер виконує і CLI `export --trident-docs`
  (рефактор — поведінка збережено; live-перевірка на реальній базі:
  «не» → join 32 record-ів · openable u16 2372 → **967 docnum → 967
  файлів**, evidence-рядок у кожному), і GUI-кнопа.
  `TridentBundle` — лічильники (joined_records / openable_u16 /
  dropped_u16 / missing / exported / attached) + папка вивантаження.
- **GUI wiring** (`gui.rs`): кнопа `trident_export_btn` у панелі
  (pos (10,346), поруч із «Закрити»); **воркер-потік** (Base-лодіння +
  рендер не блокує UI) + **окремий `trident_rx`-канал** (воркери
  основного вікна замінюють `rx` — воркер панелі не втрачає
  повідомлення про завершення) + `trident_busy` (re-entrance-захист,
  «Експорт уже запущений…») + log-рядки в результатне поле панелі
  (`trident_text`-буфер unix-newlines + `set_text_unix2dos` на
  UI-кордоні). Панель — окреме вікно → baseline `rows=430` не
  чіпається.
- **211 юніт-тестів** (новий:
  `trident_docs_bundle_exports_the_native_evidence_line` — real-base
  test: «не» → 967 docnum, assert, що evidence-рядок у **кожному**
  вивантаженому документі; skip без бази). Guitest baseline `rows=430`
  без змін. Версія 0.41.0 → 0.42.0.
- Далі (v0.43.0 / roadmap v0.44.0): інструменти rebuild індексу
  per-base (запис `.vt`/`.m` з `.mb`-скану).

## EN / English

- **GUI: the native-id export button (roadmap v0.43.0): the «Тріант»
  panel now exports.** A new **«Експорт (native-id)»** button in the
  Trident panel (the separate modal window, v0.36.0) runs the
  **`--trident-docs` path** (v0.37.0) from the GUI: the panel's query +
  base → the tsearch stem-join → the openable native u16 docnums → an
  **md bundle** where **every document carries its native-id evidence
  line** (`нативні u16 (Trident): cand@0x…, cand@0x…` — the join
  candidates + the `.vt` record offsets that produced that docnum).
  The export is written into the naming.toml folder (the per-run
  subfolder), md format, auto-lang (ukr+rus).
- **One pipeline, no duplication** (`writers.rs`): the shared
  `trident_docs_bundle()` (join → openable docnums → the bundle with
  the evidence column) now serves both the CLI `export --trident-docs`
  (refactor — behavior preserved; live-verified on the real base:
  «не» → 32 joined records · 2372 openable u16 → **967 docnums → 967
  files**, the evidence line in every file) and the GUI button.
  `TridentBundle` — the counters (joined_records / openable_u16 /
  dropped_u16 / missing / exported / attached) + the output dir.
- **GUI wiring** (`gui.rs`): the `trident_export_btn` in the panel
  (pos (10,346), next to «Закрити»); a **worker thread** (the base load
  + render never block the UI) + a **dedicated `trident_rx` channel**
  (the main-window workers replace `rx` — the panel's worker must not
  lose its completion messages) + `trident_busy` (the re-entrance
  guard, «Експорт уже запущений…») + the log lines in the panel's
  result box (the `trident_text` unix-newline buffer +
  `set_text_unix2dos` at the UI boundary). The panel is a separate
  window → the `rows=430` baseline is untouched.
- **211 unit tests** (new:
  `trident_docs_bundle_exports_the_native_evidence_line` — the
  real-base test: «не» → 967 docnums, asserting the evidence line in
  **every** written document; skipped when the base is absent). The
  guitest baseline `rows=430` is unchanged. Version 0.41.0 → 0.42.0.
- Next (v0.43.0 / roadmap v0.44.0): the per-base index-rebuild
  tooling (write `.vt`/`.m` from the `.mb` scan).
