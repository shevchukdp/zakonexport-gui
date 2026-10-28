# v0.28.0 - Changes / Зміни (2026-10-28)

## UK / Українська

- **B-tree byte-VM core `FUN_100019f0` — part 1 (roadmap v0.29.0).**
  Новий `src/trident_vm.rs`:
  - `VmCtx { instance, depth, off_counter, counter, flags, out, fold }`
    + `VmScan { key, next_off, counter }` + **`run_scan(payload)`**
    (key-байти `≤0x3F` → casefold + емісія + лічильник; op-байти
    `>0x3F` → dispatch).
  - **Part-1 ops**: key-casefold, `0x40`, `0x41`, `0x42`/`0x49`/`0xFF`
    (flush/стоп), `0x45` (instance з u16, advance 3), `0x48` (reset,
    advance 1). **Part-2 ops** (`0x43`/`0x44`/`0x46`/`0x47`/`0x4A`/
    `0x4B`/`0x4C`) — advance-and-continue (node-pool — v0.30.0).
  - `CaseFold = [u8;128]` fixture (`IDENTITY_FOLD = b & 0x3F`);
    `VM_OPS` — 15-рядковий op-каталог (UA+EN + part-маркер),
    `vm_op_catalog_size` / `vm_part1_op_count`.
- **CLI `trident`**: новий рядок `vm-опкоди (v0.28.0, FUN_100019f0
  part-1): 15 op-code · part-1 7 · skeleton trident_vm::run_scan`.
- **166 юніт-тестів** (нові 7). Guitest baseline `rows=430` без змін.
- **DoD-метрика**: 14160-record `.vt` stream працює без panic;
  counter≥1 rate = **0.715 (71.5 %)** (записано; 90 % — ціль, дельта —
  part-2 node-pool, roadmap v0.30.0). Версія **0.27.0 → 0.28.0**.
- Далі (v0.29.0 / roadmap v0.30.0): VM descent + node pool + affix ops.

## EN / English

- **The B-tree byte-VM core `FUN_100019f0` — part 1 (roadmap v0.29.0).**
  New `src/trident_vm.rs`:
  - `VmCtx { instance, depth, off_counter, counter, flags, out, fold }`
    + `VmScan { key, next_off, counter }` + **`run_scan(payload)`**
    (key bytes `<=0x3F` → case-fold + emit + count; op bytes `>0x3F` →
    dispatch).
  - **Part-1 ops**: the key case-fold, `0x40`, `0x41`, `0x42`/`0x49`/
    `0xFF` (flush / stop), `0x45` (instance from the u16 operand,
    advance 3), `0x48` (reset, advance 1). **Part-2 ops** (`0x43`/
    `0x44`/`0x46`/`0x47`/`0x4A`/`0x4B`/`0x4C`) are advance-and-
    continue (the node pool is v0.30.0).
  - `CaseFold = [u8;128]` fixture (`IDENTITY_FOLD = b & 0x3F`);
    `VM_OPS` — a 15-row op catalog (UA+EN + the part marker),
    `vm_op_catalog_size` / `vm_part1_op_count`.
- **CLI `trident`**: a new `vm-опкоди (v0.28.0, FUN_100019f0 part-1):
  15 op-code · part-1 7 · skeleton trident_vm::run_scan` line.
- **166 unit tests** (7 new). The guitest baseline `rows=430` is
  unchanged.
- **The DoD metric**: the 14160-record `.vt` stream runs without panic;
  the counter≥1 rate = **0.715 (71.5 %)** (recorded; 90 % is the
  target, the gap is the part-2 node pool, roadmap v0.30.0). Version
  **0.27.0 → 0.28.0**.
- Next (v0.29.0 / roadmap v0.30.0): the VM descent + the node pool +
  the affix ops.
