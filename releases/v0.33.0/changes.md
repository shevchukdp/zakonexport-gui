# v0.33.0 - Changes / Зміни (2026-10-28)

## UK / Українська

- **`.g` graph-template walker, part 1 (roadmap v0.34.0).** `g_graph.rs`:
  - **`GVertex` / `GEdge` / `GGraphWalk`** + **`GGraph::walk() ->
    GGraphWalk`** + **`vertices()`** / **`edges()`** — структурний
    skeleton-декод u16 stream'а `.g`. Граф-воркер DLL
    (`FUN_10019060`/`FUN_10019300`) читє stream як byte-послідовність і
    dispatch-ує по маркерам: **`0x27` (`'`) = vertex record** (іменована
    точка, `| 2` flag), **`0x2d` (`-`) = edge/arc record**
    (`FUN_10011830` link), `0x28` = sub-graph terminator, `0x2e..0x31` =
    child-ref, `0x32` = char-run, `0x3b` = nested structure. `walk()`
    рахує дві primary-класи маркерів + consumed u16-довжину (stream-end
    fingerprint); повне будівництво 16-байт slot array через
    `DAT_1002f1f0` template table = part-2 DLL-image pin (deferred).
  - **`map_gw_weights(weights, edge_count)`** — мапує `.gw` companion
    (134 u16, `0x8000` flag bit = weighted/active edge) на walked edges за
    індексом; лишок 134-стріму = власна weight table template.
- **Count invariant (UA+EN, DoD).** Реальні файли: **lmu.g — 31 vertex
  (`0x27`) / 26 edge (`0x2d`), `consumed = 31796` u16** (== stream-end);
  **lmr.g — 37 vertex / 47 edge, `consumed = 25311` u16**. Walker
  термінується на обох реальних файлах; `consumed` == stream-end
  (fingerprint). EN: both real `.g` files walk to the stream end with the
  documented vertex/edge marker counts; `consumed` is the stream-end
  fingerprint.
- **CLI `trident`**: новий рядок `g-walk (v0.33.0, FUN_10019060): vertices
  31 (0x27) · edges 26 (0x2d) · consumed 31796 u16 · lmu.gw → 26 ребер (0
  прап. 0x8000)` + usage + roadmap-сумарі.
- **186 юніт-тестів** (нові 5: synthetic walk+counts, `map_gw_weights`
  clamp/flag, real lmu.g/lmr.g walk fingerprints, `.gw`→edge mapping).
  Guitest baseline `rows=430` без змін. Версія **0.32.0 → 0.33.0**.
- Далі (v0.34.0 / roadmap v0.35.0): наступний пункт ROADMAP.

## EN / English

- **The `.g` graph-template walker, part 1 (roadmap v0.34.0).**
  `g_graph.rs`:
  - **`GVertex` / `GEdge` / `GGraphWalk`** + **`GGraph::walk() ->
    GGraphWalk`** + **`vertices()`** / **`edges()`** — the structural
    skeleton decode of the `.g` u16 stream. The DLL's graph walker
    (`FUN_10019060`/`FUN_10019300`) reads the stream as a byte sequence
    and dispatches on the markers: **`0x27` (`'`) = a vertex record**
    (a named point, the `| 2` flag), **`0x2d` (`-`) = an edge / arc
    record** (the `FUN_10011830` link), `0x28` = a sub-graph terminator,
    `0x2e..0x31` = a child reference, `0x32` = a char run, `0x3b` = a
    nested structure. `walk()` counts the two primary marker classes + the
    consumed u16 length (the stream-end fingerprint); the full 16-byte
    slot-array construction via the `DAT_1002f1f0` template table is the
    part-2 DLL-image pin (deferred).
  - **`map_gw_weights(weights, edge_count)`** — maps the `.gw` companion
    (134 u16; the `0x8000` flag bit = a weighted / active edge) onto the
    walked edges by index; the 134-stream surplus is the template's own
    weight table.
- **The count invariant (UA+EN, the DoD).** The real files: **lmu.g — 31
  vertices (`0x27`) / 26 edges (`0x2d`), `consumed = 31796` u16** (== the
  stream end); **lmr.g — 37 vertices / 47 edges, `consumed = 25311`
  u16**. The walker terminates on both real files; `consumed` == the
  stream end (the fingerprint).
- **CLI `trident`**: a new `g-walk (v0.33.0, FUN_10019060): vertices 31
  (0x27) · edges 26 (0x2d) · consumed 31796 u16 · lmu.gw -> 26 edges (0
  flagged 0x8000)` line + the usage + the roadmap summary.
- **186 unit tests** (5 new: the synthetic walk+counts, the
  `map_gw_weights` clamp/flag, the real lmu.g/lmr.g walk fingerprints, the
  `.gw`-to-edge mapping). The guitest baseline `rows=430` is unchanged.
  Version **0.32.0 → 0.33.0**.
- Next (v0.34.0 / roadmap v0.35.0): the next ROADMAP item.
