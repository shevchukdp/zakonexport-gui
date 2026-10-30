# Zakon DB Exporter

Нативное Windows-приложение (Rust) для **поиска и экспорта документов из
базы Закон** (`C:\Zakon.Local\Baza`, `C:\Zakon.Local\Baza.GK`): прямое
чтение файлов базы (`.db`/`.mb`) — не требует установки и запуска каких-либо
сторонних компонентов, работает без сети. База читается **только для чтения**.

Два исполняемых файла из одного движка:

| Бинарник | Назначение |
|---|---|
| `zakonexport-gui.exe` | двойной клик → окно (без консоли в release) |
| `zakonexport-cli.exe` | все команды: search / export / bulk / print / … + `gui` (запуск GUI) |

## Возможности

- **Поиск** по всему тексту базы: токенизированный формат `.mb` разворачивается
  нативно; параллельный обход чанков (rayon); регистронезависимый,
  кириллически-безопасный (Unicode Win32 UI, cp1251-консоль не мешает —
  запрос можно взять из файла `--queryfile`).
- **Две базы**: `--base general` (Zakon → `C:\Zakon.Local\Baza`),
  `--base audit` (ГК → `C:\Zakon.Local\Baza.GK`), `--baza <path>` — переопределение.
  Перемещённая установка (другой путь): GUI спрашивает папку и запоминает в
  `%APPDATA%\zakonexport\zakonexport.toml`, CLI-команды используют тот же путь.
- **Экспорт** с сохранением форматирования (капс-жирный, маркеры «•  »,
  центрированные строки): `md`, `rtf`, `docx` (valid zip + CRC-32), `html`,
  `csv` (таблица результатов одним файлом) и **`pdf`** — чистый Rust-райтер:
  PDF 1.4 со встраиваемым **сабсетом системного шрифта**
  (авто-выбор: arial → times → cambria → georgia → tahoma → verdana;
  cmap/glyf/hmtx/loca/post, FLATE-поток, /Differences + /Widths).
  Никакого внешнего PDF-генератора.
- **Зв'язка (bundle): все документы в ОДИН файл** — `--single` (алиас
  `--zvizka`) для `export`/`bulk` + чекбокс «зв'язка (1 файл)» в GUI:
  md/rtf/docx/html/pdf одним файлом `bundle-<N>-<docnum>.<ext>`: шапка
  «Зв'язка — N документов», нумерованное Зміст (название + № каждого
  документа), каждый документ с новой страницы; у PDF — `/Outlines`
  (закладки/содержимое PDF, кликабельные Dest на стартовую страницу).
- **GDI-друк** (без Word и других приложений): системный диалог печати
  (`PrintDlg`), GDI-цикл страниц, отмена → статус «відмінено», не ошибка.
  CLI: `print <docnum> [docnum ...]` (партия: пробел/кома, диапазоны
  `1-5`; диалог печати — на каждый документ, статус — построчно);
  в GUI — кнопка «Друк» в окне предпросмотра.
- **Подсветка слов запроса в экспорте** (`--hl`, алиас `--highlight`,
  чекбокс «підсвітити слова запиту» в GUI): слова запроса (≥2 знака,
  без учёта регистра) подсвечиваются в выданном тексте — md
  `<mark>…</mark>`, html `<mark class="hl">` (+ CSS), rtf — жёлтый
  highlight, docx — выделенные run'ы (`w:highlight`). pdf/csv —
  в следующих версиях.
