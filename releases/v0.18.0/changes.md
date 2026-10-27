# v0.18.0 - Changes / Зміни (2026-10-27)

## UK / Українська

- **Граф лем-граматики `lmu.g` структурно декодовано** (нова
  `src/g_graph.rs`; `TridentIndex.graph` + `TridentIndex.gw_u16`;
  CLI-дієслово `trident` показує граф):
  - заголовок: 0x00–0x17 вендор «Trident Software, Inc.» + NUL;
    0x18–0x1E «grammar»; 0x22 мовний символ ('U'/'R'); 0x24–0x2F
    12-байтовий slot шаблону («TEMPLATE.TXT» / «lmr.gt»);
  - u16-потік графа @0x60 (lmu.g: 31796 u16); хвіст — NUL-
    терміновані маркери **POINT1/POINT2/STEP/REPEAT** (всі чотири
    в обох `lmu.g`/`lmr.g`);
  - **`.gw` weight-стрим**: lmu.gw = 134 u16, лише 4 значення з
    прапором `0x8000` (термінатори); last-six
    `8053 8054 8057 8058 0000 0000`.
  - Семантика ребер (як у16-потік переходить у template-walk) —
    далі (v0.19.0).
- **Record-walker `.voc` — `FUN_10004230`** (розширення
  `src/trident_voc.rs`; `TridentIndex.morphology.walks`; новий
  блок CLI `trident` «записники (FUN_10004230, блок 512)»):
  - довжини записів по чотирьох режимах (розбирання
    `lmsup32.dll` Ghidra 12.1.4): `'n'` = `record_len_n`
    (по-байтовий скан до першого `0x00`; DLL рухає `short*` по 1
    байту — `(short *)((int)p + 1)`), `'f'` = `vm_record_len`
    (`FUN_100044a0`), `0xff` = `record_len_ffff`
    (`FUN_10004210`, u16-потік до `0xFFFF`, термінатор включено),
    `'e'` = `record_len_e` (`FUN_100041d0`, byte-VM: `J`+1,
    `F`/`G`/`C`/`E`+2, стоп `0xFF`);
  - `walk_section_records` — правило просування DLL: NUL після
    запису → `+1` (подвійний NUL), інакше **block-resync** до
    межі блоку (розмір блоку = `FUN_10021940()` = `DAT_1002ad98`,
    розмір буфера I/O-шару; точні лондінги незалежні від
    розміру блоку в межах 128..=4096, кандидат 512);
  - section→mode мапа (завантажувач `FUN_100032d0`): `+0x48`→'f',
    `+0x58`→ff, `+0x4C/0x50/0x54/0x60/0x64/0x68/0x6C/0x70`→'n',
    `+0x5C`→raw seek;
  - **підтверджені точні кордони** (lmu.voc, кожен лондінг =
    точний офсет *наступної* секції): f @`0x75253` (142) →
    `0x764C5`; ff @`0x764C5` (12) → `0x765B1`; афікси @`0x765B2`
    (7) → `0x765E1`; 33-слів @`0x765E1` (33) → `0x7667A`;
    +0x54 @`0x76691` (32) → `0x76714`; суфікси @`0x76714` (722)
    → `0x77B78`; listA (722) — усі 722 записів обходяться без
    втрат, listB (2157) — 819 записів і resync за кінець файлу
    (`0x78000`).
- Версія **0.17.0 → 0.18.0** (єдине джерело: `CARGO_PKG_VERSION`).
- **114 юніт-тестів** (нові: `g_graph::tests` ×6 +
  `trident_voc::tests` ×7, у т.ч. `walk_records_land_on_the_
  verified_section_boundaries` та
  `walk_landings_are_block_size_independent`); guitest baseline
  `rows=430` без змін.

### Roadmap (v0.19.0)
- Payload B-tree-вузлів listA/listB (join word→`.vt`-посилання):
  споживчий код пошуку в `lmsup32.dll` (без export-таблиці).
