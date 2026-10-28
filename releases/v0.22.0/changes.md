# v0.22.0 - Changes / Зміни (2026-10-28)

## UK / Українська

- **U16 word-таблиця структурно декодована** (нова `VmWordTable`,
  `.m` slot 11, 0x37CB ukr / 0x3001 rus): секція =
  `[u16 count][count u16 records @base+2]`. Count = кількість слів
  словника (254 ukr / 256 rus). Перша половина (126/128) — малі
  уростальні offset-значення (step 2), друга — i16-від'ємні (high
  byte 0xFF) з **kind-маркерами 0xFFFA (61 ukr / 64 rus)** та
  boundary-записом **0x8000 ukr / 0x7F00 rus**.
- **U16 predicate-таблиця структурно декодована** (нова
  `VmPredicateTable`, `.m` slot 5, 0x206D ukr / 0x1ACF rus):
  секція = `[u16 offsets × 160/168][sentinel 0xCECA][packed
  u8-op/u16-arg records]`. Offset-run уростає й завершується
  **sentinel 0xCECA в обох мовах** (міжмовний інваріант). Після
  sentinel — packed-рекорди (перший байт ukr = 0x16), які ходить
  `FUN_10015cd0`.
- **`TridentIndex`** несе 4 таблиці: `word_table`, `pred_table`,
  `rus_word_table`, `rus_pred_table`.
- **CLI `trident`** — fingerprint-рядки:
  - `u16 word-таблиця @0x37cb (v0.22.0): 254 records — 126
    offset-like (останній 0x1fa) + 128 i16 · 0xFFFA=61 ·
    boundary=0x8000`;
  - `u16 pred-таблиця @0x206d (v0.22.0): 160 offset-ів
    (0x13e..0x2316) + sentinel 0xceca · records @хвіст: 16 cb 1a cb
    21 ca 22 cc …`;
  - + rus-аналоги (256 records / 0xFFFA=64 / boundary=0x7f00;
    168 offset-ів).
- **137 юніт-тестів** (нові: реальні word/pred-таблиці обох мов —
  перші 8 offset-ів, kind-розподіли, sentinel 0xCECA,
  truncation). Guitest baseline `rows=430` без змін.

## EN / English

- **The u16 word table structurally decoded** (new `VmWordTable`,
  `.m` slot 11, 0x37CB ukr / 0x3001 rus): the section is
  `[u16 count][count u16 records @base+2]`. The count is the
  dictionary word count (254 ukr / 256 rus). The first half
  (126/128) are small ascending offset-like values (step 2), the
  second half are i16-negative (high byte 0xFF) with the **0xFFFA
  kind markers (61 ukr / 64 rus)** and a **0x8000 ukr / 0x7F00 rus
  boundary record**.
- **The u16 predicate table structurally decoded** (new
  `VmPredicateTable`, `.m` slot 5, 0x206D ukr / 0x1ACF rus): the
  section is `[u16 offsets × 160/168][0xCECA sentinel][packed
  u8-op/u16-arg records]`. The offset run is ascending and ends at
  the **0xCECA sentinel in both languages** (a cross-language
  invariant). After the sentinel sit the packed records (ukr first
  byte = 0x16) that `FUN_10015cd0` walks.
- **`TridentIndex`** carries the four tables: `word_table`,
  `pred_table`, `rus_word_table`, `rus_pred_table`.
- **CLI `trident`** — fingerprint lines:
  - `u16 word-таблиця @0x37cb (v0.22.0): 254 records — 126
    offset-like (останній 0x1fa) + 128 i16 · 0xFFFA=61 ·
    boundary=0x8000`;
  - `u16 pred-таблиця @0x206d (v0.22.0): 160 offset-ів
    (0x13e..0x2316) + sentinel 0xceca · records @хвіст: 16 cb 1a cb
    21 ca 22 cc …`;
  - + the rus analogues (256 records / 0xFFFA=64 /
    boundary=0x7f00; 168 offsets).
- **137 unit tests** (new: the real word/predicate tables of both
  languages — the first 8 offsets, the kind distributions, the
  0xCECA sentinel, the truncation cases). The guitest baseline
  `rows=430` is unchanged.