- **Команда `trident`** — инвентарь нативных FTS-индексов старой
  базы: `SOFT\` (словники lmu.voc/lmr.voc, postings lmur.vt/lmru.vt,
  метаданные lm*.m, графи/веса lmu.g/lmr.g/lmu.gw/lmr.gw,
  движки lmsup.dll/lmsup32.dll, кодовая таблица windows.kod,
  fold-таблицы lm.a, usage-прапорцы lmu.use/lmr.use) — 16
  файлов с размерами + декодованная статистика (kod-коды, R/U
  fold-пары, записи словника, vt-поток); без папки SOFT (база
  GK) — корректное «индексов нет, поиск идёт своим движком
  экспортера».
- **`tsearch <query>` — Trident-движок (v0.39.0: production)**
  (нативный индекс, `src/trident.rs`): запрос кодируется через
  LmCode-таблицу (подтверждённые якоря А/О/У/Щ + цифры + Latin;
  с/і — кандидаты по частоте, подтверждение в v0.14.0),
  сопоставляется со словарём lmu.voc (префиксы + стем-обрезки),
  postings читаются из lmur.vt → native Trident-ID документов
  (2005-индекс; связь с номерами текущей базы + морфология
  .g/.gw — v0.14.0). **v0.39.0 (roadmap v0.40.0): маршрут помечен
  production** — 100-запросный оракульный корпус (`tsearch --corpus
  corpus/trident_oracle_100.txt`: полная цепочка tsearch vs ftsearch,
  один проход по `.mb`; матрица збігу + failure-mode таблица
  `TridentError` (NoMatch/EmptyJoin/OutOfRange, zero panics), UA+EN — в
  CHANGELOG v0.39.0); `--fetch N` / `--print` (v0.38.0). EN: the
  Trident route is marked **production** (roadmap v0.40.0): a 100-query
  oracle corpus (full tsearch chain vs the ftsearch oracle, single `.mb`
  pass) with the `TridentError` failure-mode table (UA+EN in CHANGELOG
  v0.39.0). В GUI — чекбокс «Trident-рушій (нативний
  індекс, експеримент)»: при отмеченном флаге поиск идёт через
  нативный индекс (строки = native IDs, реквизитные столбцы —
  даше); выключен по умолчанию.
- **Фильтры по реквизитам**: `--status actual|canceled|notpub|all`,
  `--issuer`, `--type`, `--date-from/--date-to`, `--title`.
- **По номеру документа**: `--number` (в т.ч. «з № 249», «до 249»),
  точный выбор `--docs 136974,222-225` (работает и без запроса),
  интерактивный `--pick`.
- **Приложения (signal)**: `--attach` копирует `<baza>\signal\<docnum>.<ext>`
  к экспорту и делает пометку в отчёте; команда `signal <docnum>` —
  локализовать приложение документа.
- **GUI** (native-windows-gui): поиск в рабочем потоке (UI не блокируется),
  предпросмотр документа, выбор формата из списка, COM folder picker для
  пути экспорта, самотест `--guitest` (пишет `gui_ready.txt`).
- **Чистый Rust**: BZip2 (libbz2-rs), inflate (miniz_oxide) — ни одной C-зависимости,
  сборка без C-компилятора в PATH.

## Сборка

Требуется rustup с GNU-тулчейном (self-contained mingw, C-компилятор не нужен):

```
rustup default stable-x86_64-pc-windows-gnu
cargo build --release
```

Результат: `target\release\zakonexport-cli.exe`, `target\release\zakonexport-gui.exe`
(без иконки). Иконку: `powershell -File res\inject_icon.ps1`
(+ проверка `res\extract_icon.ps1` / `make_icon.ps1`).

Тесты:

```
cargo test --lib       # юнит-тесты движка, writers, pdf, highlight, trident (82 теста)
zakonexport-gui.exe --guitest --baza C:\Zakon.Local\Baza.GK   # GUI-самотест
```

## CLI

```
zakonexport-cli search <query> [--and|--or|--phrase] [--number] [--max n] [--fields]
zakonexport-cli export <query> [фильтры] [--format md|rtf|docx|html|csv|pdf] [--out dir]
                       [--docs n1,n2-n5 | --pick] [--attach] [--single] [--hl]