- Семантика ребер `.g`-графа (template-walk споживач u16-потіку).
- Блок-розмір `DAT_1002ad98` (кандидат 512; запис — у I/O-шарі,
  не в декомпіляції).

## EN / English

- **The `.g` grammar graph decoded structurally** (new
  `src/g_graph.rs`; `TridentIndex.graph` + `TridentIndex.gw_u16`;
  the `trident` CLI verb shows the graph):
  - the header: 0x00–0x17 vendor «Trident Software, Inc.» + NUL;
    0x18–0x1E «grammar»; 0x22 the language char ('U'/'R');
    0x24–0x2F the 12-byte template slot («TEMPLATE.TXT» /
    «lmr.gt»);
  - the graph u16 stream @0x60 (lmu.g: 31796 u16); the tail —
    NUL-terminated **POINT1/POINT2/STEP/REPEAT** markers (all
    four in both `lmu.g`/`lmr.g`);
  - **the `.gw` weight stream**: lmu.gw = 134 u16, only 4 values
    carry the `0x8000` flag (the terminators); last six
    `8053 8054 8057 8058 0000 0000`.
  - The edge semantics (how the u16 stream drives the template
    walk) — next (v0.19.0).
- **The `.voc` record walker — `FUN_10004230`** (extended
  `src/trident_voc.rs`; `TridentIndex.morphology.walks`; a new
  `trident` CLI «записники» block):
  - the per-mode record lengths (disassembled `lmsup32.dll`,
    Ghidra 12.1.4): `'n'` = `record_len_n` (byte scan to the
    first `0x00`; the DLL advances the `short*` one byte at a
    time — `(short *)((int)p + 1)`), `'f'` = `vm_record_len`
    (`FUN_100044a0`), `0xff` = `record_len_ffff` (`FUN_10004210`,
    u16 stream to `0xFFFF`, terminator included), `'e'` =
    `record_len_e` (`FUN_100041d0`, byte VM: `J`+1,
    `F`/`G`/`C`/`E`+2, stop at `0xFF`);
  - `walk_section_records` — the DLL's advance rule: a NUL after
    the record → `+1` (the double NUL), otherwise a **block
    resync** to the block boundary (the block size =
    `FUN_10021940()` = `DAT_1002ad98`, the I/O layer's buffer
    size; the verified landings are block-size-independent for
    128..=4096, 512 is the candidate);
  - the section→mode map (from the `FUN_100032d0` loader):
    `+0x48`→'f', `+0x58`→ff, `+0x4C/0x50/0x54/0x60/0x64/0x68/
    0x6C/0x70`→'n', `+0x5C`→raw seek;
  - **the verified exact boundaries** (lmu.voc, each landing =
    the *next* section's exact offset): f @`0x75253` (142) →
    `0x764C5`; ff @`0x764C5` (12) → `0x765B1`; affixes @
    `0x765B2` (7) → `0x765E1`; 33-word @`0x765E1` (33) →
    `0x7667A`; +0x54 @`0x76691` (32) → `0x76714`; suffixes @
    `0x76714` (722) → `0x77B78`; the listA (722) walk covers
    every record (no loss), the listB (2157) walk covers 819
    records and resynchronizes just past the file end
    (`0x78000`).
- Version **0.17.0 → 0.18.0** (single source: `CARGO_PKG_VERSION`).
- **114 unit tests** (new: `g_graph::tests` ×6 +
  `trident_voc::tests` ×7, incl. `walk_records_land_on_the_
  verified_section_boundaries` and
  `walk_landings_are_block_size_independent`); the guitest
  baseline `rows=430` is unchanged.

### Roadmap (v0.19.0)
- The B-tree node payloads of listA/listB (the word→`.vt`
  postings join): the search consumer code in `lmsup32.dll` (no
  export table).
- The `.g` graph edge semantics (the template-walk consumer of
  the u16 stream).
- The block size `DAT_1002ad98` (candidate 512; the write site
  is in the I/O layer, not in the decompilation).
