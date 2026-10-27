# v0.18.1 - Changes / Зміни (2026-10-27)

## UK / Українська

- **Блок-розмір I/O-шару підтверджено: `WALK_BLOCK` = 4096** (у
  v0.18.0 був «кандидат 512»).
  - `DAT_1002ad98` (`FUN_10021940()` — розмір буфера block-resync)
    **не має жодного запису в `.text`**: byte-scan варіантів
    `C7 05` (imm-store), `89 05/15/25/35` (reg-store), `A3`
    (mov [imm], eax) з абсолютним disp32 дав 0 записів; лише 5
    зчитувань (`8B 35` / `A1`) — збігається з Ghidra xref-листом.
    Отже глобальна зберігає початкове значення образу `.data`:
    `0x00001000` = **4096** (page-sized буфер читання).
  - Незалежне підтвердження даними `lmu.voc`: resync-лондінги
    listA (packed B-tree) при блок=4096 лежать **точно** на
    4096-алінованих node-slots `0x69000, 0x6A000, … 0x77000`
    (15 слотів, step 0x1000) — B-tree-вузли паковані по block.
  - Коректні наслідки для record-walker'а: при 4096 listA
    обходить **388** resync-записів (а не 722 при 512), listB —
    **382** (а не 819); обидва лондінги `0x78000` (за EOF).
    Чисті секції (f, ff, афікси, 33-слів, +0x54, суфікси) не
    залежать від блоку — їхні точні лондінги (`0x764C5`,
    `0x765B1`, `0x765E1`, `0x7667A`, `0x76714`, `0x77B78`)
    незмінні.
- **B-tree пошуковий ланцюг відстежено** (розбирання
  `lmsup32.dll`; документація `src/trident_voc.rs` уточнена —
  root-таблиця `+0x3C` це **корінь packed B-tree**, а не
  opcode-VM):
  - `FUN_10002ff0` — пошуковий вхід: seek до `handle+0x3C`
    (root-офсет), далі `FUN_10002b70`;
  - `FUN_10002b70` — case-fold descent по вузлу (прапорець
    `0x80` = список дітей, кількість `&0x7f`; children
    id<3000 → `FUN_10002620`, ≥3000 → special `FUN_100030b0`);
  - `FUN_10002620`/`FUN_10001920` — node-ID → запис через
    4-байтовий **node pointer pool** (`*(pool+id*4)+2` = ключ);
  - `FUN_100018b0`/`FUN_100019f0` — payload-walk: стоп NUL/`0xFF`/
    `0x42`, `0x49` = skip-маркер.
  - Точні leaf→`.vt` posting-офсети (join) — наступний крок
    (v0.19.0).
- **`FUN_10004370` ідентифіковано** — free/validator
  record-pointer-масиву від `FUN_10004230`: по кожному вказівнику
  переобчислює довжину запису по режиму (n/e/f/ff) і перевіряє
  інваріант подвійного NUL (`*(rec+len)==0`), після масиву —
  `FUN_10004480`.
- Версія **0.18.0 → 0.18.1** (єдине джерело: `CARGO_PKG_VERSION`).
- **114 юніт-тестів** (асерти listA/listB оновлено під 4096:
  388/382 resync-записи + 15 4096-алінованих slot-ів); guitest
  baseline `rows=430` без змін.

## EN / English

- **I/O-layer block size confirmed: `WALK_BLOCK` = 4096** (v0.18.0
  carried the "candidate 512").
  - `DAT_1002ad98` (`FUN_10021940()` — the block-resync buffer
    size) has **no store anywhere in `.text`**: a byte-scan of the
    `C7 05` (imm-store), `89 05/15/25/35` (reg-store) and `A3`
    (mov [imm], eax) forms with absolute disp32 found zero stores;
    only five loads (`8B 35` / `A1`) — matching the Ghidra xref
    list. The global therefore keeps its `.data` image initial
    value `0x00001000` = **4096** (the page-sized read buffer).
  - Independent data-driven confirmation from `lmu.voc`: the listA
    (packed B-tree) resync landings at block=4096 sit **exactly**
    on the 4096-aligned node slots `0x69000, 0x6A000, … 0x77000`
    (15 slots, step 0x1000) — the B-tree nodes are packed per
    block.
  - Record-walker consequences: at 4096 the listA walk covers
    **388** resync records (not 722 at 512), listB **382** (not
    819); both land at `0x78000` (just past EOF). The clean
    sections (f, ff, affixes, 33-word, +0x54, suffixes) are
    block-independent — their exact landings (`0x764C5`,
    `0x765B1`, `0x765E1`, `0x7667A`, `0x76714`, `0x77B78`) are
    unchanged.
- **The B-tree search chain traced** (disassembly of
  `lmsup32.dll`; `src/trident_voc.rs` docs corrected — the root
  table `+0x3C` is a **packed B-tree root**, not an opcode-VM):
  - `FUN_10002ff0` — the search entry: seeks to `handle+0x3C`
    (the root offset), then `FUN_10002b70`;
  - `FUN_10002b70` — the case-fold descent over a node (the `0x80`
    flag = the child list, count `&0x7f`; child ids <3000 →
    `FUN_10002620`, ≥3000 → the special `FUN_100030b0`);
  - `FUN_10002620`/`FUN_10001920` — a node id resolves to a record
    through the 4-byte **node pointer pool** (`*(pool+id*4)+2` =
    the key);
  - `FUN_100018b0`/`FUN_100019f0` — the payload walk: stop at NUL /
    `0xFF` / `0x42`, `0x49` = a skip marker.
  - The exact leaf → `.vt` posting offsets (the join) remain the
    next step (v0.19.0).
- **`FUN_10004370` identified** — the free/validator of the
  `FUN_10004230` record-pointer array: for every pointer it
  recomputes the record length per mode (n/e/f/ff) and checks the
  double-NUL invariant (`*(rec+len)==0`), then frees the array.
- Version **0.18.0 → 0.18.1** (single source: `CARGO_PKG_VERSION`).
- **114 unit tests** (the listA/listB assertions updated for 4096:
  388/382 resync records + the 15 4096-aligned slots); the guitest
  baseline `rows=430` is unchanged.
