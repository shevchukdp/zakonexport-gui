# v0.27.0 - Changes / Зміни (2026-10-28)

## UK / Українська

- **Каталог предикатів `FUN_100177d0` (roadmap v0.28.0).** 22-опкодова
  предикатна таблиця DLL як UA+EN референс + оцінювана підмножина.
  `trident_vt.rs`:
  - `PredOp` (22 варіанти, названі за hex-опкодом), `PredRow`
    (in-memory 224+B-рядок: `+0x51`/`+0xA4`/`+0xD7`/`+0xD8`/`+0xD9`/
    `+0xDD`/`+200` + proxy-поля), `PredCatalogEntry` + `PRED_CATALOG`
    (опкод → семантика UA+EN + evaluable-прапорець),
    `pred_from_opcode` / `pred_catalog_size` / `pred_evaluable_count`,
    `pred_flag_bits` (low/high-flag біти, які OR-ить кожна рука).
  - **`pred_evaluate(op, pool, idx) -> bool`** — оцінювана підмножина
    (20 з 22): flag-сетаери (2/8/0xC/0xD/0x10/0x16/0x1B — завжди
    true), field-checks (5/6/7/0xB/0x14/0x17/0x19/0x1D/0x23/0x24),
    row-walks (0x18 / 0x1C / 0x1E — структурні апроксимації). Руки
    0/1 (`FUN_10011600` / `FUN_10011580`) — нерозв'язані accessori,
    повертають `false`.
- **Семантика (декомпіляція)**: `2→low|=0xC`, `0xC→low|=0x28`,
  `8→low|=2`, `0xD→low|=0x40`, `0x16→high|=4`, `0x19→0xD7&2`,
  `0x1D→!(0xD7&2)`, `0x10→high|=2`, `6→H[+8]==1`, `0x23→
  prev[0xD9]<cur[0xD9]`, `0x24→+200==0`, `7→!(0xD7&8)`,
  `0xB→!(0xD7&0x10)`, `0x14→0xD8&1`, `0x17→+0x51!=0`,
  `0x1C→next-chain`, `0x1E→+0xA4→*[+0xA4]+0x4C!=0`, `0x1B→high|=8`,
  `5→next[0xD9]-cur[0xD9]==1`.
- **CLI `trident`**: новий рядок `predicate-каталог (v0.27.0,
  FUN_100177d0): 22 opcodes · evaluable 20 · unresolved-accessors 2`.
- **159 юніт-тестів** (нові 7). Guitest baseline `rows=430` без змін.
  Версія **0.26.0 → 0.27.0**.
- Далі (v0.28.0 / roadmap v0.29.0): B-tree byte-VM core `FUN_100019f0`.

## EN / English

- **The `FUN_100177d0` predicate catalog (roadmap v0.28.0).** The DLL's
  22-opcode predicate table as a UA+EN reference + the evaluable
  subset. `trident_vt.rs`:
  - `PredOp` (22 variants, named by the hex opcode), `PredRow` (the
    in-memory 224+ B row: `+0x51`/`+0xA4`/`+0xD7`/`+0xD8`/`+0xD9`/
    `+0xDD`/`+200` + proxy fields), `PredCatalogEntry` +
    `PRED_CATALOG` (opcode → semantics, UA+EN, + the evaluable flag),
    `pred_from_opcode` / `pred_catalog_size` / `pred_evaluable_count`,
    `pred_flag_bits` (the low/high flag bits each arm ORs).
  - **`pred_evaluate(op, pool, idx) -> bool`** — the evaluable subset
    (20 of 22): the flag setters (always true), the field checks, and
    the row walks (structural approximations). The two arms 0/1
    (`FUN_10011600` / `FUN_10011580`) stay unresolved accessors and
    return `false`.
- **Semantics (decompiled)**: `2→low|=0xC`, `0xC→low|=0x28`,
  `8→low|=2`, `0xD→low|=0x40`, `0x16→high|=4`, `0x19→0xD7&2`,
  `0x1D→!(0xD7&2)`, `0x10→high|=2`, `6→H[+8]==1`, `0x23→
  prev[0xD9]<cur[0xD9]`, `0x24→+200==0`, `7→!(0xD7&8)`,
  `0xB→!(0xD7&0x10)`, `0x14→0xD8&1`, `0x17→+0x51!=0`,
  `0x1C→next-chain`, `0x1E→+0xA4→*[+0xA4]+0x4C!=0`, `0x1B→high|=8`,
  `5→next[0xD9]-cur[0xD9]==1`.
- **CLI `trident`**: a new `predicate-каталог (v0.27.0, FUN_100177d0):
  22 opcodes · evaluable 20 · unresolved-accessors 2` line.
- **159 unit tests** (7 new). The guitest baseline `rows=430` is
  unchanged. Version **0.26.0 → 0.27.0**.
- Next (v0.28.0 / roadmap v0.29.0): the B-tree byte-VM core
  `FUN_100019f0` (the key scan + the stops).
