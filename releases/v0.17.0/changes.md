# v0.17.0 - Changes / Зміни (2026-10-27)

## UK / Українська

- **Точна `.voc`-таблиця секцій + морфологічні таблиці** (нова
  `src/trident_voc.rs`; `TridentIndex.morphology` + CLI-дієслово
  `trident` показує таблиці).
  - `VocSectionTable` — 17 u32 @0x3C (підтверджено по-байтовому в
    `lmu.voc`): `root=0x6832F`, `listA=0x68368` (722 записи),
    `listB=0x6CA96` (2157), `f-записи=0x75253` (142),
    `суфікси=0x76714` (722), `афікси=0x765B2` (7),
    `33-слів=0x765E1`, `двозначники=0x7668F` (порожня у lmu.voc);
    поля `0x74/0x78/0x7C` — подвійні (count, секція): 722 / 516 / 32.
  - **Довжина opcode-VM у байтах** (розбирання `lmsup32.dll`,
    Ghidra 12.1.4): `vm_program_byte_len` = `FUN_100044f0`
    (u16-потік; depth-лічильник `0x1001`/`0x1002`; повертає
    **байтову** позицію після top-level `0x1002`),
    `vm_record_scan_len` = `FUN_100044c0` (+скан до першого `0xFF`),
    `vm_record_len` = `FUN_100044a0` (`L + 3*count + 1`).
  - **Морфологія lmu.voc декодована**
    (`Morphology::parse` + `read_word_section` /
    `read_morph_records`, термінатор записів — подвійний NUL):
    - 7 афіксів: `най(1), по-(3), по(4), не(2), спів(6), анти(5),
      псевдо(5)`;
    - 722 суфікси/кінцівки (LmCode-слова, у т.ч. дефісні «ам-»,
      «і-»);
    - 33-словна таблиця `0x765E1`; двозначники `0x7668F` — порожня;
    - «конституція» як сирі LmCode-байти в lmu.voc **відсутня**
      (лише в щільному B-tree) — доведено сканом.
  - `lm_word_display` — рендер слів таблиць: 0x2d → `-`, 0x21 → `!`,
    літери через `LM_CHAR_TABLE`, решта — `·xx·`.
- **Базовий кодувальник: решта слотів докодована**
  (`src/ftsearch.rs`):
  - **`є` = `0xBA`** — підтверджено сканом `.mb`: «застосується»
    (29 збігів) та «затверджується» (34) збігаються лише з
    `0xBA` у позиції є;
  - `ґ` → `0xE3`, `Ґ` → `0xC3` (складання у слоти твердого
    г/Г — саме так база зберігає «загальний»);
  - `Ї` → `0xBF` (складання у збережений нижній `ї`; присвійні у
    базі нижнім реєстром, title-форми рідкісні);
  - **Drift оракула 222/223 документовано**: 204/223 офсетів
    збігаються точно; 19 офсетів оракула з'їхали (слово в
    `758.mb` 198873→199101; `177.mb` +311; `words.mb` перебудовано)
    + 18 нових позицій. Механізм точного слова правильний; 223/223
    на поточній базі недосяжний (оракел — старіший снимок).
- Версія **0.16.0 → 0.17.0** (єдине джерело: `CARGO_PKG_VERSION`).
- **100 юніт-тестів** (нові: `trident_voc::tests::{vm_program_byte_
  len_top_level, vm_record_len_adds_the_3c_tail, section_table_
  parses_real_lmu_voc, affix_table_decodes_the_seven_known_words,
  suffix_table_is_non_empty_lmcode, list_counts_match_the_dual_
  fields, lm_word_display_renders_hyphens_and_letters}` +
  `ftsearch::tests::{zahalnyi_encodes_with_folded_g, zastosuye_tsya_
  encodes_confirmed_ye}`).

### Roadmap (v0.18.0)
- Точні word→postings кордони (семантика packed-B-tree записів
  listA/listB; `lmsup32.dll` без export-таблиці); граф `.g`
  (даних @0x60, header «U.TEMPLATE.TXT»); пошук за графом.

## EN / English

- **Exact `.voc` section table + morphology tables** (new
  `src/trident_voc.rs`; `TridentIndex.morphology` + the `trident`
  CLI verb shows the tables).
  - `VocSectionTable` — 17 u32 @0x3C (verified byte-by-byte in
    `lmu.voc`): `root=0x6832F`, `listA=0x68368` (722 records),
    `listB=0x6CA96` (2157), `f-records=0x75253` (142),
    `suffixes=0x76714` (722), `affixes=0x765B2` (7),
    `33-word=0x765E1`, `digraphs=0x7668F` (empty in lmu.voc);
    the `0x74/0x78/0x7C` fields are dual (count, section):
    722 / 516 / 32.
  - **Opcode-VM byte lengths** (disassembly of `lmsup32.dll`,
    Ghidra 12.1.4): `vm_program_byte_len` = `FUN_100044f0`
    (u16 stream; `0x1001`/`0x1002` depth counter; returns the
    **byte** position after the top-level `0x1002`),
    `vm_record_scan_len` = `FUN_100044c0` (+scan to the first
    `0xFF`), `vm_record_len` = `FUN_100044a0`
    (`L + 3*count + 1`).
  - **lmu.voc morphology decoded** (`Morphology::parse` +
    `read_word_section` / `read_morph_records`; record terminator
    = double NUL):
    - 7 affixes: `най(1), po-(3), po(4), не(2), спів(6), анти(5),
      псевдо(5)`;
    - 722 suffixes/endings (LmCode words, incl. hyphenated «ам-»,
      «і-»);
    - the 33-word table at `0x765E1`; digraphs at `0x7668F` —
      empty;
    - «конституція» as raw LmCode bytes is **absent** from
      lmu.voc (packed B-tree only) — proven by scan.
  - `lm_word_display` — table-word rendering: 0x2d → `-`, 0x21 →
    `!`, letters via `LM_CHAR_TABLE`, everything else `·xx·`.
- **Base codepage: the remaining slots** (`src/ftsearch.rs`):
  - **`є` = `0xBA`** — confirmed by an `.mb` scan: «застосується»
    (29 hits) and «затверджується» (34) match only with `0xBA` in
    the є-position;
  - `ґ` → `0xE3`, `Ґ` → `0xC3` (folds onto the hard-г/Г slots — the
    base stores «загальний» that way);
  - `Ї` → `0xBF` (folds onto the stored lower `ї`; possessives are
    stored lower-case, title forms are rare);
  - **The 222/223 oracle drift is documented**: 204/223 offsets
    match exactly; 19 oracle offsets drifted (a `758.mb` word moved
    198873→199101; `177.mb` +311; `words.mb` rebuilt) + 18 new
    positions. The exact-word mechanism is correct; 223/223 is not
    reachable on the current base (the oracle snapshot is older).
- Version **0.16.0 → 0.17.0** (single source: `CARGO_PKG_VERSION`).
- **100 unit tests** (new: the seven `trident_voc::tests::{…}` +
  `ftsearch::tests::{zahalnyi_encodes_with_folded_g, zastosuye_tsya_
  encodes_confirmed_ye}`).

### Roadmap (v0.18.0)
- The exact word→postings boundaries (packed-B-tree record
  semantics of listA/listB; `lmsup32.dll` has no export table); the
  `.g` graph (data @0x60, header «U.TEMPLATE.TXT»); graph-based
  search.
