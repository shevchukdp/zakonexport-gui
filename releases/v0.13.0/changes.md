# v0.13.0 - Changes / Зміни (2026-10-26)

## UK / Українська

- **Trident: експериментальний нативний рушій** (`src/trident.rs`) —
  декод частин нативного FTS-індексу старого застосунку в папці
  `SOFT\` бази:
  - `windows.kod` (683 Б): кодова таблиця, 80 рядків
    `<2hex-code> <sep> <byte>` — значення ≥0x80 записано сирийм
    байтом (іноді з пробілом перед CRLF), <0x80 — двома ASCII-hex
    цифрами; сепаратори `=` (мапінг) і `>` (fold/еквівалентність);
  - `lm.a` (904 Б): case-fold таблиці, текстові секції:
    `E:` — Latin identity (`A a` … `Z z`); `G:` — міжмовні якорі
    (A a ↔ `84 a4` = А/а, O o ↔ `96 e6` = О/о, U u ↔ `9c ec` = У/у,
    окремі пари `9f ef` = Щ/щ); `R:` — 32 fold-пари (рос.),
    `U:` — 32 fold-пари (укр.; головний діапазон 0x80–0x9F
    пропускає 9a/9b/9d, додаткові пари `f4 f5`, `f6 f7`, `f8 f9`);
  - записи словника `[0x84][word][00 00]` (lmu.voc — 78 записів
    після дедуплікації) і u16 posting-потік `.vt` (419540 u16;
    ідентифікатори документів 0..65535 — native IDs 2005-індексу,
    не збігаються з номерами поточної бази).
- **`tsearch <query>`** (CLI, позначено як експеримент):
  - терміни запиту кодуються через LmCode-таблицю знаків:
    підтверджені якорі (А/а=0x84/A4, О/о=0x96/E6, У/у=0x9C/EC,
    Щ/щ=0x9F/EF) + цифри (ASCII) + Latin (ASCII); виведені за
    частотою у словнику (кандидати, підтвердження — v0.14.0):
    с/С=0xAF, і/ї/І/Ї/и/И=0xAD;
  - кодовані терміни збігаються зі словником lmu.voc
    (префіксне зіставлення + стем-обрізки 1–3 хвостики), для
    збігів читаються пропорційні posting-скліпчасті з lmur.vt;
  - вивід: покриття (% закодованих знаків), список збігів,
    native Trident-ідентифікатори документів (перші 40 + «ще N»).
  - Зв'язка native-ID з номерами документів поточної бази —
    v0.14.0 (разом з морфологією .g/.gw і точними
    word→postings кордонами).
- **Команда `trident` розширена** — інвентар 16 файлів
  (lmu.voc, lmr.voc, lmur.vt, lmru.vt, lmur.m, lmru.m, lmu.g,
  lmr.g, lmu.gw, lmr.gw, lmsup.dll, lmsup32.dll + нові
  windows.kod, lm.a, lmu.use, lmr.use) з розмірами + декодована
  статистика: `windows.kod: 80 кодів · lm.a: fold-пари R=32 U=32 ·
  словник lmu.voc: 78 записів · vt-потік: 419540 u16
  (документи 0..65535)`.
- **GUI**: чекбокс «Trident-рушій (нативний індекс, експеримент)»
  (нижній правий блок, під «підсвітити слова запиту»; висота
  вікна 780 → 804). За позначення прапорця пошук іде через
  нативний Trident-індекс: рядки показують native IDs (стовпець
  файлу «trident (native id)», реквізитні стовпці — даші, бо
  зв'язка номерів ще не зроблена); 0 збігів → статус з покриттям
  і розміром словника; немає `SOFT\` → повідомлення
  «зніміть прапорець». Чекбокс вимкнено за замовчуванням →
  звичайний fold-codec FTS-пошук, GUI-самотест `--guitest`
  незмінний.
- Версія **0.12.0 → 0.13.0** (єдине джерело: `CARGO_PKG_VERSION`).
- 82 юніт-тести (нові: `trident::tests` ×11 — kod raw/hex,
  формат реальної таблиці, fold-розбір рядків, розділи lm.a,
  таблиця знаків, покриття encode_term, словник/vt, префіксний
  пошук з обрізками + інтеграційний тест на реальному індексі).

## EN / English

- **Trident: experimental native engine** (`src/trident.rs`) —
  decodes parts of the legacy app's native FTS index in the base's
  `SOFT\` folder:
  - `windows.kod` (683 B): the code table, 80 lines
    `<2hex-code> <sep> <byte>` — values ≥0x80 stored as a raw
    byte (some lines have a trailing space before CRLF), <0x80 as
    two ASCII-hex digits; separators `=` (mapping) and `>`
    (fold/equivalence);
  - `lm.a` (904 B): case-fold tables, text sections: `E:` — Latin
    identity (`A a` … `Z z`); `G:` — cross-language anchors
    (A a ↔ `84 a4` = А/а, O o ↔ `96 e6` = О/о, U u ↔ `9c ec`
    = У/у, standalone pair `9f ef` = Щ/щ); `R:` — 32 fold pairs
    (Russian), `U:` — 32 fold pairs (Ukrainian; the main
    0x80–0x9F range skips 9a/9b/9d, extra pairs `f4 f5`,
    `f6 f7`, `f8 f9`);
  - dictionary records `[0x84][word][00 00]` (lmu.voc — 78
    records after de-duplication) and the u16 posting stream
    from `.vt` (419540 u16; doc ids 0..65535 — native ids of the
    2005-era index, not the current base's docnums).
- **`tsearch <query>`** (CLI, labelled experimental):
  - query terms are encoded through the LmCode character table:
    confirmed anchors (А/а=0x84/A4, О/о=0x96/E6, У/у=0x9C/EC,
    Щ/щ=0x9F/EF) + digits (ASCII) + Latin (ASCII); inferred from
    dictionary frequency (candidates, to be confirmed in
    v0.14.0): с/С=0xAF, і/ї/І/Ї/и/И=0xAD;
  - encoded terms are matched against the lmu.voc dictionary
    (prefix-style + stem trimming, 1–3 tails); for a hit a
    proportional posting slice is read from lmur.vt;
  - output: coverage % (encodable query characters), matched
    terms, native Trident document ids (first 40 + "… N more").
  - Joining the native ids to the current base's docnums —
    v0.14.0 (with the .g/.gw morphology and the exact
    word→postings boundaries).
- **Extended `trident` verb** — a 16-file inventory (lmu.voc,
  lmr.voc, lmur.vt, lmru.vt, lmur.m, lmru.m, lmu.g, lmr.g,
  lmu.gw, lmr.gw, lmsup.dll, lmsup32.dll + the new windows.kod,
  lm.a, lmu.use, lmr.use) with sizes + decoded stats:
  `windows.kod: 80 codes · lm.a: fold pairs R=32 U=32 · dict
  lmu.voc: 78 records · vt stream: 419540 u16 (docs 0..65535)`.
- **GUI**: a "Trident-рушій (нативний індекс, експеримент)"
  checkbox (bottom-right block, below the highlight checkbox;
  window height 780 → 804). When checked, search runs through
  the native Trident index: rows show the native ids (file
  column "trident (native id)", field columns dashed until the
  docnum join lands); 0 hits → a status line with the coverage
  and the dictionary size; no `SOFT\` → "uncheck the flag"
  message. Unchecked by default → the classic fold-codec FTS
  search, the `--guitest` self-test is unchanged.
- Version **0.12.0 → 0.13.0** (single source: `CARGO_PKG_VERSION`).
- 82 unit tests (new: `trident::tests` ×11 — kod raw/hex, the
  real-table shape, line-based fold parsing, lm.a sections, the
  char table, encode_term coverage, dict/vt, prefix search with
  trimming + a real-index integration test).
