# v0.31.0 - Changes / Зміни (2026-10-28)

## UK / Українська

- **`.voc` 4096-resync dense-node payload semantics (roadmap v0.32.0).**
  `trident_voc.rs`:
  - **`DenseNode`** — один 4096-resync-слот (`FUN_10002b70` header): byte0
    `0x80`=child-list / `0x40`=`FUN_100028e0` leaf-path; byte1 `0x80`=
    count-present, low7=count; payload = u16 child list (child-вузли) або
    packed LmCode key-runs (leaf/key). `extent()` = `2 + count*2` (child) /
    `2 + count` (leaf).
  - **`VocDenseNodes { slots, list_a_count, list_b_count, list_a_walked,
    list_b_walked }`** — data-driven decode: 4096-aligned слоти від
    `ceil(listA/4096)*4096` до першого walk-landing (у `lmu.voc` = 15 слотів
    `0x69000..0x77000`). `parse(voc)` + `reconcile() -> DenseReconcile` +
    `validate() -> (ok, mismatches)`.
  - **`DenseReconcile`** — reconciliation facts: `delta() = listB − listA`
    (1435), `list_a_dense()` (334 = 722−388), `list_b_dense()`
    (1775 = 2157−382).
- **722/2157 reconciliation (DoD-формула, UA+EN).** Section headers
  декларує `listA=722` / `listB=2157` packed-записів, але pointer-array
  resync-walk (block=4096) ходить лише `388` / `382` starts (landed=0x78000).
  Дельта `2157−722=1435` + dense-shortfalls `334`/`1775` — node-ключі,
  запаковані всередині 4096-block data та споживані напряму.
- **In-memory word-struct layout (documented, clean-room):** `FUN_10004370`
  oracle — NULL-terminated масив word-struct pointer (`param_3`), по одному
  на запис; byte-довжина = mode-specific (`'n'`=strlen, `'e'`=`FUN_100041d0`,
  `'f'`=`FUN_100044a0`, `-1`=`FUN_10004210`); валідатор асертить
  `*(ptr+len)==0` (non-NUL = `FUN_10021940`). Dense-node = on-disk
  packed-форма цих struct.
- **CLI `trident`**: новий рядок `voc-dense (v0.31.0, 4096-resync): 15 slots
  (0x69000..0x77000) · 722/2157 delta 1435 · dense shortfalls 334/1775 ·
  validate 15/15 ok` + usage + roadmap-сумарі.
- **177 юніт-тестів** (нові 4). Guitest baseline `rows=430` без змін.
  Версія **0.30.0 → 0.31.0**.
- Далі (v0.32.0 / roadmap v0.33.0): per-word `'f'` opcode-VM execution +
  load (повне LmCode key cross-check vs dictionary + `.vt` — `DAT_1002e642`
  case-fold таблиця, DLL-image глобал, deferred).

## EN / English

- **The `.voc` 4096-resync dense-node payload semantics (roadmap v0.32.0).**
  `trident_voc.rs`:
  - **`DenseNode`** — one 4096-resync slot (the `FUN_10002b70` header):
    byte 0 `0x80`=child-list / `0x40`=the `FUN_100028e0` leaf-path; byte 1
    `0x80`=count-present, low7=count; the payload is the u16 child list
    (child nodes) or the packed LmCode key runs (leaf/key). `extent()` =
    `2 + count*2` (child) / `2 + count` (leaf).
  - **`VocDenseNodes { slots, list_a_count, list_b_count, list_a_walked,
    list_b_walked }`** — the data-driven decode: the 4096-aligned slots from
    `ceil(listA/4096)*4096` to the first walk landing (in `lmu.voc` = the 15
    slots `0x69000..0x77000`). `parse(voc)` + `reconcile() -> DenseReconcile`
    + `validate() -> (ok, mismatches)`.
  - **`DenseReconcile`** — the reconciliation facts: `delta() = listB −
    listA` (1435), `list_a_dense()` (334 = 722−388), `list_b_dense()`
    (1775 = 2157−382).
- **The 722/2157 reconciliation (the DoD formula, UA+EN).** The section
  headers declare `listA=722` / `listB=2157` packed records, but the
  pointer-array resync walk (block=4096) walks only `388` / `382` starts
  before it runs off the file end (landed=0x78000). The delta
  `2157−722=1435` + the dense shortfalls `334`/`1775` are the node keys
  packed inside the 4096-block data, consumed directly rather than via the
  pointer array.
- **The in-memory word-struct layout (documented, clean-room):** the
  `FUN_10004370` oracle — a NULL-terminated array of word-struct pointers
  (`param_3`), one per record; the struct byte length is mode-specific
  (`'n'`=strlen, `'e'`=`FUN_100041d0`, `'f'`=`FUN_100044a0`,
  `-1`=`FUN_10004210`); the validator asserts `*(ptr+len)==0` (a non-NUL =
  the free/overrun `FUN_10021940`). The dense-node decode is the on-disk
  packed form of exactly these per-word structs.
- **CLI `trident`**: a new `voc-dense (v0.31.0, 4096-resync): 15 slots
  (0x69000..0x77000) · 722/2157 delta 1435 · dense shortfalls 334/1775 ·
  validate 15/15 ok` line + the usage + the roadmap summary.
- **177 unit tests** (4 new: the real lmu.voc 15-slot decode, the
  validate-clean pass, the `DenseNode` extent, the `DenseReconcile`
  delta/shortfalls). The guitest baseline `rows=430` is unchanged. Version
  **0.30.0 → 0.31.0**.
- Next (v0.32.0 / roadmap v0.33.0): the per-word `'f'` opcode-VM execution +
  load (the full LmCode key cross-check vs the dictionary + `.vt` — the
  `DAT_1002e642` case-fold table is a DLL-image global, deferred).
