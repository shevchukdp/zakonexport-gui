# v0.15.0 - Changes / Зміни (2026-10-27)

## UK / Українська

- **Trident: ЗАГАЛЬНИЙ формат записів словника**
  (`src/trident.rs::extract_dict_words`).
  - Формат запису — `[tag][word NUL][flag]` (tag 0x84/0x8d,
    flag 0x00/0x01), а не лише суворий `[0x84][word][00 00]`.
    Тег `0x8d` та прапорець `0x01` раніше впускалися.
  - Вилучення словника з `lmu.voc` зросло з **78 до ~1002
    унікальних стем** (O(n)-скан, обмежений 12-байтовий пошук
    вперед, розподіл довжин: 1:87, 2:1889, 3:702, 4:178, 5:228,
    6:116, 7:16, 8:4).
  - Словник тепер містить **реальні українські стеми** —
    «зувати», «раст»/«ряст», «зват», «дити», «овувати» — разом із
    морфологічними клас-кодами (`зувати0`, `зувати1`, `титиь33`,
    `утик33`).
- **`.gw` — морфологічна таблиця ваг** (`read_gw_u16`).
  - `lmu.gw` (268 B = 134 u16) / `lmr.gw` (264 B = 132 u16) —
    плоский little-endian u16-масив **без заголовка**.
  - Бит `0x8000` — прапорець; ставиться на останніх записах
    (хвіст `8053 8054 8057 8058`).
  - Завантажується в `TridentIndex::gw_u16` (структурне
    декодування). Повний семантичний декод потребує графу `.g`
    (заголовок «Trident Software, Inc. / grammar / UTEMPLATE.TXT»,
    пак-дані з 0x60) — далі в roadmap.
- **Б-дерево / word-структури задокументовано** (з `lmsup32.dll`):
  - пошук слова `FUN_10021520` = лінійний скан **NULL-термінованого
    масиву вказівників на word-структури** (база
    `*(int**)(&DAT_1002db7a + DAT_1002db27*0x4f)`);
  - word-байти NUL-терміновані за **offset +1** у структурі
    (структура ≥0xe1 байт; поля: +0xc0 ptr, +0xd5/+0xd7 flag-байти
    (бит 0x80), +0xdd ptr);
  - порівняння — `FUN_1001bca0` (case-fold: кожен байт через
    `FUN_10011930(x,0x80)`).
  - «конституція» (`aa ae ad e1 …`) **немає** як сирого байт-рядка
    в `lmu.voc` (0 збігів) — повний набір слів у **щільному
    (Patricia-) B-дереві** з узагальненням префіксів; декод —
    наступний крок.
- **`tsearch` влучає реальні стеми**:
  - «раст» → **279** документів (раніше 0);
  - «зват» → **294** документів (раніше 0);
  - «п30» → **310** (було 2670 — пропорційна оцінка тепер
    ділиться на повний словник 1002 слова, а не 78);
  - «конституція» → **0** (щільне B-дерево + join native-ID →
    docnum поточної бази — далі в roadmap).
- Версія **0.14.0 → 0.15.0** (єдине джерело: `CARGO_PKG_VERSION`).
- **86 юніт-тестів** (нові: `extract_dict_words_captures_tag_and_
  flag_variants`, `read_gw_u16_reads_le_values`; оновлено
  `real_index_loads` (словник ≥500 слів, `.gw` присутній) і
  `dict_words_are_sorted_and_deduped`).

### Roadmap (v0.16.0)
- Повний щільний B-tree-словник (Patricia-trie, у тому числі
  «конституція»), точні кордони word→postings, `.g` граф
  морфології та зв'язка native-ID → номери документів поточної
  бази (оракел «конституція» = 223 хіти).

## EN / English

- **Trident: GENERAL dictionary record format**
  (`src/trident.rs::extract_dict_words`).
  - The record format is `[tag][word NUL][flag]` (tag 0x84/0x8d,
    flag 0x00/0x01), not only the strict `[0x84][word][00 00]`.
    The `0x8d` tag and the `0x01` flag were previously missed.
  - Extraction from `lmu.voc` grew from **78 to ~1002 unique
    stems** (O(n) scan, bounded 12-byte forward look; length
    distribution 1:87, 2:1889, 3:702, 4:178, 5:228, 6:116, 7:16,
    8:4).
  - The dictionary now holds **real Ukrainian stems** — «зувати»,
    «раст»/«ряст», «зват», «дити», «овувати» — together with
    morphological class-codes (`зувати0`, `зувати1`, `титиь33`,
    `утик33`).
- **`.gw` — morphology weight table** (`read_gw_u16`).
  - `lmu.gw` (268 B = 134 u16) / `lmr.gw` (264 B = 132 u16) — a
    flat little-endian u16 array **with no header**.
  - The `0x8000` bit is a flag; set on the last entries (tail
    `8053 8054 8057 8058`).
  - Loaded into `TridentIndex::gw_u16` (structural decode). The
    full semantic decode needs the `.g` grammar graph (header
    «Trident Software, Inc. / grammar / UTEMPLATE.TXT», packed data
    from 0x60) — roadmap.
- **B-tree / word-struct layout documented** (from `lmsup32.dll`):
  - word lookup `FUN_10021520` = linear scan of a **NULL-terminated
    array of word-struct pointers** (base
    `*(int**)(&DAT_1002db7a + DAT_1002db27*0x4f)`);
  - word bytes are NUL-terminated at **offset +1** in the struct
    (struct ≥0xe1 bytes; fields: +0xc0 ptr, +0xd5/+0xd7 flag bytes
    (0x80 bit), +0xdd ptr);
  - comparison — `FUN_1001bca0` (case-fold: each byte through
    `FUN_10011930(x,0x80)`).
  - «конституція» (`aa ae ad e1 …`) is **absent** as a raw byte run
    in `lmu.voc` (0 hits) — the full word set lives in the
    **packed (Patricia) B-tree** with prefix sharing; decode is the
    next increment.
- **`tsearch` now hits real stems**:
  - «раст» → **279** docs (was 0);
  - «зват» → **294** docs (was 0);
  - «п30» → **310** (was 2670 — the proportional estimate now
    divides by the full 1002-word dictionary, not 78);
  - «конституція» → **0** (packed B-tree + native-ID → current
    docnums join — next on the roadmap).
- Version **0.14.0 → 0.15.0** (single source: `CARGO_PKG_VERSION`).
- **86 unit tests** (new: `extract_dict_words_captures_tag_and_
  flag_variants`, `read_gw_u16_reads_le_values`; updated
  `real_index_loads` (dict ≥500 words, `.gw` present) and
  `dict_words_are_sorted_and_deduped`).

### Roadmap (v0.16.0)
- The full packed B-tree dictionary (Patricia-trie, including
  «конституція»), the exact word→postings boundaries, the `.g`
  morphology graph, and the native-ID → current docnums join (the
  «конституція» oracle = 223 hits).
