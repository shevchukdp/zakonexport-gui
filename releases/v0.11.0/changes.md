# v0.11.0 — Changes / Зміни (2026-10-26)

## UK / Українська

- **«Зв'язка» (зв'язка / bundle): всі експортовані документи в ОДИН файл** —
  CLI `--single` (аліас `--zvizka`) для `export`/`bulk` + чекбокс «зв'язка
  (1 файл на всі документи)» в GUI: md/rtf/docx/html/pdf (csv і так один
  файл). Ім'я файлу: `bundle-<кількість>-<перший docnum>.<ext>`. Кожен
  формат отримує шапку «Зв'язка — N документ(ів)» + нумерований Зміст
  (назви документів + їхні №); роздільники: md `---`, rtf `\page`,
  docx `w:br w:type="page"` (кожен документ починається з нової сторінки),
  html `<section id>` + клікабельні якорі + CSS `page-break-before`.
  **PDF: `/Outlines` (зміст-закладки PDF)** — кожен документ з нової
  сторінки, entry з `/Dest` на початковий документ.
- **Авто-вибір шрифту PDF** (`pdf::select_font`): кандидатні
  `arial → times → cambria → georgia → tahoma → verdana`
  (`C:\Windows\Fonts`); обирається перший шрифт із 100% покриттям вжитих
  код-поінтів, інакше — найкраще покриття, інакше arial.ttf. «Stale»-
  мапінги (gid поза межами власної таблиці гліфів шрифту) лічать за
  НЕ покриті. Файли шрифтів парсяться один раз на процес (кеш `OnceLock`).
  `write_pdf` та `write_pdf_bundle` вибрають шрифт автоматично (у зв'язці —
  об'єднання код-поінтів усіх документів).
- Версія **0.10.0 → 0.11.0** (єдине джерело: `CARGO_PKG_VERSION`).
- 59 юніт-тестів (нові: структура зв'язок md/rtf/docx/html/pdf,
  `select_font` ×2, `font_coverage`) + GUI-самотест `--guitest` (тепер
  включає зв'язку: `files=1` в звіті).
- Нюанс: на розробничій машині жоден кандидатний шрифт не покриває весь
  кириличний набір (stale arial + відсутній regular Cambria), тож селектор
  обирає arial (рівність покриття → перший у порядку пріоритетів); на
  машині зі звичайним Arial вибір падає на arial із повним покриттям.

## EN / English

- **“Зв'язка” (bundle): all exported documents into ONE file** — CLI
  `--single` (alias `--zvizka`) for `export`/`bulk` + a “зв'язка (1 файл)”)
  checkbox in the GUI: md/rtf/docx/html/pdf (csv is already a single file).
  File name: `bundle-<count>-<first docnum>.<ext>`. Every format gets a
  “Зв'язка — N documents” header + a numbered table of contents (document
  titles + their numbers); separators: md `---`, rtf `\page`,
  docx `w:br w:type="page"` (each document starts on a fresh page),
  html `<section id>` + clickable anchors + CSS `page-break-before`.
  **PDF: `/Outlines` (PDF bookmarks/TOC)** — each document on a fresh page,
  entries with `/Dest` pointing at its start page.
- **Automatic PDF font selection** (`pdf::select_font`): candidates
  `arial → times → cambria → georgia → tahoma → verdana`
  (`C:\Windows\Fonts`); the first font with 100% coverage of the used
  codepoints wins, else the best coverage, else arial.ttf. “Stale”
  mappings (gid beyond the font's own glyph table) count as NOT covered.
  Font files are parsed once per process (`OnceLock` cache). `write_pdf`
  and `write_pdf_bundle` now auto-pick the font (bundle: the union of all
  documents' codepoints).
- Version **0.10.0 → 0.11.0** (single source: `CARGO_PKG_VERSION`).
- 59 unit tests (new: bundle structure md/rtf/docx/html/pdf,
  `select_font` ×2, `font_coverage`) + GUI self-test `--guitest` (now
  includes the bundle mode: `files=1` in the report).
- Note: on the dev machine no candidate font covers the full Cyrillic set
  (stale arial + missing regular Cambria), so the selector returns arial
  (coverage tie → first in preference order); on a machine with a standard
  Arial, arial is chosen with full coverage.

Full per-version history: `CHANGELOG.md` (repo root).
