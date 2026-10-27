# v0.19.0 - Changes / Зміни (2026-10-28)

## UK / Українська

- **Точний leaf→`.vt` stem-join** (головний новий механізм версії;
  пропорційний `posting_slice` v0.15.0 лишається fallback):
  - **`.vt` layout експериментально декодовано** (byte-dumps реального
    `lmur.vt`): 64B-заголовок — vendor «Trident Soft. Inc.» @0x00,
    name `lmUR.vt` @0x18, маркер `FREE` @0x38, u32 @0x3C =
    **хвіст** (кінець record-потіку, `0xCCDE5` = filelen−4; останні
    4 байти файлу = цей же u16 в LE). Потік = **14160 зажатих
    NUL-термінованих записів** `[u8 count][payload][NUL]`
    `0x40..0xCCDE5` (повний walk приходить точно в хвіст).
    Payload = **case-folded Latin-стеблі** словника + u16-слова
    (документи 0..65535): «не»→`NE`, «по»→`PO`, «пере»→`PERE`,
    «спів»→`SPV` (і→V), «був»→`BUV` (в→V), «бус»→`BUS` (з→S),
    «неч»→`HCH` (н→H!), Ї→`Y`. Маппінг **контекстно-двозначний**
    (стеблі граф-нормалізовані через `lmu.g`) — тому кандидатний
    підхід. Потік в тому ж порядку сортування, що й `.voc`
    словник (`@` 0x40 та цифри 0x30–0x39 — в самому початку).
  - **Новий модуль `src/trident_vt.rs`**: `TridentFileHeader`
    (64B), `VtRecordStream::parse` (walk `0x40..tail`) +
    `find_stem(needle)` (латинський run-під рядок), `VmHeader`
    (`.m`: 64B + u32-таблиця @0x40; `lmur.m`: `0x1000, 0x88, 0xBCB,
    0x726, 0xA1, 0x206D, 0x244B, 0x29D7`, хвіст @0x70E).
    Документація DLL join-ланцюга: `FUN_10015340` (ctx 0x58B: 9
    seek-ів + per-section handles), `FUN_100176c0` (u16 word-
    таблиця @ctx+0x44: `table[idx]` → record kind-байт `0xFA`/
    `0x33`), `FUN_10015cd0` (u16 predicate-таблиця @ctx+0x30,
    `0xFF` = всі документи), `FUN_100177d0` (doc-flag предикати,
    `.m` 0xE-структури, flags 0xD7/0xD8/0xD9), `FUN_100134d0`
    (tail-scan: `[LmCode-рядок NUL][u16-список 0x0000]`,
    wildcard-матчинг `FUN_100135d0`), `FUN_100019f0` (byte-VM:
    ≤0x3F = key-byte, `0x43` = child-descent, `0x40` = кінець,
    `0x49`/`0x42`/`0xFF` = стоп, `0x44`/`0x45`/`0x48`/`0x4A`/
    `0x4B`/`0x4C` = case-swap/depth/reset/variant/restart/
    backtrack). **Blocked (v0.20)**: в old-format `.vt` поля
    header `0x4C..0x7C` = сміт потоку; u16 word/predicate-таблиці
    DLL read-mode виводить з tail B-tree.
  - **`src/trident.rs`**: `latin_candidates` (`LATIN_VARIANTS`,
    пріоритетні варіанти per-літера, BFS cap 32) +
    `exact_postings(word)` (LmCode → кандидатів → records + їхні
    u16-списки, dedup по record, cap 32) + `TridentResult.joined`
    + `search()`: exact-join primary / `posting_slice` fallback.
    Перевірено: «був» → `[a1 e3 a2]` → `BUV` → 2 records
    (@0x5362b / @0x84181 — саме ті, де знайшли `BUV` у
    0x537D0/0x8422F).