zakonexport-cli export --docs 136974,222-225 [--format pdf] [--out dir]
zakonexport-cli fields <docnum>      # реквизиты документа
zakonexport-cli extract <docnum>     # полный текст документа
zakonexport-cli signal <docnum>      # найти signal-приложение (если есть)
zakonexport-cli print  <docnum> [docnum ...]  # GDI-друк (системный диалог печати; партия)
zakonexport-cli trident [dump <f> [off] [len] | uvt [--synth [dir]]]
                        # инвентарь нативных FTS-индексов (SOFT\) + sub-verbs;
                        # uvt (v0.40.0) = .uvt write-mode update-cache status / synth
zakonexport-cli tsearch <query> [--oracle] [--fetch N] [--print] [--corpus <file>]
                        # Trident-пошук (v0.39.0: production; --corpus = 100-запитний оракульний корпус)
zakonexport-cli bulk [--format …] [--out dir] [--max n]
zakonexport-cli stats
zakonexport-cli about               # версия/автор
zakonexport-cli gui [--guitest] [--base general|audit|<path>] [--baza <path>]
```

Полезные флаги поиска:

```
--queryfile <utf8.txt>   # запрос из файла (обходит cp1251-консоль)
--list <utf8.txt>        # весь пронумерованный список результатов (с реквизитами) в файл
--out <dir>              # папка экспорта (по умолчанию C:\temp\opencode\export)
```

## Структура

```
src\main.rs        CLI-команды (все verb'ы)
src\bin\zakonexport-gui.rs  точка входа GUI (без консоли)
src\lib.rs         версия, about, persistence пути базы
src\db.rs          .db/.mb: запись/span'ы, cp1251+fold-кодек, поисковый движок
src\render.rs      развёртка токенов в текст, титулы/превью, выбор базы
src\writers.rs     md/rtf/docx/html/csv + диспетч export_hits()
src\pdf.rs         TTF-парсер/сабсетирование + PDF 1.4-сборка (FontFile2)
src\printdoc.rs    GDI-печать: PrintDlg + цикл страниц, Win32 FFI вручную
src\gui.rs         native-windows-gui UI (рабочий поток, предпросмотр, «Друк»)
src\picker.rs      COM IFileDialog folder picker
src\naming.rs      шаблоны/санитизация/дедупликация имён, run-папки
src\fields.rs      парсер реквизитов; src\mdconv.rs, src\signal.rs, src\update.rs
src\highlight.rs   подсветка слов запроса (Hl: terms → char-диапазоны → партиции)
src\trident.rs     экспериментальный Trident-движок: kod/lm.a/словник/vt (SOFT\)
res\               иконки + inject_icon.ps1 (реинжект иконки после release)
build\run-gui.bat  запуск GUI двойным кликом
```

## Известные особенности

- **Шрифт PDF**: встраиваемый сабсет берётся из системы с **авто-выбором**
  (`pdf::select_font`): кандидаты arial → times → cambria → georgia →
  tahoma → verdana; выбирается первый шрифт с 100% покрытием использованных
  кодов (иначе — лучшее покрытие, иначе arial.ttf). На части машин
  системный `arial.ttf` имеет «усталый» cmap: часть кириллических кодов
  указывает за пределы собственной glyph-таблицы шрифта; такие маппинги
  считаются непокрытыми (gid за пределами таблицы → `.notdef`), и селектор
  переходит на лучшего кандидата, если тот покрывает больше. На машинах с
  обычным Arial (full Cyrillic) выбирается arial с полным покрытием.
- База читается **только для чтения**; каталог по умолчанию
  `C:\temp\opencode\export`.

## Релізи / Releases

Історія версій — в **CHANGELOG.md**: кожен запис UA + EN, сторінки
GitHub-релізу несуть ті самі нотатки (UA + EN).
Version history — in **CHANGELOG.md**: every entry UK + EN, the GitHub
release pages carry the same notes (UK + EN).

Поточний / Current:
- **v0.42.0** — **GUI: native-id export button (roadmap v0.43.0)**:
  Trident-панель (окреме модальне вікно) тепер **експортує** — новий
  кнопа **«Експорт (native-id)»** запускає шлях `--trident-docs` (v0.37.0)
  з GUI: запит + база з панелі → tsearch stem-join → openable native
  u16 docnums → md-бандл, де **кожен документ несе рядок
  native-id-довідки** (`нативні u16 (Trident): cand@0x…`). Один
  pipeline без дублювання: спільна `writers::trident_docs_bundle()`
  (join → openable docnums → бандл + evidence-колонка; `TridentBundle`
  — лічильники + папка) виконує і CLI `export --trident-docs` (live:
  «не» → 32 record-ів · 2372 openable u16 → 967 docnum → 967 файлів),
  і GUI-кнопу (md, auto-lang, воркер-потік + окремий `trident_rx`-канал
  + `trident_busy`-захист + log у результатне поле панелі; panel —
  separate window → rows=430 untouched). / **The GUI native-id export
  button (roadmap v0.43.0):** the Trident panel (the separate modal
  window) now **exports** — the new **«Експорт (native-id)»** button
  runs the `--trident-docs` path (v0.37.0) from the GUI: the panel's
  query + base → the tsearch stem-join → the openable native u16
  docnums → an md bundle where **every document carries its native-id
  evidence line** (`нативні u16 (Trident): cand@0x…`). One pipeline,
  no duplication: the shared `writers::trident_docs_bundle()`
  (join → openable docnums → the bundle + the evidence column;
  `TridentBundle` — the counters + the output dir) now serves both the
  CLI verb (live: «не» → 32 records · 2372 openable u16 → 967 docnums
  → 967 files) and the GUI button (md, auto-lang, a worker thread + a
  dedicated `trident_rx` channel + the `trident_busy` guard + the log
  lines in the panel's result box; the panel is a separate window →
  rows=430 untouched). 211 unit tests.
- **v0.41.0** — **Rust `LMaster_*` driver (roadmap v0.42.0)**: FFI-facade
для `lmsup32.dll` (32-бітний PE32 0x14c — 64-бітний процес не може
LoadLibrary-ти його, err 126): **32-бітний helper**
`ffi32/lmsup32-ffi32.exe` (i686; 4-й release-асет
`lmsup32-ffi32-0.41.0.exe`) + новий sub-verb
`trident lmaster [--corpus <file>] [--ffi32 <path>]`:
`EXPORTS OK 4/4` (`LMaster_{Begin,End,Lang,Tran}`) + `LMaster_Lang`
drivable («ne» → 69); `LMaster_Begin`/`Tran` (DLL-join) **blocked** на
не-експортованій Lm-шлях-глобальній рушія `DAT_1002a30c`="lm.a"
(`LmOpen Error` — задокументовано, не дефект); **ground-truth join —
Rust-side**: `corpus_crosscheck` через `TridentIndex` на 100-запитному
корпусі (100 запитів · 13 з joined docs · 8176 distinct native u16
docs) — детермінований референс для DLL-join. Rust: `lmaster.rs`
(C-ABI type-aliases `LMaster*`, чистий protocol-parser
`parse_lm_line`/`parse_report`, `LmasterDriver::locate`/`run`
zero-panic, `corpus_crosscheck`/`corpus_summary`), 210 юніт-тестів. /
  **Rust `LMaster_*` driver (roadmap v0.42.0):** the FFI facade over
  `lmsup32.dll` (a 32-bit PE32 0x14c — a 64-bit process cannot
  LoadLibrary it, err 126): the **32-bit helper**
  `ffi32/lmsup32-ffi32.exe` (i686; the 4th release asset
  `lmsup32-ffi32-0.41.0.exe`) + the new sub-verb `trident lmaster
  [--corpus <file>] [--ffi32 <path>]`: `EXPORTS OK 4/4`
  (`LMaster_{Begin,End,Lang,Tran}`) + `LMaster_Lang` drivable ("ne" →
  69); `LMaster_Begin`/`Tran` (the DLL join) **blocked** on the
  engine's non-exported Lm path global `DAT_1002a30c`="lm.a" (`LmOpen
  Error` — documented, not a defect); the **ground-truth join is the
  Rust-side** `corpus_crosscheck` through `TridentIndex` over the
  100-query corpus (100 queries · 13 with joined docs · 8176 distinct
  native u16 docs) — the deterministic reference for the DLL join.
  Rust: `lmaster.rs` (the `LMaster*` C-ABI aliases, the pure protocol
  parser `parse_lm_line`/`parse_report`, `LmasterDriver::locate`/`run`
  zero-panic, `corpus_crosscheck`/`corpus_summary`), 210 unit tests.
- **v0.40.0** — **поведення `.uvt` (roadmap v0.41.0)**: write-mode
  per-language update cache (рушій відкриває `lm{lang}.uvt` лише на
  запис; у read-only базі файл відсутній). Новий sub-verb
  `trident uvt [--synth [dir]]`: per-language статус `.uvt` (ABSENT/
  PRESENT + parse) + `--synth` — синтез відсутніх `.uvt` **на копії
  бази** (16-файловий inventory → `<dir>\SOFT\`; pinned 204B
  FREE-container form: 64B header з vendor + name lmUR.uvt/lmRU.uvt +
  tail=0 + 140B zero body — форма FUN_10011e10) + verify (parse 0
  records + `TridentIndex::open` на копії + search parity:
  `tsearch «не»` оригінал == копія, 967 u16 == 967 u16). Rust:
  `UVT_LANGS` + `UvtState`/`uvt_states()` + `synth_uvt_from_vt()`
  (`trident_vt.rs`), 202 юніт-тести. /
  **`.uvt` behavior (roadmap v0.41.0):** the write-mode per-language
  update cache (the engine opens `lm{lang}.uvt` for writing only; the
  file is absent in the read-only base). New sub-verb `trident uvt
  [--synth [dir]]`: the per-language `.uvt` status (ABSENT/PRESENT +
  parse) + `--synth` — synthesizes the absent `.uvt` caches **on a base
  copy** (the 16-file inventory → `<dir>\SOFT\`; the pinned 204B
  FREE-container form: the 64B header with the vendor + the lmUR.uvt/
  lmRU.uvt name + tail=0 + the 140B zero body — the FUN_10011e10
  shape) + verify (parse 0 records + `TridentIndex::open` on the copy +
  search parity: `tsearch «не»` original == copy, 967 u16 == 967 u16).
  Rust: `UVT_LANGS` + `UvtState`/`uvt_states()` +
  `synth_uvt_from_vt()` (`trident_vt.rs`), 202 unit tests.
- **v0.39.0** — **hardening + production marking (roadmap v0.40.0)**:
  100-запитний оракульний корпус (`corpus/trident_oracle_100.txt`,
  `tsearch --corpus <file>` — повна tsearch-ланцюга vs ftsearch-оракула,
  один `.mb`-прохід; збіг-матриця + failure-mode таблиця
  `TridentError` (NoMatch/EmptyJoin/OutOfRange, zero panics)),
  production-статусний рядок у слові `trident` («Trident-маршрут:
  production (оракельно підтверджено, roadmap v0.40.0)»), явна
  `TridentError`-поверхня замість тихих порожніх (no panics on partial
  data), швидкий presence-only oracle-скан (`scan_present`), 199
  юніт-тестів. /
  **Hardening + production marking (roadmap v0.40.0):** the 100-query
  oracle corpus (`corpus/trident_oracle_100.txt`, `tsearch --corpus
  <file>` — the full tsearch chain vs the ftsearch oracle in a single
  `.mb` pass; the agreement matrix + the `TridentError` failure-mode
  table (NoMatch/EmptyJoin/OutOfRange, zero panics)), the production
  status line in the `trident` verb ("Trident route: production
  (oracle-confirmed, roadmap v0.40.0)"), the explicit `TridentError`
  error surface instead of silent empties (no panics on partial data),
  and the fast presence-only oracle scan (`scan_present`), 199 unit tests.
- **v0.16.0** — **виявлено базовий кодувальник документів**
  (`src/ftsearch.rs`) + **`ftsearch` — повний текстовий пошук по
  `.mb`** (механізм оракула старої програми, без B-tree). Текст `.mb`
  — це cp1251 + українські літери у `0x80–0xBF` (`І`=`0xB2`, `і`=`0xB3`,
  `Є`=`0xAA`, `ї`=`0xBF`; «Конституція» = `CA EE ED F1 F2 E8 F2 F3
  F6 B3 FF`). `ftsearch «конституція»` → **222 збіги** (оракел = 223,
  99,5%), перші 40 збігаються з оракулом 1-до-1. Повний декод
  щільного B-tree + word→postings — v0.17.0. 91 юніт-тест. /
  **Discovered the base document codepage** (`src/ftsearch.rs`) +
  **`ftsearch` — full-text search over the `.mb` files** (the legacy
  app's oracle mechanism, no B-tree). The `.mb` text is cp1251 +
  Ukrainian letters in `0x80–0xBF` (`І`=`0xB2`, `і`=`0xB3`, `Є`=`0xAA`,
  `ї`=`0xBF`; «Конституція» = `CA EE ED F1 F2 E8 F2 F3 F6 B3 FF`).
  `ftsearch «конституція»` → **222 hits** (oracle = 223, 99.5%), the
  first 40 match the oracle 1-to-1. The full packed B-tree decode +
  word→postings — v0.17.0. 91 unit tests.
- **v0.15.0** — Trident: **загальний формат записів словника**
  (`[tag][word NUL][flag]`, tag 0x84/0x8d, flag 0x00/0x01):
  вилучення словника зросло з 78 до ~1000 стем (реальні «зувати»,
  «раст», «зват», «дити»), структурне декодування `.gw` (u16
  таблиця морфологічних ваг, бит 0x8000), задокументовано
  розкладку word-структур B-tree (NULL-термінований масив
  вказівників, case-fold порівняння з DLL). `tsearch` тепер
  влучає реальні стеми («раст» → 279, «зват» → 294 документів);
  «конституція» лишається 0 (щільний B-tree — далі). 86 юніт-тестів. /
  Trident: **general dictionary record format** (`[tag][word NUL][flag]`,
  tag 0x84/0x8d, flag 0x00/0x01): dictionary extraction grew 78 → ~1000
  stems (real «зувати», «раст», «зват», «дити»), structural decode of
  `.gw` (the u16 morphology-weight table, 0x8000 flag bit), and the
  B-tree word-struct layout documented (NULL-terminated pointer array +
  case-fold comparator, from the DLL). `tsearch` now hits real stems
  («раст» → 279, «зват» → 294 docs); «конституція» stays 0 (packed
  B-tree — next). 86 unit tests.
- **v0.14.0** — Trident: **повна таблиця відповідності
  знаків** (`src/trident.rs::LM_CHAR_TABLE`): cp1251-порядок,
  усі слоти 0x80–0x9F (А–Я), 0xA0–0xAF (а–п), 0xE0–0xEF (р–я)
  + українські 0xF2–0xF9 (Ґ/ґ/Є/є/І/і/Ї/ї) — підтверджено
  через windows.kod, DLL-таблицю та реальні слова lmu.voc
  («нема», «бути»). `tsearch` тепер кодує «конституція» на 100%
  і показує LmCode-код + розшифровку; `п30` → 2670 документів.
  Морфологія .g/.gw, B-tree-словник і зв'язка native-ID →
  docnum поточної бази — v0.15.0. 84 юніт-тести. / Trident:
  **full character correspondence**
  (`src/trident.rs::LM_CHAR_TABLE`): cp1251 order, all
  0x80–0x9F (А–Я), 0xA0–0xAF (а–п), 0xE0–0xEF (р–я) slots + the
  Ukrainian 0xF2–0xF9 (Ґ/ґ/Є/є/І/і/Ї/ї) — confirmed via
  windows.kod, the DLL keyboard table and real lmu.voc words
  («нема», «бути»). `tsearch` now encodes «конституція» at 100%
  and shows the LmCode encoding + decode; `п30` → 2670 docs.
  .g/.gw morphology, the B-tree dictionary and the
  native-ID → current docnums join — v0.15.0. 84 unit tests.
- **v0.13.0** — Trident: експериментальний нативний рушій
  (`src/trident.rs`: декод windows.kod, lm.a, записів словника
  lmu.voc і u16 posting-потіку .vt), CLI `tsearch <query>`
  (нативні Trident-ID документів + покриття знаків), розширена
  команда `trident` (16 файлів + статистика), GUI-чекбокс
  «Trident-рушій», 82 юніт-тести. / Trident: experimental
  native engine (`src/trident.rs`: windows.kod, lm.a, lmu.voc
  dictionary records and the u16 posting stream decoded), CLI
  `tsearch <query>` (native Trident doc ids + char coverage),
  extended `trident` verb (16 files + stats), a GUI
  "Trident-рушій" checkbox, 82 unit tests.
- **v0.12.0** — підсвітка слів запиту в експорті (`--hl` / чекбокс
  «підсвітити слова запиту»; md `<mark>`, html `<mark class=hl>`,
  rtf/docx жовтий хайлайт; pdf/csv — пізніше), партійний друк
  `print <docnum> [docnum ...]`, команда `trident` (інвентар
  SOFT-індексів; повний Trident-рушій — у розробці), 71 юніт-тест. /
  query-term highlighting in exports (`--hl` / GUI checkbox; md
  `<mark>`, html `<mark class=hl>`, rtf/docx yellow highlight;
  pdf/csv later), batch `print <docnum> [docnum ...]`, `trident`
  command (SOFT index inventory; full Trident engine in development),
  71 unit tests.
- **v0.11.0** — «зв'язка» (все документы в один файл: `--single` / чекбокс
  «зв'язка (1 файл)»; md/rtf/docx/html/pdf, у PDF — `/Outlines` зміст +
  новий документ з нової сторінки), авто-вибір PDF-шрифту (кириличний
  fallback arial→times/cambria/…), 59 юніт-тестів. / “Зв'язка” bundle:
  all documents into ONE file (`--single` / GUI checkbox; md/rtf/docx/
  html/pdf, PDF gets a `/Outlines` TOC + fresh page per document),
  automatic PDF font selection (Cyrillic fallback), 59 unit tests.
- **v0.10.0** — GDI-друк (системний діалог, `print <docnum>`, кнопка «Друк»),
  PDF-експорт (чистий Rust, Arial-сабсет), 51 юніт-тест. / GDI printing
  (system dialog), pure-Rust PDF export (Arial subset), 51 unit tests.
- **v0.9.0** — ядро: read-only `.db`/`.mb`, пошук (rayon), експорт
  md/rtf/docx/html/csv, bulk, фільтри, GUI, реквізити/extract/signal;
  бинарники з іконкою. / Core: read-only `.db`/`.mb`, search (rayon),
  md/rtf/docx/html/csv export, bulk, filters, GUI, fields/extract/signal;
  icon-injected binaries.
- **v0.8.0 / v0.7.0 / v0.6.0 / v0.4.0** — архівні GUI-zip (pre-git) /
  archived GUI zips (pre-git era).
- **v0.1–v0.3** — ранні ітерації, бинарників немає / early iterations,
  no archived binaries.

Процес релізу / Release process: `CHANGELOG.md` (UA+EN) → тег + сторінка
релізу (UA+EN) → `releases/<v>/` в репо (**changes.md** UA+EN + бинарники)
+ ассети GitHub-релізу (`.github/workflows/release-assets.yml`;
завантаження йде через `uploads.github.com`). / `CHANGELOG.md` (UA+EN) →
tag + release page (UK+EN) → `releases/<v>/` in-repo (**changes.md**
UK+EN + binaries) + release assets (uploads go through `uploads.github.com`).
