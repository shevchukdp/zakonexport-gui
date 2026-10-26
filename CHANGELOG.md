# CHANGELOG / Журнал змін / Change history

Правило для кожної нової версії (standing rule, UA + EN):
1. Запис у цьому файлі (UA + EN) — до публікації;
2. Нотатки сторінки GitHub-релізу — UA + EN;
3. Файл `changes.md` (UA + EN) **в папці версії** `releases/<v>/`;
4. Бинарники: `releases/<v>/` у репо + ассети GitHub-релізу
   (`.github/workflows/release-assets.yml` або прямий POST на `uploads.github.com`).

Rule for every new version (UA + EN):
1. An entry in this file (UA + EN) — before publishing;
2. GitHub release notes — UK + EN;
3. A `changes.md` file (UK + EN) **inside the version folder** `releases/<v>/`;
4. Binaries: `releases/<v>/` in-repo + GitHub release assets
   (`.github/workflows/release-assets.yml` or direct POST to `uploads.github.com`).

---

## v0.13.0 — 2026-10-26

### UK
- **Trident: експериментальний нативний рушій** (`src/trident.rs`) —
  декод частин нативного FTS-індексу старого застосунку в папці
  `SOFT\` бази:
  - `windows.kod` (683 Б): кодова таблиця, 80 рядків
    `<2hex-code> <sep> <byte>` — значення ≥0x80 записано сирийм
    байтом, <0x80 — двома ASCII-hex цифрами; сепаратори `=`
    (мапінг) і `>` (fold);
  - `lm.a` (904 Б): case-fold таблиці, текстові секції `E:`
    (Latin identity A…Z), `G:` (міжмовні якорі: A↔84/a4,
    O↔96/e6, U↔9c/ec, Щ↔9f/ef), `R:`/`U:` — по 32 fold-пари
    (в U головний діапазон пропускає 9a/9b/9d, додаткові пари
    f4f5/f6f7/f8f9);
  - записи словника `[0x84][word][00 00]` (lmu.voc — 78 записів)
    і u16 posting-потік `.vt` (419540 u16, ідентифікатори
    документів 0..65535 — native IDs 2005-індексу).
- **`tsearch <query>`** (CLI, експеримент): терміни запиту
  кодуються через LmCode-таблицю знаків (підтверджені якорі
  А/О/У/Щ + цифри + Latin; с/і — виведені за частотою у
  словнику, підтвердження — v0.14.0), збігаються зі словником
  lmu.voc (префікси + стем-обрізки 1–3 хвостики), postings
  читаються з lmur.vt. Результат — native Trident-ідентифікатори
  документів + покриття (% закодованих знаків запиту). Зв'язка
  native-ID з номерами документів поточної бази — v0.14.0
  (разом з морфологією .g/.gw і точними word→postings кордонами).
- **Команда `trident` розширена** — інвентар 16 файлів (додано
  windows.kod, lm.a, lmu.use, lmr.use до попередніх 12) +
  декодована статистика: число kod-кодів, R/U fold-пари,
  записи словника, vt-потік (min..max ідентифікаторів).
- **GUI**: чекбокс «Trident-рушій (нативний індекс,
  експеримент)» — за позначення пошук йде через нативний індекс
  (SOFT\); рядки показують native IDs (стовпець файлу
  «trident (native id)», реквізитні стовпці — даші, доки не
  зроблена зв'язка номерів); без SOFT\-індексу — повідомлення в
  статус-барі. Чекбокс вимкнено за замовчуванням → звичайний
  пошук, guitest незмінний.
- Версія **0.12.0 → 0.13.0** (єдине джерело: `CARGO_PKG_VERSION`).
- 82 юніт-тести (нові: `trident::tests` ×11 — kod, fold,
  розділи lm.a, таблиця знаків, покриття, словник, vt,
  префіксний пошук + інтеграційний на реальному індексі).

### EN
- **Trident: experimental native engine** (`src/trident.rs`) —
  decodes parts of the legacy app's native FTS index in the base's
  `SOFT\` folder:
  - `windows.kod` (683 B): the code table, 80 lines
    `<2hex-code> <sep> <byte>` — values ≥0x80 stored as a raw
    byte, <0x80 as two ASCII-hex digits; separators `=`
    (mapping) and `>` (fold);
  - `lm.a` (904 B): case-fold tables, text sections `E:` (Latin
    identity A…Z), `G:` (cross-language anchors: A↔84/a4,
    O↔96/e6, U↔9c/ec, Щ↔9f/ef), `R:`/`U:` — 32 fold pairs each
    (U skips 9a/9b/9d in the main range; extra pairs
    f4f5/f6f7/f8f9);
  - dictionary records `[0x84][word][00 00]` (lmu.voc — 78
    records) and the u16 posting stream from `.vt` (419540 u16,
    doc ids 0..65535 — native ids of the 2005-era index).
- **`tsearch <query>`** (CLI, experimental): query terms are
  encoded through the LmCode character table (confirmed anchors
  A/O/U/Щ + digits + Latin; С/І inferred from dictionary
  frequency, to be confirmed in v0.14.0), matched against the
  lmu.voc dictionary (prefix + stem trimming, 1–3 tails),
  postings are read from lmur.vt. The result is native Trident
  document ids plus a coverage % of encodable query characters.
  Joining the native ids to the current base's docnums — v0.14.0
  (with the .g/.gw morphology and the exact word→postings
  boundaries).
- **Extended `trident` verb** — a 16-file inventory (added
  windows.kod, lm.a, lmu.use, lmr.use to the previous 12) +
  decoded stats: kod codes, R/U fold pairs, dictionary records,
  vt stream (min..max doc ids).
- **GUI**: a "Trident-рушій (нативний індекс, експеримент)"
  checkbox — when checked, search runs through the native index
  (SOFT\); rows show the native ids (file column
  "trident (native id)", field columns dashed until the docnum
  join lands); without a SOFT\ index — a status-bar message.
  Unchecked by default → the classic search, guitest unchanged.
- Version **0.12.0 → 0.13.0** (single source: `CARGO_PKG_VERSION`).
- 82 unit tests (new: `trident::tests` ×11 — kod, fold, lm.a
  sections, the char table, coverage, dict, vt, prefix search +
  a real-index integration test).

---

## v0.12.0 — 2026-10-26

### UK
- **Підсвітка слів запиту в експорті** — новий флаг CLI `--hl`
  (аліас `--highlight`) для `export` + чекбокс «підсвітити слова
  запиту» в GUI (поруч із «зв'язка»). Слова запиту (≥2 знаків,
  регістронезалежно) позначаються в експортованому тексті: md
  `<mark>…</mark>`; html `<mark class="hl">` + CSS
  `mark.hl{background:#ffe86b}`; rtf жовлий хайлайт
  (`\highlight6`/`\highlight0`); docx — окремі біги з
  `<w:highlight w:val="yellow"/>`. pdf/csv — у наступних версіях.
  Ядро: `highlight::Hl` (слова → char-діапазони → партиціонування
  рядка; найдовше слово перетворює, діапазони не перетинаються);
  порожній `Hl` = хайлайт вимкнено (гарячий шлях, без кошту).
- **Партійний друк** — `print <docnum> [docnum ...]`: кілька
  документів (пробіл/кома, діапазони `1-5` через той самий
  розбір, що й `--docs`) друкуються один за одним; системний
  діалог друку — на кожен документ, статус друкується окремим
  рядком («надіслано в принтер» / «відмінено» / помилка).
- **Команда `trident`** — інвентар нативних FTS-індексів
  старої бази: `SOFT\` (lmu.voc / lmr.voc словники, lmur.vt /
  lmru.vt postings, lm*.m метадані, lmu.g / lmr.g графи,
  lmu.gw / lmr.gw ваги, lmsup.dll / lmsup32.dll рушії — 12
  файлів з розмірами). Без папки SOFT (наприклад, БАЗА GK) —
  коректне повідомлення, що індексів немає і пошук йде власним
  рушієм експортера. Повний Trident-рушій («пошук як у головному
  застосунку») — у розробці (v0.13.0).
- Випущено функцію section-headings (0 зі 120 зразкових
  документів — ознак не знайдено; зовільнює експорт).
- Версія **0.11.0 → 0.12.0** (єдине джерело: `CARGO_PKG_VERSION`).
- 71 юніт-тест (нові: `highlight::tests` ×6, hl-тести
  writer'ів ×6: md/html/rtf/docx/bundle) + GUI-самотест
  `--guitest` (тепер звітує `marks=N` — кількість `<mark>` в
  експортованих файлах, і перевіряє чекбокс підсвітки).

### EN
- **Search-term highlighting in exports** — new CLI flag `--hl`
  (alias `--highlight`) for `export` + a "підсвітити слова
  запиту" checkbox in the GUI (next to the bundle checkbox).
  Query terms (≥2 chars, case-insensitive) are marked in the
  exported text: md `<mark>…</mark>`; html `<mark class="hl">` +
  CSS `mark.hl{background:#ffe86b}`; rtf yellow highlight
  (`\highlight6`/`\highlight0`); docx — separate runs with
  `<w:highlight w:val="yellow"/>`. pdf/csv — a later version.
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
  on the exporter's own engine. The full Trident engine
  ("search like the main app") is in development (v0.13.0).
- The section-headings feature was dropped (0 of 120 sampled
  documents show any heading markers; it only slowed exports).
- Version **0.11.0 → 0.12.0** (single source: `CARGO_PKG_VERSION`).
- 71 unit tests (new: `highlight::tests` ×6, writer hl tests
  ×6: md/html/rtf/docx/bundle) + the GUI self-test `--guitest`
  now reports `marks=N` (count of `<mark>` in the exported
  files) and checks the highlight checkbox.

## v0.11.0 — 2026-10-26

### UK
- **«Зв'язка» (зв'язка / bundle): всі експортовані документи в ОДИН файл** —
  CLI `--single` (аліас `--zvizka`) для `export`/`bulk` + чекбокс «зв'язка
  (1 файл на всі документи)» в GUI: md/rtf/docx/html/pdf (csv і так один
  файл). Ім'я: `bundle-<кількість>-<перший docnum>.<ext>`. Кожен формат:
  шапка «Зв'язка — N документ(ів)» + нумерований Зміст (назви + № документів);
  роздільники: md `---`, rtf `\page`, docx `w:br w:type="page"` (кожен
  документ з нової сторінки), html `<section id>` + клікабельні якорі +
  CSS `page-break-before`. **PDF: `/Outlines` (зміст-заатчеки PDF)** — кожен
  документ з нової сторінки, entry з `/Dest` на початковий документ.
- **Авто-вибір шрифту PDF** (`pdf::select_font`): кандидатні
  `arial → times → cambria → georgia → tahoma → verdana`
  (`C:\Windows\Fonts`); обирається перший шрифт із 100% покриттям вжитих
  код-поінтів, інакше — найкраще покриття, інакше arial.ttf. «Stale»-мапінги
  (gid поза межами власної таблиці гліфів) лічать за НЕ покриті. Файли
  шрифтів парсяться один раз на процес (кеш `OnceLock`). `write_pdf` та
  `write_pdf_bundle` тепер вибрають шрифт автоматично (у зв'язці — об'єднання
  кодів усіх документів).
- Версія **0.10.0 → 0.11.0** (єдине джерело: `CARGO_PKG_VERSION`).
- 59 юніт-тестів (нові: структура зв'язок md/rtf/docx/html/pdf,
  `select_font` ×2, `font_coverage`) + GUI-самотест `--guitest` (тепер
  включає зв'язку: `files=1` в звіті).
- Нюанс: на розробничій машині жоден кандидатний шрифт не покриває весь
  кириличний набір (stale arial + відсутній regular Cambria), тож селектор
  обирає arial (рівність покриття → перший у порядку пріоритетів); на
  машині зі звичайним Arial вибір падає на arial із повним покриттям.

### EN
- **“Зв'язка” (bundle): all exported documents into ONE file** — CLI
  `--single` (alias `--zvizka`) for `export`/`bulk` + a “зв'язка (1 файл)”
  checkbox in the GUI: md/rtf/docx/html/pdf (csv is already a single file).
  Name: `bundle-<count>-<first docnum>.<ext>`. Every format gets a
  “Зв'язка — N documents” header + a numbered table of contents (titles +
  doc numbers); separators: md `---`, rtf `\page`, docx `w:br w:type="page"`
  (each document starts on a fresh page), html `<section id>` + clickable
  anchors + CSS `page-break-before`. **PDF: `/Outlines` (PDF
  bookmarks/TOC)** — each document on a fresh page, entries with `/Dest`
  pointing at its start page.
- **Automatic PDF font selection** (`pdf::select_font`): candidates
  `arial → times → cambria → georgia → tahoma → verdana`
  (`C:\Windows\Fonts`); the first font with 100% coverage of the used
  codepoints wins, else the best coverage, else arial.ttf. “Stale” mappings
  (gid beyond the font's own glyph table) count as NOT covered. Font files
  are parsed once per process (`OnceLock` cache). `write_pdf` and
  `write_pdf_bundle` now auto-pick the font (bundle: the union of all
  documents' codepoints).
- Version **0.10.0 → 0.11.0** (single source: `CARGO_PKG_VERSION`).
- 59 unit tests (new: bundle structure md/rtf/docx/html/pdf,
  `select_font` ×2, `font_coverage`) + GUI self-test `--guitest` (now
  includes the bundle mode: `files=1` in the report).
- Note: on the dev machine no candidate font covers the full Cyrillic set
  (stale arial + missing regular Cambria), so the selector returns arial
  (coverage tie → first in preference order); on a machine with a standard
  Arial, arial is chosen with full coverage.

---

## v0.10.0 — 2026-10-25

### UK
- **GDI-друк** (Win32, без зовнішніх додатків): системний діалог друку (`PrintDlgW`)
  + GDI-цикл сторінок; форматування збережене: рядки ВЕЛИКОЮ — жирним, пункти
  списків «•  », центрований запис; відміна в діалозі — результат `Ok(Cancelled)`,
  не помилка. CLI: `zakonexport-cli print <docnum>`; GUI: кнопка «Друк» в вікні
  перегляду (статус: «Друк: завдання надіслано / відмінено / Помилка друку: …»).
- **PDF-експорт** — 6-й формат (CLI `--format pdf` + вибір формату в GUI):
  PDF 1.4, A4 12pt/ведення 15pt, DEFLATE-потоки, вбудований сабсет системного
  Arial (`C:\Windows\Fonts\arial.ttf`: cmap F4/F12, sfnt, FontFile2, кламп
  позагілсці gid на .notdef). Чистий Rust, без зовнішніх PDF-засобів.
- Версія **0.9.0 → 0.10.0** (єдине джерело: `CARGO_PKG_VERSION`).
- 51 юніт-тест (writers, pdf, db, render, …) + GUI-самотест `--guitest`.
- Нюанш шрифту: на розробничій машині системний `arial.ttf` не містить
  кириличних гліфів (GDI підставляє з'єднані шрифти); райтер клампає такі
  коди на `.notdef`. На машині зі звичайним Arial (повна кирилиця) обмеження
  не проявляється.

### EN
- **GDI printing** (Win32, no external apps): system print dialog (`PrintDlgW`)
  + GDI page loop; formatting preserved — ALL-CAPS lines bold, “•  ” list
  markers, centered lines; dialog cancel is an outcome `Ok(Cancelled)`, not an
  error. CLI: `zakonexport-cli print <docnum>`; GUI: “Print” button in the
  preview window (status: “Druk: sent / cancelled / print error: …”).
- **PDF export** — the 6th format (CLI `--format pdf` + GUI format combo):
  PDF 1.4, A4 12pt / 15pt leading, DEFLATE streams, embedded system-Arial
  subset (`C:\Windows\Fonts\arial.ttf`: cmap F4/F12, sfnt, FontFile2,
  out-of-range glyph ids clamped to `.notdef`). Pure Rust, no external
  PDF tooling.
- Version **0.9.0 → 0.10.0** (single source: `CARGO_PKG_VERSION`).
- 51 unit tests (writers, pdf, db, render, …) + GUI self-test `--guitest`.
- Font quirk: on the dev machine the system `arial.ttf` carries no Cyrillic
  glyphs (GDI substitutes linked fonts); the writer clamps those codes to
  `.notdef`. On a machine with a standard full-Cyrillic Arial this does not
  apply.

---

## v0.9.0 — 2026-10-25

### UK
- **Windows x64 бинарники** (CLI + GUI) **з іконкою**: `releases/v0.9.0/` у репо
  та як ассети GitHub-релізу.
- Ядро: пряме **read-only** читання баз Закону (`.db`/`.mb`, cp1251 + fold-кодек),
  паралельний пошук (rayon), кирилично-безпечний (Unicode-UI, `--queryfile`).
- Експорт з оформленням (капс-жирний, «•  », центровані рядки):
  `md` / `rtf` / `docx` / `html` / `csv` (5 форматів).
- Фільтри по реквізитах (`--status`, `--issuer`, `--type`, дати, `--title`),
  пошуки по номеру (`--number`, `--docs`, `--pick`), signal-додатки
  (`--attach`, `signal <docnum>`), `bulk`, `stats`.
- GUI (native-windows-gui): пошук в робочому потоці, перегляд, вибір формату,
  COM folder picker, `--guitest`.

### EN
- **Windows x64 binaries** (CLI + GUI) **with icon**: in-repo `releases/v0.9.0/`
  and as GitHub release assets.
- Core: direct **read-only** access to Zakon DB (`.db`/`.mb`, cp1251 + fold
  codec), parallel search (rayon), Cyrillic-safe (Unicode UI, `--queryfile`).
- Formatting-preserving export (caps-bold, “•  ” markers, centered lines) in
  `md` / `rtf` / `docx` / `html` / `csv` (5 formats).
- Filters by document fields (`--status`, `--issuer`, `--type`, dates,
  `--title`), number search (`--number`, `--docs`, `--pick`), signal
  attachments (`--attach`, `signal <docnum>`), `bulk`, `stats`.
- GUI (native-windows-gui): worker-thread search, preview, format combo,
  COM folder picker, `--guitest`.

---

## v0.8.0 / v0.7.0 / v0.6.0 / v0.4.0 — архівні (дата не збережена / date unknown)

### UK
- Архівні збірки GUI (zip), pre-git ера: детального журналу змін не зберігалося
  (історія версій не велася). Бинарники збережені користувачем і публікуються
  в `releases/zakonexport-gui-<v>.zip` та як ассети відповідних GitHub-релізів.

### EN
- Archived GUI builds (zip), pre-git era: detailed change lists were not
  preserved (version history did not exist yet). The binaries were kept by the
  user and are published under `releases/zakonexport-gui-<v>.zip` and as assets
  of the matching GitHub releases.

---

## v0.1 – v0.3 — перші версії (архівних бинарників немає / no archived binaries)

### UK
- Перші ітерації ядра: читання `.mb`, пошук, базові формати експорту.
  Бинарники не збереглися.

### EN
- Early core iterations: `.mb` reading, search, basic export formats.
  No archived binaries.
