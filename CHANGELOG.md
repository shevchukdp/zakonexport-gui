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

## v0.16.0 — 2026-10-27

### UK
- **Виявлено базовий кодувальник документів** (`src/ftsearch.rs`) —
  текст документів `.mb` у базі — це **не** звичайний cp1251 і не
  LmCode, а cp1251 (стандартна кирилиця) з українськими літерами у
  символічному діапазоні `0x80–0xBF`:
  - стандартна кирилиця (cp1251): великі `А–П`=`0xC0–0xCF`,
    `Р–Я`=`0x90–0x9F`; малі `а–п`=`0xE0–0xEF`, `р–я`=`0xF0–0xFF`;
  - українські перевизначення: **`І`=`0xB2`, `і`=`0xB3`**
    (підтверджено — «Конституція» в `class1.mb` = `CA EE ED F1 F2 E8
    F2 F3 F6 B3 FF` на самому офсеті оракула 42 = 0x2A); `Є`=`0xAA`,
    `ї`=`0xBF` (визначено з тексту); решта слотів (`є`, `ґ`, `Ї`, `Г`)
    — рідкісні, далі.
- **`ftsearch` — повний текстовий пошук по `.mb`** (нова
  `src/ftsearch.rs` + CLI-дієслово `ftsearch <запит>`): кодує запит
  через базовий кодувальник і сканує **всі** `.mb`-файли
  (верхній рівень + `text\`, рекурсивно) у двох реєстрах (нижній +
  перша велика) — **саме механізм оракула** старої програми, без
  B-tree. Зв'язка native→оракел реалізована: «конституція» →
  **222 збіги** (оракел = 223, 99,5%), і перші 40 збігів збігаються
  з оракулом 1-до-1 (`class1.mb @ 42`, `1042.mb @ 1840761`, …).
  Пошук за стеমে («конституц» = ~12 000) **не** оракул — підтверджено,
  що оракул — точне слово (case-insensitive).
- **Дієслово `tsearch`** (нативний B-tree-індекс) лишається
  експериментальним; повний декод щільного B-tree + word→postings
  кордони — v0.17.0. 91 юніт-тест.

### EN
- **Discovered the base document codepage** (`src/ftsearch.rs`) — the
  `.mb` document text is **not** plain cp1251 or LmCode: it is cp1251
  (standard Cyrillic) with the Ukrainian letters placed in the
  `0x80–0xBF` symbol range:
  - standard Cyrillic (cp1251): upper `А–П`=`0xC0–0xCF`, `Р–Я`
    =`0x90–0x9F`; lower `а–п`=`0xE0–0xEF`, `р–я`=`0xF0–0xFF`;
  - Ukrainian overrides: **`І`=`0xB2`, `і`=`0xB3`** (confirmed —
    «Конституція» in `class1.mb` = `CA EE ED F1 F2 E8 F2 F3 F6 B3 FF`
    at the oracle's own offset 42 = 0x2A); `Є`=`0xAA`, `ї`=`0xBF`
    (read from the text); the remaining slots (`є`, `ґ`, `Ї`, `Г`) are
    rare — later.
- **`ftsearch` — full-text search over the `.mb` files** (new
  `src/ftsearch.rs` + the `ftsearch <query>` CLI verb): encodes the
  query through the base codepage and scans **all** `.mb` files
  (top level + `text\`, recursive) in two cases (lower + title) —
  **exactly the legacy app's oracle mechanism**, no B-tree needed. The
  native→oracle join is implemented: «конституція» → **222 hits**
  (oracle = 223, 99.5%), and the first 40 hits match the oracle
  1-to-1 (`class1.mb @ 42`, `1042.mb @ 1840761`, …). The stem search
  («конституц» ≈ 12 000) is *not* the oracle — confirming the oracle is
  an exact-word (case-insensitive) match.
- The `tsearch` verb (native B-tree index) stays experimental; the full
  packed B-tree decode + word→postings boundaries — v0.17.0. 91 unit
  tests.

---

## v0.15.0 — 2026-10-27

### UK
- **Trident: ЗАГАЛЬНИЙ формат записів словника**
  (`src/trident.rs::extract_dict_words`) — формат запису
  `[tag][word NUL][flag]` (tag 0x84/0x8d, flag 0x00/0x01), а не лише
  суворий `[0x84][word][00 00]`. Вилучення словника з `lmu.voc`
  зросло з **78 до ~1002 унікальних стем** (O(n)-скан з обмеженим
  12-байтовим пошуком вперед). Словник тепер містить реальні
  українські стеми — «зувати», «раст»/«ряст», «зват», «дити»,
  «овувати» — разом із морфологічними клас-кодами (`зувати0`,
  `титиь33`).
- **`.gw` — морфологічна таблиця ваг** (`read_gw_u16`):
  `lmu.gw` (268 B = 134 u16) / `lmr.gw` (264 B = 132 u16) —
  плоский little-endian u16-масив без заголовка; бит `0x8000` —
  прапорець (ставиться на останніх записах, напр. `8053 8054 8057
  8058`). Завантажується в `TridentIndex::gw_u16` (структурне
  декодування; повний семантичний декод потребує графу `.g` —
  roadmap).
- **Б-дерево / word-структури задокументовано** (з `lmsup32.dll`):
  пошук слова `FUN_10021520` = лінійний скан **NULL-термінованого
  масиву вказівників на word-структури**; word-байти NUL-терміновані
  за offset +1 у структурі (≥0xe1 байт, поля: +0xc0 ptr, +0xd5/+0xd7
  flag-байти, +0xdd ptr); порівняння — `FUN_1001bca0` (case-fold:
  кожен байт через `FUN_10011930(x,0x80)`). «конституція»
  (`aa ae ad e1 …`) **немає** як сирого байт-рядка в `lmu.voc`
  (0 збігів) — повний набір слів у **щільному (Patricia-) B-дереві**,
  декод якого — наступний крок.
- **`tsearch` влучає реальні стеми**: «раст» → 279 документів,
  «зват» → 294 документів (раніше 0). «п30» → 310 (було 2670 —
  пропорційна оцінка тепер ділиться на повний словник 1002 слова, а
  не 78). «конституція» → 0 (щільне B-дерево + join native-ID →
  docnum поточної бази — далі в roadmap). 86 юніт-тестів.

### EN
- **Trident: GENERAL dictionary record format**
  (`src/trident.rs::extract_dict_words`) — the record format is
  `[tag][word NUL][flag]` (tag 0x84/0x8d, flag 0x00/0x01), not only
  the strict `[0x84][word][00 00]`. Extraction from `lmu.voc` grew
  from **78 to ~1002 unique stems** (O(n) scan with a bounded 12-byte
  forward look). The dictionary now contains real Ukrainian stems —
  «зувати», «раст»/«ряст», «зват», «дити», «овувати» — together with
  morphological class-codes (`зувати0`, `титиь33`).
- **`.gw` — morphology weight table** (`read_gw_u16`):
  `lmu.gw` (268 B = 134 u16) / `lmr.gw` (264 B = 132 u16) — a flat
  little-endian u16 array with no header; the `0x8000` bit is a flag
  (set on the last entries, e.g. `8053 8054 8057 8058`). Loaded into
  `TridentIndex::gw_u16` (structural decode; the full semantic decode
  needs the `.g` grammar graph — roadmap).
- **B-tree / word-struct layout documented** (from `lmsup32.dll`):
  word lookup `FUN_10021520` = linear scan of a **NULL-terminated
  array of word-struct pointers**; word bytes are NUL-terminated at
  offset +1 in the struct (≥0xe1 bytes; fields +0xc0 ptr, +0xd5/+0xd7
  flag bytes, +0xdd ptr); comparison — `FUN_1001bca0` (case-fold:
  each byte through `FUN_10011930(x,0x80)`). «конституція»
  (`aa ae ad e1 …`) is **absent** as a raw byte run in `lmu.voc`
  (0 hits) — the full word set lives in the **packed (Patricia)
  B-tree**, whose decode is the next increment.
- **`tsearch` now hits real stems**: «раст» → 279 docs, «зват» → 294
  docs (was 0). «п30» → 310 (was 2670 — the proportional estimate now
  divides by the full 1002-word dictionary, not 78). «конституція» →
  0 (packed B-tree + native-ID → current docnums join — next on the
  roadmap). 86 unit tests.

---

## v0.14.0 — 2026-10-26

### UK
- **Trident: ПОВНА таблиця відповідності знаків**
  (`src/trident.rs::LM_CHAR_TABLE`) — тепер відомий повний
  cp1251-порядок слотів LmCode, підтверджений трима незалежними
  джерелами:
  1. `windows.kod` кодує cp1251-байти → слоти: `c0–df → 80–9f`
     (А–Я), `e0–ef → a0–af` (а–п), `f0–ff → e0–ef` (р–я),
     додаткові `a5/b4/aa/ba/b2/b3/af/bf → f2–f9`;
  2. DLL `lmsup32.dll` містить QWERTY-алфавітну таблицю, що
     розшифровується в ті самі слоти;
  3. реальні слова `lmu.voc` («нема», «бути», «Мин») читаються
     коректним українським **лише** в цьому порядку.
  Повний діапазон: великі 0x80–0x9F, малі 0xA0–0xAF і 0xE0–0xEF,
  українські 0xF2–0xF9 (Ґ/ґ=0xF2/0xF3, Є/є=0xF4/0xF5, І/і=0xF6/0xF7,
  Ї/ї=0xF8/0xF9). Попередні «якорі» з секції `G:` lm.a
  (А=0x84, О=0x96, У=0x9C, Щ=0x9F) виявилися помилковим
  прочитанням міжмовних якорів; цифри та Latin — ASCII.
- **`tsearch` показує кодування + розшифровку**: для кожного
  терміну `код: «термін» → [hex-байти] → «розшифрування»`.
  «конституція» тепер кодується на **100%** (було ~45%):
  `[aa ae ad e1 e2 a8 e2 e3 e6 f7 ef]` → «конституція» (повний
  раунд-трип). `п30` → 2670 Trident-документів (точний збіг з
  попереднім, але тепер без хибного с=0xAF); `с3` → 0 (власне слово).
- Дода `lm_byte_to_char` / `decode_lm_word` (зворотний декодер) і
  `TridentIndex::encode_query` (терміни → LmCode-байти).
- Версія **0.13.0 → 0.14.0** (єдине джерело: `CARGO_PKG_VERSION`).
- 84 юніт-тести (+нові: `char_map_is_the_full_cp1251_correspondence`,
  `lm_byte_to_char_round_trips`, `decode_lm_word_reproduces_real_
  dictionary_words`; оновлено таблицю знаків і покриття).

### EN
- **Trident: FULL character correspondence**
  (`src/trident.rs::LM_CHAR_TABLE`) — the complete cp1251-based
  LmCode slot order is now known, confirmed by three independent
  sources:
  1. `windows.kod` maps cp1251 bytes → slots: `c0–df → 80–9f`
     (А–Я), `e0–ef → a0–af` (а–п), `f0–ff → e0–ef` (р–я), extras
     `a5/b4/aa/ba/b2/b3/af/bf → f2–f9`;
  2. the `lmsup32.dll` QWERTY alphabet table decodes to the same
     slots;
  3. the real `lmu.voc` words («нема», «бути», «Мин») read as
     correct Ukrainian **only** under this order.
  Full range: upper 0x80–0x9F, lower 0xA0–0xAF and 0xE0–0xEF,
  Ukrainian 0xF2–0xF9 (Ґ/ґ=0xF2/0xF3, Є/є=0xF4/0xF5, І/і=0xF6/0xF7,
  Ї/ї=0xF8/0xF9). The earlier `G:`-section "anchors" (А=0x84,
  О=0x96, У=0x9C, Щ=0x9F) were a misread of the cross-language
  anchors; digits and Latin are ASCII.
- **`tsearch` now shows the encoding + decode**: per term
  `code: «term» → [hex bytes] → «decoding»`. «конституція» now
  encodes at **100%** (was ~45%):
  `[aa ae ad e1 e2 a8 e2 e3 e6 f7 ef]` → «конституція» (full
  round-trip). `п30` → 2670 Trident docs (exact match, now without
  the spurious с=0xAF); `с3` → 0 (its own word).
- Added `lm_byte_to_char` / `decode_lm_word` (the reverse decoder)
  and `TridentIndex::encode_query` (terms → LmCode bytes).
- Version **0.13.0 → 0.14.0** (single source: `CARGO_PKG_VERSION`).
- 84 unit tests (+ new: `char_map_is_the_full_cp1251_correspondence`,
  `lm_byte_to_char_round_trips`, `decode_lm_word_reproduces_real_
  dictionary_words`; char table + coverage tests updated).

### Roadmap (v0.15.0)
- Морфологія `.g`/`.gw` (граф/ваги), B-tree-словник (повний
  перелік слів, не лише 78 записів-референтів) і зв'язка
  native-ID → номери документів поточної бази (оракел «конституція»
  = 223 хіти). / The `.g`/`.gw` morphology graphs, the full B-tree
  dictionary (not just the 78 reference records) and the
  native-ID → current docnums join (the «конституція» oracle = 223
  hits).

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
