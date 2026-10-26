# v0.12.0 - Changes / Зміни (2026-10-26)

## UK / Українська

- **Підсвітка слів запиту в експорті** — новий флаг CLI `--hl`
  (аліас `--highlight`) для `export` + чекбокс «підсвітити слова
  запиту» в GUI (поруч із «зв'язка»). Слова запиту (≥2 знаків,
  регістронезалежно) позначаються в експортованому тексті:
  md `<mark>…</mark>`; html `<mark class="hl">` + CSS
  `mark.hl{background:#ffe86b}`; rtf жовлий хайлайт
  (`\highlight6`/`\highlight0`); docx — окремі біги з
  `<w:highlight w:val="yellow"/>` (збереження вихідної форми,
  текст не ламається). pdf/csv — у наступних версіях. Ядро:
  `highlight::Hl` (terms → char-діапазони → партиціонування
  рядка; найдовше слово перетворює, збіги не перетинаються);
  порожній `Hl` = хайлайт вимкнено (гарячий шлях, без кошту).
- **Партійний друк** — `print <docnum> [docnum ...]`: кілька
  документів (пробіл/кома, діапазони `1-5` через той самий
  розбір, що й `--docs`) друкуються один за одним, системний
  діалог друку — на кожен документ, статус друкується окремим
  рядком («надіслано в принтер» / «відмінено» / помилка).
- **Команда `trident`** — інвентар нативних FTS-індексів
  старої бази: `SOFT\` (lmu.voc / lmr.voc словники, lmur.vt /
  lmru.vt postings, lm*.m метадані, lmu.g / lmr.g графи, lmu.gw /
  lmr.gw ваги, lmsup.dll / lmsup32.dll рушії — 12 файлів з
  розмірами). Без папки SOFT (наприклад, БАЗА GK) — коректне
  повідомлення, що індексів немає і пошук йде власним рушієм.
  Повний Trident-рушій («пошук як у головному застосунку») —
  у розробці (v0.13.0: словники = стеми, .voc — 7-байтові
  хвостові записи, .vt — delta/packed postings).
- Випущено функцію section-headings (0 зі 120 зразкових
  документів — ознак не знайдено; зовільнює експорт).
- Версія **0.11.0 → 0.12.0** (єдине джерело: `CARGO_PKG_VERSION`).
- 71 юніт-тест (нові: `highlight::tests` ×6, hl-тести writer'ів
  ×6: md/html/rtf/docx/bundle) + GUI-самотест `--guitest` (тепер
  звітує `marks=N` — кількість `<mark>` у експортованих файлах,
  і перевіряє чекбокс підсвітки).

## EN / English

- **Search-term highlighting in exports** — new CLI flag `--hl`
  (alias `--highlight`) for `export` + a "підсвітити слова запиту"
  checkbox in the GUI (next to the bundle checkbox). Query terms
  (≥2 chars, case-insensitive) are marked in the exported text:
  md `<mark>…</mark>`; html `<mark class="hl">` + CSS
  `mark.hl{background:#ffe86b}`; rtf yellow highlight
  (`\highlight6`/`\highlight0`); docx — separate runs with
  `<w:highlight w:val="yellow"/>` (the matched text stays inside
  its own run; layout preserved). pdf/csv — a later version.
  Core: `highlight::Hl` (terms → char ranges → string partition;
  longest match wins, ranges do not overlap); empty `Hl` =
  highlighting off (hot path, zero cost).
- **Batch print** — `print <docnum> [docnum ...]`: several
  documents (space/comma separated, `1-5` ranges via the same
  parser as `--docs`) print one after another; the system print
  dialog appears per document and the per-document outcome
  ("sent to printer" / "cancelled" / error) is printed.
- **`trident` command** — inventory of the legacy base's native
  FTS indexes: `SOFT\` (lmu.voc / lmr.voc dictionaries, lmur.vt /
  lmru.vt postings, lm*.m metadata, lmu.g / lmr.g graphs,
  lmu.gw / lmr.gw weights, lmsup.dll / lmsup32.dll engines — 12
  files with sizes). When the SOFT folder is absent (e.g. the
  GK base) it reports that gracefully: no indexes, search runs
  on the exporter's own engine. The full Trident engine ("search
  like the main app") is in development (v0.13.0 plan:
  dictionaries are stems, .voc — 7-byte tail records, .vt —
  delta/packed postings).
- The section-headings feature was dropped (0 of 120 sampled
  documents show any heading markers; it only slowed exports).
- Version **0.11.0 → 0.12.0** (single source: `CARGO_PKG_VERSION`).
- 71 unit tests (new: `highlight::tests` ×6, writer hl tests ×6:
  md/html/rtf/docx/bundle) + the GUI self-test `--guitest` now
  reports `marks=N` (count of `<mark>` in the exported files)
  and checks the highlight checkbox.