- **`.voc` u16-параметри @0x80** (`trident_voc.rs`, новий
  `VocParams`): 16 u16 — `2,2,1,0x600,2,2,0x42,0x2000,0x20,0x108,
  0x104,0x10,2,4,0x80,0x400`. Ідентифіковано: `[6]` = listA
  stop-marker `0x42` ('B' — точно той термінатор record-walker'а),
  `[8]` = sec_54 count (32), `[9]` = розмір `lmr.gw` (264), `[7]`
  = page-константа 0x2000. `lmr.voc`: `2,1,0x220,0x402,0x800,0x10,
  8,4` (решта 0).
- **CLI `trident`** — нові блоки: `lmur.vt: 14160 record-ів
  0x40..0xccde5 · 76225 Latin-run-ів · 402210 u16-слів`; `lmur.m:
  u32-таблиця @0x40 (8) · хвіст @0x70e`; `lmu.voc параметри @0x80:
  stop=0x42 блок=0x2000 sec_54=32 gw=264`. **CLI `tsearch`** —
  новий блок «join word→.vt (v0.19.0, точний stem-join)»: record-
  офсет + u16-список («був» → 2 records; «пере не по» → 69
  records: PERE×5 + NE×32 + PO×32).
- **125 юніт-тестів** (вперше >100): нові `trident_vt::tests`
  (header / 14160-walk / `find_stem` на реалі / `.m` / синтетика),
  `VocParams` (значення реального `lmu.voc`), `latin_candidates` +
  `exact_postings` (реальна база). Guitest baseline `rows=430`
  без змін.

## EN / English

- **The exact leaf→`.vt` stem-join** (the headline mechanic; the
  proportional `posting_slice` of v0.15.0 stays as the fallback):
  - **the `.vt` layout was empirically decoded** (byte dumps of the
    real `lmur.vt`): a 64-byte header — vendor «Trident Soft.
    Inc.» @0x00, name `lmUR.vt` @0x18, the `FREE` marker @0x38, u32
    @0x3C = **tail** (the end of the record stream, `0xCCDE5` =
    filelen−4; the file's last 4 bytes are that u32 in LE). The
    stream = **14160 packed NUL-terminated records**
    `[u8 count][payload][NUL]` in `0x40..0xCCDE5` (the full walk
    lands exactly on the tail). Payload = **case-folded Latin
    stems** of the dictionary + u16 words (documents 0..65535):
    «не»→`NE`, «по»→`PO`, «пере»→`PERE`, «спів»→`SPV` (і→V),
    «був»→`BUV` (в→V), «бус»→`BUS` (з→S), «неч»→`HCH` (here
    н→H!), Ї→`Y`. The mapping is **context-dependent** (the stems
    are graph-normalized through `lmu.g`) — hence the candidate
    approach. The stream keeps the `.voc` dictionary's sort order
    (`@` 0x40 and the digits 0x30–0x39 sort first).
  - **New module `src/trident_vt.rs`**: `TridentFileHeader` (64
    bytes), `VtRecordStream::parse` (the `0x40..tail` walk) +
    `find_stem(needle)` (Latin run-substring match), `VmHeader`
    (`.m`: 64-byte header + the u32 table @0x40; `lmur.m`:
    `0x1000, 0x88, 0xBCB, 0x726, 0xA1, 0x206D, 0x244B, 0x29D7`,
    tail @0x70E). Docs of the DLL join chain: `FUN_10015340` (the
    0x58-byte context: 9 seeks + the per-section handles),
    `FUN_100176c0` (the u16 word table @ctx+0x44: `table[idx]` →
    the record's kind byte `0xFA`/`0x33`), `FUN_10015cd0` (the
    u16 predicate table @ctx+0x30, `0xFF` = all documents),
    `FUN_100177d0` (the doc-flag predicates, the `.m` 0xE
    structs, flags 0xD7/0xD8/0xD9), `FUN_100134d0` (the tail
    scan: `[LmCode string NUL][u16 list 0x0000]`, the wildcard
    matcher `FUN_100135d0`), `FUN_100019f0` (the byte-VM: ≤0x3F =
    a key byte, `0x43` = child descent, `0x40` = end,
    `0x49`/`0x42`/`0xFF` = stop, `0x44`/`0x45`/`0x48`/`0x4A`/
    `0x4B`/`0x4C` = case-swap/depth/reset/variant/restart/
    backtrack). **Blocked (v0.20):** in the old-format `.vt` the
    header fields `0x4C..0x7C` are stream bytes; the DLL
    read-mode derives the u16 word/predicate tables from the tail
    B-tree.
  - **`src/trident.rs`**: `latin_candidates` (`LATIN_VARIANTS`,
    preferred variants per letter, BFS cap 32) +
    `exact_postings(word)` (LmCode → candidates → records + their
    u16 lists, deduplicated per record, cap 32) +
    `TridentResult.joined` + `search()`: the exact join primary,
    the `posting_slice` fallback. Verified: «був» → `[a1 e3 a2]`
    → `BUV` → 2 records (@0x5362b / @0x84181 — exactly the
    records carrying the confirmed `BUV` bytes).
- **`.voc` u16 parameters @0x80** (`trident_voc.rs`, new
  `VocParams`): 16 u16 — `2,2,1,0x600,2,2,0x42,0x2000,0x20,0x108,
  0x104,0x10,2,4,0x80,0x400`. Identified: `[6]` = the listA stop
  marker `0x42` ('B' — exactly the record-walker terminator),
  `[8]` = the sec_54 count (32), `[9]` = the `lmr.gw` size (264),
  `[7]` = the 0x2000 page constant. `lmr.voc`: `2,1,0x220,0x402,
  0x800,0x10,8,4` (the rest zero).
- **CLI `trident`** — new blocks: `lmur.vt: 14160 records
  0x40..0xccde5 · 76225 Latin runs · 402210 u16 words`; `lmur.m:
  u32 table @0x40 (8 entries) · tail @0x70e`; `lmu.voc params
  @0x80: stop=0x42 block=0x2000 sec_54=32 gw=264`. **CLI
  `tsearch`** — the new «join word→.vt (v0.19.0, exact stem-join)»
  block: the record offset + the u16 list («був» → 2 records;
  «пере не по» → 69 records: PERE×5 + NE×32 + PO×32).
- **125 unit tests** (the first >100): new `trident_vt::tests`
  (header / the 14160 walk / `find_stem` on the real file / `.m` /
  synthetic), `VocParams` (the real `lmu.voc` values),
  `latin_candidates` + `exact_postings` (real base). The guitest
  baseline `rows=430` is unchanged.
