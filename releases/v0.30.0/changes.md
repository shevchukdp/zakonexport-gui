# v0.30.0 - Changes / Зміни (2026-10-28)

## UK / Українська

- **`.voc` root B-tree descent (roadmap v0.31.0) — part 1.** `trident_voc.rs`:
  - **`VocBTreeNode`** (`FUN_10002b70` case-fold-вузол): byte0 `0x80` =
    child-list flag, `0x40` = `FUN_100028e0` leaf-path; byte1 `0x80` =
    count-present, low 7 bits = child count; bytes 2.. = u16 child list
    (`<3000` = `FUN_10002620` pool-resolve; `3000..0xC1C` = virtual children
    `+0x48` під flags `0x1000`/`0x4000`).
  - **`VocBTree { root, pool }`** (root = `+0x3C` з `VocSectionTable.root`;
    `pool` = `DAT_1002db56`, caller-supplied) + **`VocLeaf { node_off,
    vt_off, key }`** + **`find(word)`** — pool-descent скелет (`<3000`
    children → `pool[c]` → найглибший leaf).
- **Емпірика (real lmu.voc)**: root @0x6832f, count=26, УСІ 26 children
  `<3000`; root 0x40-flag SET. `find()` з empty pool зупиняється на root
  (без panic); synthetic pool-descend доходить до leaf.
- **`TridentIndex.lmu_voc`** — raw `lmu.voc` bytes (re-parse root-вузла).
- **CLI `trident`**: новий рядок `voc-B-tree (v0.30.0, FUN_10002b70):
  root=0x6832f count=26 (26 pool-children) · 0x40-flag yes · find() =
  pool-descent skeleton (node-pool pin deferred)`.
- **173 юніт-тестів** (нові 2: real lmu.voc root decode + synthetic
  pool-descent leaf). Guitest baseline `rows=430` без змін.
- Версія **0.29.0 → 0.30.0**.
- Далі (v0.31.0 / roadmap v0.32.0): node-pool pin + `FUN_100028e0` + alias
  tables (`DAT_1002eca2..eca5`) — повне «конституція»-розв'язання + leaf→`.vt`
  join.

## EN / English

- **The `.voc` root B-tree descent (roadmap v0.31.0) — part 1.**
  `trident_voc.rs`:
  - **`VocBTreeNode`** (the `FUN_10002b70` case-fold node): byte 0 `0x80` =
    the child-list flag, `0x40` = the `FUN_100028e0` leaf-path; byte 1 `0x80`
    = the count-present flag, low 7 bits = the child count; bytes 2.. = the
    u16 child list (`<3000` = the `FUN_10002620` pool resolve;
    `3000..0xC1C` = the virtual children, the `+0x48` offset under flags
    `0x1000`/`0x4000`).
  - **`VocBTree { root, pool }`** (root = `+0x3C` from `VocSectionTable.root`;
    `pool` = the `DAT_1002db56`, caller-supplied) + **`VocLeaf { node_off,
    vt_off, key }`** + **`find(word)`** — the pool-descent skeleton
    (`<3000` children → `pool[c]` → the deepest leaf).
- **Empirics (the real lmu.voc)**: the root @0x6832f, count=26, ALL 26
  children `<3000`; the root 0x40-flag is SET. `find()` with an empty pool
  stops at the root (no panic); the synthetic pool-descent reaches a leaf.
- **`TridentIndex.lmu_voc`** — the raw `lmu.voc` bytes (for the root-node
  re-parse).
- **CLI `trident`**: a new `voc-B-tree (v0.30.0, FUN_10002b70): root=0x6832f
  count=26 (26 pool-children) · 0x40-flag yes · find() = pool-descent
  skeleton (node-pool pin deferred)` line.
- **173 unit tests** (2 new: the real lmu.voc root decode + the synthetic
  pool-descent leaf). The guitest baseline `rows=430` is unchanged.
- Version **0.29.0 → 0.30.0**.
- Next (v0.31.0 / roadmap v0.32.0): the node-pool pin + `FUN_100028e0` + the
  alias tables (`DAT_1002eca2..eca5`) — the full «конституція» resolution +
  the leaf→`.vt` join.
