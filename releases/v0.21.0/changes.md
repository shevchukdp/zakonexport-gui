# v0.21.0 - Changes / Зміни (2026-10-28)

## UK / Українська

- **Повний заголовок `.m` декодовано — u32-таблиця має 16 слотів, а не
  8** (v0.19.0 читав лише першу половину). Емпіричні дампи
  `lmur.m`/`lmru.m` зафіксували ролі слотів:
  - **slot 5 = u16 predicate-таблиця** (`ctx+0x44`, яку ходить
    `FUN_100176c0`/`FUN_10015cd0`): 0x206D ukr / 0x1ACF rus;
  - **slot 11 = u16 word-таблиця** (`ctx+0x30`): 0x37CB ukr / 0x3001 rus.
  Висновок (розблоковує blocked item #1 старого плану): u16
  word/predicate-таблиці лежать **у `.m`**, а не в порожньому хвості
  `.vt` (v0.20.0) і не виводяться з B-tree `.voc`.
- **Tail N-секція `.m` НЕ порожня** (на відміну від `.vt`): `VmHeader`
  тепер парсить її вбудовано (`VtTailScan`):
  - `lmur.m` @0x70E: **N=3** — `"R"→[0x1]`, `"N*"→[0x1C,0x3E]`,
    `"*"→[0x3E,0x14]`;
  - `lmru.m` @0x53E: **N=4** — додається `"*N"→[0x24,0x0E,0x26,0x1D,0x3E]`.
  Це wildcard-ключі, які `tail_match` (v0.20.0) завершують у повний
  word → u16 word-index join (v0.22+).
- **Міжмовний інваріант заголовка зафіксовано**: `u32@0x84 = pred_off −
  7` (0x2066+7=0x206D; 0x1AC8+7=0x1ACF) та `u32@0x80 = pred_off − 19`
  (різниця полів = 12). Семантика 0xE-структур потоку @0x8C — далі
  (v0.22+).
- **U16 word-таблиця структурно ідентифікована**: `[u16 count]` +
  `count` відносних u16 офсетів, рівно заповнюють секцію до вказівника
  slot 12 (ukr: count=0xFE=254, 0x37CB..0x39C9; rus: count=0x100=256,
  0x3001..0x3203). Predicate-таблиця — чистий уростальний u16-масив
  (ukr @0x206D: 0x013E 0x0154 0x0164 0x0170 …).
- **Новий CLI sub-verb `trident dump <файл> [офс] [довжина]`** —
  console-only hex-dump (16 байтів/рядок + latin1; `<файл>` в
  `SOFT\`, потім корінь бази; офси/довжини у hex або dec).
- **`.use`/`.uvt` задокументовано (факти з `lmsup32.dll`)**:
  - `lmC.use` відкривається **в write-режимі**
    (`FUN_10011b20(...,0x40,0,'W')`; fallback
    `FUN_10011e10(...,0x40,0xCC,'W')` — саме 204-байтова форма) →
    `.use` = write-back cache (у read-only базі — нульовий);
  - `.uvt` відкривається лише `'W'` (`.vt` — `'R'`) → update-cache
    зарезервовано й не писалося.
- **CLI `trident`** — оновлені блоки `lmur.m`/`lmru.m` (16-slot
  таблиця + slot5/slot11 + u80/u84 + tail N-секція) та tail-scan рядок
  (u16 таблиці виявлено в `.m`).
- **132 юніт-тестів** (нові: 16-slot таблиця + tail N-секції для обох
  мов, u80/u84-інваріанти, count+offsets shape, ascending
  predicate-масив). Guitest baseline `rows=430` без змін.

## EN / English

- **The full `.m` header decoded — the u32 table has 16 slots, not 8**
  (the v0.19.0 parse was only the first half). The empirical dumps of
  `lmur.m`/`lmru.m` pin the slot roles:
  - **slot 5 = the u16 predicate table** (`ctx+0x44`, the table
    `FUN_100176c0`/`FUN_10015cd0` walks): 0x206D ukr / 0x1ACF rus;
  - **slot 11 = the u16 word table** (`ctx+0x30`): 0x37CB ukr /
    0x3001 rus.
  Conclusion (unblocks the old blocked item #1): the u16
  word/predicate tables live **in the `.m`**, not the empty `.vt`
  tail (v0.20.0) and are not derived from the `.voc` B-tree.
- **The `.m` tail N-section is NOT empty** (unlike the `.vt`):
  `VmHeader` now parses it inline (`VtTailScan`):
  - `lmur.m` @0x70E: **N=3** — `"R"→[0x1]`, `"N*"→[0x1C,0x3E]`,
    `"*"→[0x3E,0x14]`;
  - `lmru.m` @0x53E: **N=4** — the extra record
    `"*N"→[0x24,0x0E,0x26,0x1D,0x3E]`.
  These are the wildcard keys `tail_match` (v0.20.0) completes into
  the full word → u16 word-index join (v0.22+).
- **The cross-language header invariant pinned**: `u32@0x84 =
  pred_off − 7` (0x2066+7=0x206D; 0x1AC8+7=0x1ACF) and
  `u32@0x80 = pred_off − 19` (the fields differ by 12). The 0xE-
  structure stream semantics (@0x8C) come next (v0.22+).
- **The u16 word table structurally identified**: a `[u16 count]` +
  `count` relative u16 offset array that exactly fills the section up
  to the slot-12 pointer (ukr: count=0xFE=254, 0x37CB..0x39C9; rus:
  count=0x100=256, 0x3001..0x3203). The predicate table is a plain
  ascending u16 array (ukr @0x206D: 0x013E 0x0154 0x0164 0x0170 …).
- **New CLI sub-verb `trident dump <file> [off] [len]`** — a
  console-only hex dump (16 bytes/line + latin1; `<file>` resolves
  against `SOFT\`, then the base root; offsets/lengths in hex or
  decimal).
- **`.use`/`.uvt` documented (RE facts from `lmsup32.dll`)**:
  - `lmC.use` is opened **in write mode**
    (`FUN_10011b20(...,0x40,0,'W')`; fallback
    `FUN_10011e10(...,0x40,0xCC,'W')` — the exact 204-byte form) →
    `.use` is a write-back cache (zero in this read-only base);
  - `.uvt` opens `'W'` only (`.vt` opens `'R'`) → the update cache is
    reserved and was never written.
- **CLI `trident`** — updated `lmur.m`/`lmru.m` blocks (the 16-slot
  table + slot5/slot11 + u80/u84 + the tail N-section) and the
  tail-scan line (the u16 tables are in the `.m`).
- **132 unit tests** (new: the 16-slot table + the tail N-sections
  for both languages, the u80/u84 invariants, the count+offsets
  shape, the ascending predicate array). The guitest baseline
  `rows=430` is unchanged.
