# v0.29.0 - Changes / Зміни (2026-10-28)

## UK / Українська

- **B-tree byte-VM core `FUN_100019f0` — part 2 (roadmap v0.30.0).**
  VM-спуск у node pool + affix-опи в `src/trident_vm.rs`:
  - **`NodePool { ptrs, byte_lens }`** — масив спуску `DAT_1002db56`
    (u32-показники вузлів + байтова довжина = 2-й байт показаного
    регіону, `pool[child]+1`); **`descend(ctx, pool, child)`** —
    `off_counter += byte_len(child)`, `leaf = child`.
  - **`VmRecord { keys, leaf, counter }`** + **`run_record(pool, payload)`**
    — повний part-1+2-скан: усі 7 part-2 опів (`0x43` спуск через pool,
    `0x44` case-swap, `0x46`/`0x47` affix, `0x4A`/`0x4B`/`0x4C`).
  - **`affix_op(op_tag, operand)`** — решолвер `FUN_10002320` (fold-class
    u16-маска `instance+0x76`; `'F'`=match-when-set, `'G'`=match-when-clear;
    без маски = re-scan path). `with_affix_mask` — 64-слотова per-instance
    таблиця.
  - **Корекція `0x49`**: м'який continue (DLL `FUN_100018b0` продовжує
    після `0x49`, відновлюючи offset-лічильник; лише `0x42`/`0xFF` —
    жорсткий стоп). Part-1 `run_scan` лишається як є; `run_record`
    реалізує виправлену семантику.
- **CLI `trident`**: рядок `vm-опкоди (v0.29.0, FUN_100019f0 part-1+2):
  15 op-code · part-1 7 · part-2 7 (0x43/44/46/47/4A/4B/4C) · run_record
  (node pool + affix; 0x49 soft-continue)` + usage + roadmap-сумарі.
- **171 юніт-тестів** (нові 5: node-pool descent, affix mask, 0x49
  soft-continue, full part-2 op coverage, real-stream run_record).
  Guitest baseline `rows=430` без змін.
- **DoD-метрика**: 14160-record `.vt` stream працює без panic через
  `run_record`; non-empty key rate = **0.738 (73.8 %)** (записано; дельта
  до 90 % — node-pool спуск: `DAT_1002db56` + case-fold таблиці — DLL-image
  глобали, deferred). Версія **0.28.0 → 0.29.0**.
- Далі (v0.30.0 / roadmap v0.31.0): `.voc` root B-tree descent
  (search chain, leaf → `.vt` join) — `VocBTree::find(word)`.

## EN / English

- **The B-tree byte-VM core `FUN_100019f0` — part 2 (roadmap v0.30.0).**
  The VM descent into the node pool + the affix ops in `src/trident_vm.rs`:
  - **`NodePool { ptrs, byte_lens }`** — the `DAT_1002db56` descent array
    (u32 node pointers + each node's byte length = the pointed-to region's
    second byte, `pool[child]+1`); **`descend(ctx, pool, child)`** advances
    `off_counter += byte_len(child)` and latches `leaf = child`.
  - **`VmRecord { keys, leaf, counter }`** + **`run_record(pool, payload)`**
    — the full part-1+2 scan: all 7 part-2 ops (`0x43` pool descent, `0x44`
    case-swap, `0x46`/`0x47` affix, `0x4A`/`0x4B`/`0x4C`).
  - **`affix_op(op_tag, operand)`** — the `FUN_10002320` resolver (the
    fold-class `u16` mask at `instance+0x76`; `'F'`=match-when-set,
    `'G'`=match-when-clear; no mask = the re-scan path). `with_affix_mask`
    — the 64-slot per-instance table.
  - **`0x49` correction**: the soft continue (the DLL's `FUN_100018b0`
    continues past `0x49`, restoring the offset counter; only `0x42`/`0xFF`
    hard-stop). The part-1 `run_scan` keeps its shipped semantics;
    `run_record` implements the corrected one.
- **CLI `trident`**: the `vm-опкоди (v0.29.0, FUN_100019f0 part-1+2): 15
  op-code · part-1 7 · part-2 7 (0x43/44/46/47/4A/4B/4C) · run_record
  (node pool + affix; 0x49 soft-continue)` line + the usage + roadmap
  summary.
- **171 unit tests** (5 new: the node-pool descent, the affix mask, the
  0x49 soft-continue, the full part-2 op coverage, the real-stream
  `run_record`). The guitest baseline `rows=430` is unchanged.
- **The DoD metric**: the 14160-record `.vt` stream runs without panic
  through `run_record`; the non-empty key rate = **0.738 (73.8 %)**
  (recorded; the gap to 90 % is the node-pool descent: `DAT_1002db56` +
  the case-fold tables are DLL-image globals, deferred). Version
  **0.28.0 → 0.29.0**.
- Next (v0.30.0 / roadmap v0.31.0): the `.voc` root B-tree descent (the
  search chain, leaf → `.vt` join) — `VocBTree::find(word)`.
