# v0.37.0 - Changes / Зміни (2026-10-29)

## UK / Українська

- **`export --trident-docs` (roadmap v0.38.0).** `export` тепер бере прапорець
  `--trident-docs` [запит]: замість звичайного повнотекстового пошуку запускає
  **нативний tsearch stem-join** і експортує **розв'язані docnums** як звичайний
  експорт-бандл, з кожною документацією несучи **native-id evidence column**:
  - **`cli.rs::trident_docs_export`** — `TridentIndex::open(&baza)` →
    `search_lang`/`search_auto` (за `--lang u|r|auto`) → `r.joined`
    (кандидат → `.vt` record @off → native u16) → розв'язує u16 у поточні
    docnums (openable: `0 < u16 ≤ doc_count`, Identity/Bounded join, v0.23.0) →
    `Vec<db::Hit>` через `render::Base::span` → `writers::export_hits` з новою
    evidence-колонкою.
  - **`trident.rs::trident_docs_from_result(r, doc_count)`** — чистий resolver:
    `docnum -> Vec<(candidate, record_off)>` (openable u16, де-дуплікований);
    покладений в основу CLI + тестів.
  - **`writers.rs`** — `export_hits(..., evidence: Option<&HashMap<u32, String>>)
    + head_with_evidence`: кожна документ отримує рядок
    `нативні u16 (Trident): <cand>@0x…, <cand>@0x…` в head-блоці.
    GUI/`bulk`/звичайний `export` передають `None` (колонка не з'являється).
- **Sample run (DoD).** `export "не" --trident-docs --format md --single
  --out … --baza C:\Zakon.Local\Baza`: `--trident-docs: join 32 record-ів →
  openable u16 2372 → 967 docnum (0 поза межами ≤613173)` →
  `зв'язка (Trident): 967 документи -> 1 файл: bundle-967-18601.md`. Кожні 967
  документов несуть evidence-рядок (напр. `нативні u16 (Trident): NE@0x148bf`)
  — docnums збігаються з виводом `tsearch "не"` для того самого запиту.
- **192 юніт-тестів** (нові 2: `trident_docs_from_result_drops_out_of_range_u16`
  (synthetic) + `trident_docs_from_result_resolves_openable_docnums` (real
  base)). Guitest baseline `rows=430` без змін. Версія **0.36.0 → 0.37.0**.
- Далі (v0.38.0 / roadmap v0.39.0): `tsearch --fetch` (пошук → текст
  документа в одному слові).

## EN / English

- **`export --trident-docs` (roadmap v0.38.0).** `export` now takes a
  `--trident-docs` [query] flag: instead of the built-in full-text search it
  runs the **native tsearch stem-join** and exports the **resolved docnums** as
  a normal export bundle, each document carrying a **native-id evidence
  column**:
  - **`cli.rs::trident_docs_export`** — `TridentIndex::open(&baza)` →
    `search_lang`/`search_auto` (per `--lang u|r|auto`) → `r.joined`
    (candidate → `.vt` record @off → native u16) → resolves u16 into
    current-base docnums (openable: `0 < u16 ≤ doc_count`, the Identity/Bounded
    join, v0.23.0) → `Vec<db::Hit>` via `render::Base::span` →
    `writers::export_hits` with the new evidence column.
  - **`trident.rs::trident_docs_from_result(r, doc_count)`** — a pure resolver:
    `docnum -> Vec<(candidate, record_off)>` (the openable u16, de-duplicated);
    the basis of both the CLI and the tests.
  - **`writers.rs`** — `export_hits(..., evidence: Option<&HashMap<u32, String>>)
    + head_with_evidence`: each document gains a
    `нативні u16 (Trident): <cand>@0x…, <cand>@0x…` line in the head block.
    GUI / `bulk` / the regular `export` pass `None` (the column stays absent).
- **Sample run (DoD).** `export "не" --trident-docs --format md --single
  --out … --baza C:\Zakon.Local\Baza`: `--trident-docs: join 32 records →
  2372 openable u16 → 967 docnums (0 out of range ≤ 613173)` → `зв'язка
  (Trident): 967 documents -> 1 file: bundle-967-18601.md`. All 967 docs carry
  their evidence line (e.g. `нативні u16 (Trident): NE@0x148bf`) — the docnums
  match the `tsearch "не"` output for the same query (the same `.vt` records,
  the same openable u16 subset).
- **192 unit tests** (2 new: `trident_docs_from_result_drops_out_of_range_u16`
  (synthetic) + `trident_docs_from_result_resolves_openable_docnums` (real
  base)). The guitest baseline `rows=430` is unchanged. Version **0.36.0 →
  0.37.0**.
- Next (v0.38.0 / roadmap v0.39.0): `tsearch --fetch` (search → document text
  in one verb).
