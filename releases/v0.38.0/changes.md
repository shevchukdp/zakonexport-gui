# v0.38.0 - Changes / Зміни (2026-10-29)

## UK / Українська

- **`tsearch --fetch [N]` / `tsearch --print` (roadmap v0.39.0).** `tsearch`
  тепер замикає ланцюг «пошук → docnum → документ» в одному слові:
  - **`--fetch N`** (top-N, за замовчуванням 10) — розв'язує openable docnums
    (через `trident_docs_from_result`, v0.37.0) і для кожного друкують
    **title + перший абзац** через `render::Base::doc_text` + `fields::parse`.
  - **`--print`** — GDI printdoc top-N joined docnums (системний діалог
    друку, `crate::printdoc::print_text`), формат як у слові `print` (v0.31.0).
  - Нові хелпери `first_nonempty_line` (title fallback) + `first_body_line`
    (перший абзац, обмежено 160 зн.). Прапорці беруть docnums з `r.joined` →
    openable subset (`0 < u16 ≤ doc_count`).
- **10-query manual check (DoD).** `tsearch --fetch 2` на реальній базі
  (613173 docnums): 6/10 stem/префікси (не/пере/по/зак/розп/про) розв'язуються
  в коректно читабельні юридичні titli (напр. [258] «Про умови оплати праці
  працівників», [257] «Про представників Кабінету Міністрів України»,
  [468] «НАЦІОНАЛЬНИЙ БАНК УКРАЇНИ»); 4/10 (спів/заст/розк/бук) —
  задокументований packed-B-tree gap (повне word-set, «конституція» — далі).
- **194 юніт-тестів** (нові 2: `first_nonempty_line_skips_blank_leading_lines`
  + `first_body_line_is_the_second_nonempty_line_capped`). Guitest baseline
  `rows=430` без змін. Версія **0.37.0 → 0.38.0**.
- Далі (v0.39.0 / roadmap v0.40.0): hardening + production marking.

## EN / English

- **`tsearch --fetch [N]` / `tsearch --print` (roadmap v0.39.0).** `tsearch`
  now closes the "search → docnum → document" chain in one verb:
  - **`--fetch N`** (top-N, default 10) — resolves the openable docnums (via
    `trident_docs_from_result`, v0.37.0) and, for each, prints a **title +
    first paragraph** via `render::Base::doc_text` + `fields::parse`.
  - **`--print`** — a GDI printdoc of the top-N joined docnums (the system
    print dialog, `crate::printdoc::print_text`), formatted like the `print`
    verb (v0.31.0).
  - New helpers `first_nonempty_line` (title fallback) + `first_body_line`
    (first paragraph, capped to 160 chars). The flags take docnums from
    `r.joined` → the openable subset (`0 < u16 ≤ doc_count`).
- **10-query manual check (DoD).** `tsearch --fetch 2` on the real base
  (613173 docnums): 6/10 stems/prefixes (не/пере/по/зак/розп/про) resolve to
  correctly-reading legal titles (e.g. [258] «Про умови оплати праці
  працівників», [257] «Про представників Кабінету Міністрів України»,
  [468] «НАЦІОНАЛЬНИЙ БАНК УКРАЇНИ»); 4/10 (спів/заст/розк/бук) are the
  documented packed-B-tree gap (the full word set; «конституція» deferred).
- **194 unit tests** (2 new: `first_nonempty_line_skips_blank_leading_lines`
  + `first_body_line_is_the_second_nonempty_line_capped`). The guitest
  baseline `rows=430` is unchanged. Version **0.37.0 → 0.38.0**.
- Next (v0.39.0 / roadmap v0.40.0): hardening + production marking.
