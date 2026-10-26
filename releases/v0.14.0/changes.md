# v0.14.0 - Changes / Зміни (2026-10-26)

## UK / Українська

- **Trident: ПОВНА таблиця відповідності знаків**
  (`src/trident.rs::LM_CHAR_TABLE`).
  - Встановлено повний **cp1251-порядок** слотів LmCode для всіх
    32 великих (0x80–0x9F, А–Я), 32 малих (0xA0–0xAF а–п,
    0xE0–0xEF р–я) та 8 українських (0xF2–0xF9: Ґ/ґ, Є/є, І/і,
    Ї/ї). Підтверджено трьома незалежними джерелами:
    1. `windows.kod` (cp1251-байт → слот): `c0–df → 80–9f`,
       `e0–ef → a0–af`, `f0–ff → e0–ef`, додаткові
       `a5/b4/aa/ba/b2/b3/af/bf → f2–f9`;
    2. QWERTY-алфавітна таблиця `lmsup32.dll` (філ. офсет
       0x2a3f0 / 0x2a3b0) розшифровується в ті самі слоти;
    3. реальні слова `lmu.voc` («нема» = `ad a5 ac a0`,
       «бути» = `a1 e3 e2 a8`, «Мин») читаються коректним
       українським лише в цьому порядку.
  - Попередні «якорі» з секції `G:` lm.a (А=0x84, О=0x96,
    У=0x9C, Щ=0x9F) виявилися **помилковим прочитанням**: це
    міжмовні якорі (латинська літера → LmCode-слот), а не
    власний порядок. Цифри та Latin — ASCII.
- **`tsearch` показує кодування + розшифровку**: рядок
  `код: «термін» → [hex-байти] → «розшифрування»` для кожного
  терміну запиту.
  - «конституція» тепер кодується на **100%** (було ~45%):
    `[aa ae ad e1 e2 a8 e2 e3 e6 f7 ef]` → «конституція»
    (повний раунд-трип);
  - `п30` → 2670 Trident-документів (тепер без хибного с=0xAF —
    0xAF це «п», не «с»); `с3` → 0 (власне слово).
- **Нові функції**: `lm_byte_to_char` (слот → знак),
  `decode_lm_word` (LmCode-слово → текст) та
  `TridentIndex::encode_query` (терміни → LmCode-байти).
- Версія **0.13.0 → 0.14.0** (єдине джерело: `CARGO_PKG_VERSION`).
- **84 юніт-тести** (нові: `char_map_is_the_full_cp1251_correspondence`,
  `lm_byte_to_char_round_trips`, `decode_lm_word_reproduces_real_
  dictionary_words`; оновлено таблицю знаків і покриття).

### Roadmap (v0.15.0)
- Морфологія `.g`/`.gw` (граф/ваги), повний B-tree-словник
  (не лише 78 записів-референтів) і зв'язка native-ID → номери
  документів поточної бази (оракел «конституція» = 223 хіти).

## EN / English

- **Trident: FULL character correspondence**
  (`src/trident.rs::LM_CHAR_TABLE`).
  - Established the complete **cp1251-based** LmCode slot order for
    all 32 upper (0x80–0x9F, А–Я), 32 lower (0xA0–0xAF а–п,
    0xE0–0xEF р–я) and 8 Ukrainian (0xF2–0xF9: Ґ/ґ, Є/є, І/і,
    Ї/ї) slots. Confirmed by three independent sources:
    1. `windows.kod` (cp1251 byte → slot): `c0–df → 80–9f`,
       `e0–ef → a0–af`, `f0–ff → e0–ef`, extras
       `a5/b4/aa/ba/b2/b3/af/bf → f2–f9`;
    2. the `lmsup32.dll` QWERTY alphabet table (file offsets
       0x2a3f0 / 0x2a3b0) decodes to the same slots;
    3. the real `lmu.voc` words («нема» = `ad a5 ac a0`,
       «бути» = `a1 e3 e2 a8`, «Мин») read as correct Ukrainian
       only under this order.
  - The earlier `G:`-section "anchors" (А=0x84, О=0x96, У=0x9C,
    Щ=0x9F) were a **misread**: those are cross-language anchors
    (Latin letter → LmCode slot), not the slot order itself.
    Digits and Latin are ASCII.
- **`tsearch` shows the encoding + decode**: a
  `code: «term» → [hex bytes] → «decoding»` line for each query
  term.
  - «конституція» now encodes at **100%** (was ~45%):
    `[aa ae ad e1 e2 a8 e2 e3 e6 f7 ef]` → «конституція» (full
    round-trip);
  - `п30` → 2670 Trident docs (now without the spurious с=0xAF —
    0xAF is «п», not «с»); `с3` → 0 (its own word).
- **New functions**: `lm_byte_to_char` (slot → char),
  `decode_lm_word` (LmCode word → text) and
  `TridentIndex::encode_query` (terms → LmCode bytes).
- Version **0.13.0 → 0.14.0** (single source: `CARGO_PKG_VERSION`).
- **84 unit tests** (new: `char_map_is_the_full_cp1251_correspondence`,
  `lm_byte_to_char_round_trips`, `decode_lm_word_reproduces_real_
  dictionary_words`; char table + coverage tests updated).

### Roadmap (v0.15.0)
- The `.g`/`.gw` morphology graphs, the full B-tree dictionary
  (not just the 78 reference records) and the native-ID → current
  docnums join (the «конституція» oracle = 223 hits).
