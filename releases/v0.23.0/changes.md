# v0.23.0 - Changes / Зміни (2026-10-28)

## UK / Українська

- **Doc-number join: native u16 ↔ поточні docnums** (нова
  `src/trident_join.rs`: `DocJoin` / `JoinKind`). Модуль пробує
  у16-простір подингів `.vt` (dense-потік `TridentIndex.vt_u16`)
  проти поточного doc-лічильника бази (`render::Base::doc_count()`,
  `docloc.db`) і класифікує:
  - **Identity** — max u16 ≤ doc_count: u16-ідентифікатори
    кандидат-поточні docnums (таблиця-мапінг не потрібна);
  - **Bounded** — простір 2005 ширший; openable = u16 ≤ docnum;
  - **Unknown** — поточну базу не завантажено.
- **Емпірика цієї бази**: у16-простір `.vt` сягає 65535 (~46k
  distinct, чисті per-record u16 від 257), а поточна docloc-база =
  **613173 док-ів** (`docloc.db` 3065877 B → (len−12)/5; контейнер
  `doc\doc.mb`). Простір 2005 цілком вкладається в поточний
  docnum-діапазон → проба класифікує **Identity**. (Легасі
  `text\*.mb` = 2363 файлів — це вхід для ftsearch-оракула, не
  docloc-база; раніше їх плутали як doc-лічильник.)
- **CLI `tsearch`** — новий рядок `join native→docnum (v0.23.0)`:
  у16-простір + distinct/total vs поточна база + kind; per-record
  **openable-субмножина** (≤ docnum) для топ-4 join-кандидатів;
  фінальний рядок «результат» тепер динамічний (показує actual kind,
  а не hard-coded Bounded).
- **142 юніт-тестів** (нові 5: identity/bounded/unknown-класи,
  openable-субмножина, реальний lmur.vt-потік → Identity). Guitest
  baseline `rows=430` без змін. Версія **0.22.0 → 0.23.0**.
- Наступний RE-крок (v0.24.0): oracle-кросчек tsearch vs ftsearch
  (чи u16 docnums = docloc docnums семантично), потім rus
  узагальнення `tsearch --lang`.

## EN / English

- **Doc-number join: native u16 ↔ current docnums** (new
  `src/trident_join.rs`: `DocJoin` / `JoinKind`). The module probes
  the `.vt` u16 posting space (the dense `TridentIndex.vt_u16`
  stream) against the current base doc count
  (`render::Base::doc_count()`, `docloc.db`) and classifies:
  - **Identity** — max u16 ≤ doc_count: the u16 ids are candidate
    current docnums (no mapping table needed);
  - **Bounded** — the 2005 space is wider; openable = u16 ≤ docnum;
  - **Unknown** — the current base did not load.
- **Empirical (this base)**: the u16 space of `.vt` reaches 65535
  (~46k distinct; clean per-record u16s from 257), while the current
  docloc base = **613173 docs** (`docloc.db` 3065877 B → (len−12)/5;
  `doc\doc.mb` container). The 2005 space fits entirely inside the
  current docnum range → the probe classifies **Identity**. (The
  legacy `text\*.mb` = 2363 files is the ftsearch-oracle input, not
  the docloc base; it was previously mistaken for the doc count.)
- **CLI `tsearch`** — a new `join native→docnum (v0.23.0)` line:
  the u16 space + distinct/total vs the current base + the kind; a
  per-record **openable subset** (≤ docnum) for the top-4 join
  candidates; the final «результат» line is now dynamic (shows the
  actual kind, not a hard-coded Bounded).
- **142 unit tests** (5 new: the identity/bounded/unknown classes,
  the openable subset, and the real lmur.vt stream → Identity). The
  guitest baseline `rows=430` is unchanged. Version **0.22.0 →
  0.23.0**.
- Next RE step (v0.24.0): the tsearch vs ftsearch oracle
  cross-check (are the u16 docnums the docloc docnums, semantically),
  then the rus generalization `tsearch --lang`.
