# v0.32.0 - Changes / Зміни (2026-10-28)

## UK / Українська

- **`.voc` per-word `'f'` opcode-VM execution (roadmap v0.33.0).**
  `trident_voc.rs`:
  - **`FResult { byte_len, word_count, data, terminated }`** +
    **`run_f_program(rec, child_word_count)`** — виконав `FUN_100044f0`
    (u16 instruction stream `0x1001..0x1004`): `0x1001`=subtree-enter,
    `0x1002`=terminator (top-level — стоп), `0x1003`=word-leaf
    (`word_count += 1`), `0x1004 [u16 ptr]`=child node (resolver
    `child_word_count` через `DAT_1002ddec`; `None`-default → 0; child ptr в
    `data`), інші u16 = inline data (в `data`).
  - **`Morphology.f_programs: Option<Vec<FResult>>`** — execution-report
    (усі walked `+0x48` записи; `None` коли секція відсутня).
- **Count invariant (UA+EN, DoD).** У `lmu.voc`: `+0x48` декларує **142**
  записів, і walk (block=4096) ходить по ВСІХ 142 (double-NUL in-file).
  Виконані ВСІ 142: **142/142 завершуються на top-level `0x1002`** +
  **176 `0x1003` word-leaf hits**. `0x1004` child word-counts потребують
  `DAT_1002ddec` node pool (DLL-image глобал, deferred) → structural
  skeleton. EN: all 142 declared per-word `'f'` programs are walked and all
  142 terminate at the top-level `0x1002`; 176 `0x1003` word-leaf hits are
  the resolvable part; the `0x1004` child counts need the `DAT_1002ddec`
  pool (deferred).
- **Loader-consumer triage (documented, clean-room):**
  - `+0x5C` — **raw seek** (`FUN_10021800(handle, +0x5C)` → `DAT_1002db76`);
    без record-walk.
  - `+0x64` — 33-word table, walked by the **same** loader
    (`FUN_10004230(handle, +0x64, 'n')` під `param_1 & 8`) →
    `DAT_1002db7e`. Не окремий consumer — це `& 8` mode-гілка лоадера.
- **CLI `trident`**: новий рядок `f-opcode (v0.32.0, FUN_100044f0): 142
  programs (142 declared) · terminated 142 · word-leaf hits 176 · 0x1004
  child counts need the DAT_1002ddec pool (skeleton)` + usage +
  roadmap-сумарі.
- **181 юніт-тестів** (нові 4). Guitest baseline `rows=430` без змін.
  Версія **0.31.0 → 0.32.0**.
- Далі (v0.33.0 / roadmap v0.34.0): `.g` edge semantics (the graph-template
  walker) — `GGraph::walk()`.

## EN / English

- **The `.voc` per-word `'f'` opcode-VM execution (roadmap v0.33.0).**
  `trident_voc.rs`:
  - **`FResult { byte_len, word_count, data, terminated }`** +
    **`run_f_program(rec, child_word_count)`** — the `FUN_100044f0`
    executor (the `0x1001..0x1004` u16 instruction stream): `0x1001`=
    subtree-enter (depth+1), `0x1002`=terminator (top-level stops),
    `0x1003`=word-leaf (`word_count += 1`), `0x1004 [u16 ptr]`=child node
    (the resolver = the node's word count via `DAT_1002ddec`; the `None`
    default contributes 0; the child pointer is recorded in `data`), other
    u16 = inline data (into `data`).
  - **`Morphology.f_programs: Option<Vec<FResult>>`** — the execution report
    (every walked `+0x48` record; `None` when the section is absent).
- **The count invariant (UA+EN, the DoD).** In `lmu.voc`: the `+0x48`
  section declares **142** records and the `block=4096` walk walks **all
  142** (each ends in a double-NUL in-file). All 142 run through
  `run_f_program`: **142/142 terminate at the top-level `0x1002`** +
  **176 `0x1003` word-leaf hits**. The `0x1004` child-subtree word counts
  need the `DAT_1002ddec` node pool (a DLL-image global, deferred) —
  `f_programs` is the structural skeleton. EN: all 142 declared per-word
  `'f'` programs are walked and all 142 terminate at the top-level `0x1002`;
  the 176 `0x1003` word-leaf hits are the resolvable part; the `0x1004`
  child counts need the `DAT_1002ddec` pool (deferred).
- **The loader-consumer triage (documented, clean-room):**
  - `+0x5C` — a **raw seek** (`FUN_10021800(handle, +0x5C)` →
    `DAT_1002db76`); no record walk (a direct file-position/count read).
  - `+0x64` — the 33-word table, walked by the **same** loader
    (`FUN_10004230(handle, +0x64, 'n')` under the `param_1 & 8` flag) →
    `DAT_1002db7e`. Not a separate consumer — it is the loader's `& 8` mode
    branch.
- **CLI `trident`**: a new `f-opcode (v0.32.0, FUN_100044f0): 142 programs
  (142 declared) · terminated 142 · word-leaf hits 176 · 0x1004 child
  counts need the DAT_1002ddec pool (skeleton)` line + the usage + the
  roadmap summary.
- **181 unit tests** (4 new: the top-level terminator, the child-resolver,
  the inline-data, the real lmu.voc 142-program report). The guitest
  baseline `rows=430` is unchanged. Version **0.31.0 → 0.32.0**.
- Next (v0.33.0 / roadmap v0.34.0): the `.g` edge semantics (the
  graph-template walker) — `GGraph::walk()`.
