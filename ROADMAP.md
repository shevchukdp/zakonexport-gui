# ROADMAP — Trident native FTS engine: v0.20.0 → v0.40.0

> **AI-drafted 2026-10-28** (feature-ideation subagent — thinking, not implementing).
> Supersedes the v0.19.0-era "next increment" notes. v0.19.0 is shipped (exact
> leaf→.vt stem-join: `trident_vt.rs::VtRecordStream`/`VmHeader`,
> `trident_voc.rs::VocParams`, `trident.rs::latin_candidates`/`exact_postings`),
> so the plan starts at v0.20.0.
>
> 21 major versions (v0.20.0–v0.40.0) carrying the **30-task flat checklist**
> at the end (the requested 30–40 granularity lives there); v0.41+ sketched
> under "Beyond".
>
> **Standing rules every version must satisfy** (from the CHANGELOG header):
> 1. Bilingual UA+EN: CHANGELOG entry before publishing + `changes.md` (UA+EN)
>    inside `releases/<v>/`.
> 2. Real-file test asserts against `C:\Zakon.Local\Baza\SOFT\` (skip-guarded
>    when the base is absent).
> 3. New/updated CLI verbs; console-only publishing (GUI features ride in the
>    GUI build only).
> 4. guitest baseline `rows=430` unchanged — new UI must not add self-test rows;
>    an explicit baseline bump is called out in the changelog.
> 5. Clean-room: repo code names the engine by file (`lmsup32.dll` /
>    `lmsup.dll`), not by legacy app/task narrative.
>
> **Ordering principle** — cheap wins + the doc-number join payoff first
> (v0.20–v0.25), rus generalization + per-word metadata (v0.26–v0.28), the
> B-tree byte-VM (v0.29–v0.33), `.g` edge semantics + exact Latin mapping
> (v0.34–v0.35), legacy-engine parity (v0.36), GUI/export-facing (v0.37–v0.39),
> hardening (v0.40).
>
> **New evidence gathered for this plan** (byte dumps, 2026-10-28, `trident/`):
> - `lmur.m` (0x3DB2): the u32 table @0x40 is **16 entries, not 8**:
>   `[0x1000, 0x88, 0xBCB, 0x726, 0xA1, 0x206D, 0x244B, 0x29D7, 0x3000, 0x339D, 0x36EF, 0x37CB, 0x39C9, 0xBF3, 0xBFC, 0x1405]`
>   (+ u32s 0x205A/0x2066 @0x80/0x84, dual `0x00FFE703` @0x88, u16 param
>   region @0x8C, hdr+0x100 = `0x00FFE700`… see per-version dumps);
>   hdr+0x6C (u16 **word table**) = **0x37CB**, hdr+0x54 (u16 **predicate
>   table**) = **0x206D**.
> - `lmur.m` tail @hdr+0x3C=0x70E is exactly the `FUN_100134d0` tail shape:
>   `u8 N=3` + 3× `[pattern NUL][u16 list 0x0000-terminated]`:
>   `"R"→[1]`, `"N*"→[0x1C,0x3E]`, `"*"→[0x3E,0x14]`; the walk lands at
>   **0x726** = table[3] (the next slot). Tail patterns are Latin index
>   alphabet (wildcard classes of `FUN_100135d0`).
> - `lmru.m` (0x35CA): tail 0x53E; table16
>   `[0xC20, 0x88, 0xBD8, 0x56B, 0x99, 0x1ACF, 0x2000, 0x25C1, 0x28AD, 0x2C0F, 0x2F17, 0x3001, 0x3203, 0xC09, 0xC12, 0xF5D]`;
>   word table 0x3001, predicate table 0x1ACF.
> - `lmru.vt`: **14421 records** in 0x40..0xD2648 (file 0xD264C) — the rus
>   stream walks exactly, like the ukr 14160/0xCCDE5.
> - `lmu.use` / `lmr.use` (204 B each): vendor@0x00, name@0x18
>   (`lmU.use`/`lmR.use`), `FREE`@0x38, u32@0x3C = **0**, zero body — an
>   *empty* FREE container, not a flag stream.
> - `lmsup.dll` is a **PE32** (machine 0x446, 179520 B), not 16-bit;
>   `lmsup32.dll` (200704 B) carries exactly 4 API strings:
>   `LMaster_Begin/End/Lang/Tran`; `lmsup.dll` has **zero** `LMaster_*`
>   identifiers (different legacy surface). `dump_dll_exports.cjs` is
>   broken for both (prints `functions: 0 named: 16384` — garbage RVA walk).
> - `FUN_10002320.c` / `FUN_10002390.c` / `FUN_100044f0.c` extracted today.
>   `FUN_10002b70` child-id classes: <3000 → `FUN_10002620` node resolve;
>   3000..0xC1C = virtual children (+0x48) under flags 0x1000/0x4000;
>   node 0x40 flag → `FUN_10002fc0`/`FUN_100028e0`; alias tables
>   `DAT_1002eca2..eca5`.

## v0.20.0 — `.m` full header: 16-entry table + tail "N" section (the join substrate)

**Pitch.** Blocked item #1 (u16 word/predicate table locations) is only half
blocked: in the `.m` files the tables sit at header fields (0x6C/0x54) and the
tail "N" section is a small decodable table. Ship the full `.m` header + tail
as the substrate for the doc-number join.

**Evidence.**
- `FUN_10015340.c` — the `.m` loader: seeks hdr+0x4C/0x50/0x54/0x5C/0x6C/0x70/
  0x74/0x78/**0x100** (the 0x58-byte context `DAT_1002f1f0`); opens
  `lm{lang}.m` ('R'), `lm{lang}.uvt` ('W'), `lm{lang}.vt` ('R').
- `FUN_100134d0.c` — tail = seek(hdr+0x3C) → `u8 N` → N ×
  `[pattern NUL][u16 list 0x0000-terminated]`, matched via `FUN_100135d0`,
  output = u16 word-table indices.
- Byte dumps (done): lmur.m 0x40..0x110, lmru.m 0x40..0x110. To pin:
  `node -e` dumps of lmur.m/lmru.m 0x80..0x110 (the 0x80/0x84 u32s, the 0x88
  dual, the 0x8C u16 params, hdr+0x100 semantics) + the tail u16-list length
  distribution.

**Rust surface.** `trident_vt.rs`: `VmHeader::parse(bytes, 16)` →
`table: Vec<u32>` (16 slots) + `hdr_100: u32` + `params: Vec<u16>` (@0x8C,
width pinned by the dump); new `VmTail::parse(bytes, tail_off)` →
`N`, `Vec<VmTailRec { pattern: String, word_idxs: Vec<u16> }>`, `landed`
(must equal the next table slot: 0x726 for lmur.m). `trident.rs::open`
switches the current `VmHeader::parse(m, 8)` call to 16.

**Effort:** S–M.

**Definition of done.** Real-file asserts: lmur.m table16 == the cited values,
tail N=3 with the three cited records, landing 0x726; lmru.m table16 + tail
@0x53E (asserts, counts free to be pinned during the dumps); `trident` verb
line `lmur.m: … tail N=3 @0x70e (→0x726) · word-table @0x37cb · predicate
@0x206d`; docs + release per the standing rules; guitest rows=430.

## v0.21.0 — `.use` decode + `trident dump` helper verb

**Pitch.** Both `.use` files are 204-byte *empty* FREE containers (tail=0,
zero body) — document that fact (they are not a usage-flag stream in this
base) and ship a hexdump verb that accelerates every future RE step.

**Evidence.** Dumps done (204 B, both languages, byte-identical layout).
`node -e` scan of `lmsup32_all.c` for the `.use` consumer (which
`LMaster_*` path opens `lm%s.use`; if none → reserved/stub file — say so in
the docs). Also document the `.uvt` fact from `FUN_10015340` (engine opens
`lm{lang}.uvt` in 'W' mode; absent in this base → update cache is reserved,
never written).

**Rust surface.** `trident.rs::UseFile::parse` (204 B: vendor/name/FREE/
u32@0x3C, zero-body assert) + `TridentIndex.use_files: Vec<UseFile>`;
`cli.rs` sub-verb `trident dump <file> [off] [len]` (hex + latin1, console-only).

**Effort:** S.

**Definition of done.** Real-file asserts for lmu.use/lmr.use (204 B,
tail=0, zero body, name fields), trident verb line `lmu.use: empty FREE
container (tail=0, 204 B)`; `trident dump` smoke sample in the changelog;
docs; guitest rows=430.

## v0.22.0 — `FUN_100135d0` wildcard matcher + tail join (word → u16 word idx)

**Pitch.** The DLL tail matcher is a 4-class wildcard grammar; implementing it
in Rust completes the word→u16-word-index join, so tsearch output stops
being raw offsets and starts being real join evidence.

**Evidence.** `FUN_100135d0.c`: `'*'` = any-string (length-bounded),
`'N'` = casefold char (`FUN_10021b16`), `'R'` = separator class
(`- ; : & ,`) or single-char word, `'L'` = class/length predicate
(`FUN_10011580 & 0x40`), default = casefold char (`FUN_10011930(-0x80)`).
Real anchors: «не»→`NE` matches `"N*"` → idx `[0x1C, 0x3E]`; any word
matches `"*"`.

**Rust surface.** `trident_vt.rs::match_pattern(latin_word, pattern) -> bool`
+ `VmTail::word_indices(term_latin) -> Vec<u16>`; `trident.rs::TridentResult`
gains `word_idxs: Vec<u16>`; tsearch prints the join.

**Effort:** S–M.

**Definition of done.** Synthetic unit tests per wildcard class + real:
`idx.search("не")` reports `word_idxs=[0x1C, 0x3E]` (via "N*"); a catch-all
`"*"` contributes its list for every term (documented); docs; guitest
rows=430.

## v0.23.0 — u16 word/predicate tables: structural decode

**Pitch.** The two u16 tables the join core (`FUN_100176c0`/`FUN_10015cd0`)
consumes are plain u16 offset arrays inside the `.m` — decode their
structure, and the nested predicate chains become walkable.

**Evidence.** Dumps done: lmur.m 0x37CB = ascending u16 record offsets
(0x00FE, 0x0100, 0x0102, … step 2 — a record pool in the same section);
0x206D = ascending u16 offsets (0x013E, 0x0154, 0x0164, 0x0170, …).
`FUN_100176c0.c`: `table[idx]` → record → kind byte (`0xFA` special →
flag|8; else predicate lookup via `FUN_10015cd0`); `0x33` = nested
`FUN_100177d0(u8,u8)` chains with skip-bytes < 0x20. To pin: dump the
record pool (0x37CB+0xFE…) to fix record layout (kind + operand widths);
dump lmru.m 0x3001/0x1ACF for the same shape.

**Rust surface.** `trident_vt.rs::VmWordTable { base: u32, offsets: Vec<u16>,
records: Vec<VmWordRec { kind: u8, ops: Vec<u16> }> }` +
`VmPredicateTable { base, offsets }`; `TridentIndex` carries
`word_table: Option<VmWordTable>` / `pred_table: Option<VmPredicateTable>`.

**Effort:** M.

**Definition of done.** Real-file asserts (first 8 offsets of both languages,
kind-byte histogram over the first 64 records), trident verb fingerprint
line (table sizes + kind distribution); docs (UA+EN diagram of the join
core); guitest rows=430.
## v0.24.0 — **PAYOFF: doc-number join (native u16 doc-ids ↔ current base docnums)**

**Pitch.** The actual payoff: map the `.vt` stream's u16 doc-ids to the
current base's docnums, so tsearch reports documents the user can open with
the existing `fields`/`extract` verbs.

**Evidence (three probes, in order — stop at the first that holds).**
1. `node -e`: max/percentiles of the u16 doc-ids over all 14160 lmur.vt
   records vs the current base doc count (via `db.rs` `file_docs`/`span`
   path, or a `node -e` over the `.mb` index file). If max u16 ≈ doc count →
   the u16s *are* the current docnums (identity join — cheapest outcome).
2. Otherwise, probe for a mapping table: `lmur.m` hdr+0x5C section
   (0x29D7) + the hdr+0x100 dual (`0x00FFE700`) + lmru.m analogues —
   `node -e` hex dumps; a monotone u32/u16 index table is the candidate.
3. Oracle either way: 20 real queries, ftsearch (`.mb` scan) vs tsearch doc
   sets; record the agreement rate in the changelog.

**Rust surface.** New `src/trident_join.rs`:
`DocJoin::probe(idx) -> JoinKind { Identity, Table(u32 section) }` +
`join_docs(term, u16s) -> Vec<DocHit { docnum, term, evidence }>`.
`TridentResult.docs` becomes `Vec<DocHit>` (raw u16 kept as evidence);
tsearch prints joined docnums.

**Effort:** L.

**Definition of done.** tsearch prints joined docnums; 5 spot-checks via
the `fields <docnum>` verb (titles/contexts match the query); agreement
rate documented (UA+EN); tsearch verb + docs updated; guitest rows=430.

## v0.25.0 — tsearch v2: joined docnums as the primary evidence

**Pitch.** Retire the proportional `posting_slice` heuristic (fallback only,
clearly labelled, for terms the exact join cannot resolve), add an
`--oracle` cross-check against ftsearch, and demote «експеримент» where the
v0.24.0 agreement justifies it.

**Evidence.** v0.24.0 join results; `ftsearch.rs::FtHit { file, offset }`
shape for the oracle diff.

**Rust surface.** `trident.rs::search` restructured: (1) exact stem join
(Latin candidate → `.vt` record → u16 doc-ids → join), (2) tail word-idx →
word table → predicate chain, (3) proportional fallback only when (1)+(2)
are empty (labelled "оцінка"). CLI: `tsearch --oracle` prints per-term
`tsearch ∩ ftsearch` counts; verb label "оракельно підтверджено" when
agreement ≥ 0.9, otherwise "експеримент (пошук)".

**Effort:** M.

**Definition of done.** No unlabeled heuristic numbers in tsearch output;
`--oracle` runs on the 20-query corpus; docs + release; guitest rows=430.

## v0.26.0 — rus generalization: language-generic loader + `tsearch --lang`

**Pitch.** Everything so far is Ukrainian-primary; the rus side (`lmr.*`)
uses the same formats — generalize the loader so Russian-base search works
for free.

**Evidence.** Dumps done: lmru.vt (14421 records, exact walk to 0xD2648),
lmru.m (tail 0x53E, table16, word table 0x3001, predicate 0x1ACF), lmr.g
(already in `g_graph` tests: template `lmr.gt`, head
`[2, 0x1C, 2, 0xC, 1]`). To pin: `Morphology::parse` record counts on
lmr.voc; rus query encoding via `FoldTables.rus` (32 pairs, real).

**Rust surface.** `trident.rs`: `struct LangIndex` (the per-language
subset of today's `TridentIndex` fields) + `TridentIndex { ukr:
Option<LangIndex>, rus: Option<LangIndex> }` (or `open_lang(baza, lang)`);
`tsearch --lang u|r|auto` (auto = both, merged, labelled); rus encoding
through `fold.rus`.

**Effort:** M.

**Definition of done.** Rus real-file asserts (14421 records, tail landing,
table16 values, lmru.m tail), tsearch `--lang r` «закон» prints joined
records, trident verb lists both languages; docs; guitest rows=430.

## v0.27.0 — `.m` section walker + 0xE per-word rows + field-name registry

**Pitch.** Walk every walkable `.m` section with the proven
`FUN_10004230` advance rule (WALK_BLOCK=4096), decode the ~1143 NUL
records of the 0x1000 section (≈ the 1002-word dictionary), the 0x88
field-name registry, and the 0xE per-word rows the predicates consume.

**Evidence.** `probe_m*.cjs` (0x88 = u16-counted NUL-terminated string
section — field names); `FUN_100177d0.c` row offsets (+0x51, +0xA4, +0xDD,
+0xD7, +0xD8, +0xD9, +200; prev-row walk via `DAT_1002f40a`). To pin:
`node -e` dumps of lmur.m 0x1000 section head (count + first 16 records),
0x88 registry strings decoded via `lm_word_display`, 0x29D7 / 0x205A /
0x2066 shape classification.

**Rust surface.** `trident_vt.rs`: `VmSectionWalk` (reuse
`trident_voc::walk_section_records`, WALK_BLOCK=4096, over the 16-slot
table), `VmWordRows { rows, flags_d7_d8_d9 }`, `VmFieldRegistry:
Vec<String>`.

**Effort:** M–L.

**Definition of done.** Every walkable `.m` section lands on the next table
slot (asserts); 0x1000 record count vs `dict_words` (1002) — the delta
documented; 0x88 registry decodes to plausible field names; 0xD7/0xD8/
0xD9 flag histogram in docs; trident verb line; guitest rows=430.

## v0.28.0 — `FUN_100177d0` predicate catalog (22 opcodes → flag semantics)

**Pitch.** Document the 22-opcode predicate table as a UA+EN reference and
implement the evaluable subset (flag set/clear, field checks, row walks)
against the v0.27.0 rows.

**Evidence.** `FUN_100177d0.c` arms: `2→|0xC`, `0xC→|0x28`, `8→|2`,
`0xD→|0x40`, `0x16→|4`, `6→(+0xDD, flag+8)`, `0x19→(0xD7&2)`,
`0x1D→!(0xD7&2)`, `0x10→|2`, `0→FUN_10011600(+0xDD)&0x80→|0x80`,
`1→FUN_10011580&0x40→|1`, `5→next-row Δ(0xD9)==1`, `0x23→prev-row
0xD9 <`, `0x24→+200==0`, `7→!(0xD7&8)`, `0xB→!(0xD7&0x10)`,
`0x14→0xD8&1`, `0x17→+0x51!=0`, `0x18→prev-chain walk (+0x51)`,
`0x1C→next-chain walk`, `0x1E→+0xA4→*[0]+0x4C!=0`, `0x1B→|8`.

**Rust surface.** `trident_vt.rs::PredOp` (22-variant enum) +
`evaluate(op, row, prev_row, next_row) -> bool` (rows from v0.27.0; the
`FUN_10011580`/`FUN_10011600` arms stay documented as unresolved accessors
until the `.g`/template decode).

**Effort:** M.

**Definition of done.** Catalog table (opcode → semantics, UA+EN) in docs;
unit tests mirroring every decompiled arm on synthetic rows; trident verb
reports the catalog size; guitest rows=430.
## v0.29.0 — B-tree byte-VM core: `FUN_100019f0` key scan + stops

**Pitch.** The big VM, part 1: the core loop (key bytes ≤0x3F through the
casefold tables, `0x40/0x41/0x42/0x45/0x48/0x49/0xFF` ops, `*param_4`
counter) — the skeleton that runs `.vt` record payloads.

**Evidence.** `FUN_100019f0.c` (full read: loop `LAB_10001a0f`, switch
0x40..0x4C, backtrack `LAB_10001c37`/`LAB_10001c5d` with the ±5 key-shift
windows, second pool `joined_r0x10001e71` via `DAT_1002db62` +
`FUN_100044c0`). Identify `DAT_1002e362`/`DAT_1002e642` init: xref
store-scan of the two globals (an `xrefs_ad98.py`-style byte scan for
`C7 05`/`89 0D`/`A3` forms) to confirm the source (lm.a sections vs `.g`
graph vs `.data` image init — if image init, dump straight from the DLL).
Byte-dump 5 real `.vt` record payloads as synthetic op fixtures.

**Rust surface.** New `src/trident_vm.rs`: `VmCtx { instance, depth,
off_counter, out: Vec<u8> }`, `run_scan(payload) -> VmScan { key, next_off,
counter }` (ops ≤0x3F casefold, 0x40, 0x41 via the `FUN_100018b0` loop,
0x42/0x49/0xFF, 0x45, 0x48); casefold tables as `&[u8; 128]`-style
fixtures.

**Effort:** L.

**Definition of done.** Synthetic tests per op (from the decompiled arms)
+ the 14160-record `.vt` stream runs without panic, counter ≥ 1 on ≥ 90 %
of records (the rate is recorded); docs (VM op table, UA+EN); guitest
rows=430.

## v0.30.0 — VM descent + node pool + affix ops

**Pitch.** Big VM part 2: `0x43` descent through the `DAT_1002db56` node
pool, `0x44` case-swap, `0x46/0x47` affix ops, `0x4A/0x4B/0x4C`, and the
second pool — the VM can then walk a `.vt` B-tree payload end-to-end.

**Evidence.** `FUN_100019f0.c` arms 0x43/0x44/0x46/0x47/0x4A/0x4B/0x4C;
`FUN_10002320.c` (extracted: u16 mask at `instance+0x76 +
(fold(b)&0x3F)*2`; `'G'` = skip if the mask bit is set, `'F'` = skip if
clear); `FUN_10002390.c` (extracted: skip resolver — `pool[child+1]` =
child byte length, `DAT_1002dac0` advance); `FUN_10001920.c` (pool
resolution, `DAT_1002dad0` latch); `FUN_100044c0.c` (extract: the second-
pool record scan). Pin: where the node pool lives in lmur.vt (`node -e`
from the loader's seek list; pool start = u32 array after/before the
record stream).

**Rust surface.** `trident_vm.rs`: `NodePool { ptrs: Vec<u32> }`,
`descend(ctx, child)`, `affix_op(ctx, op, operand)`, `run_record(payload)
-> VmRecord { keys, leaf, counter }`.

**Effort:** L.

**Definition of done.** Full op-coverage synthetic tests; the full 14160-
record stream runs with no panic and ≥ 90 % non-empty key extraction;
docs (op semantics incl. the backtrack ±5 windows, UA+EN); guitest
rows=430.

## v0.31.0 — `.voc` root B-tree descent (search chain, leaf → `.vt` join)

**Pitch.** The search chain open since v0.15.0: walk the `lmu.voc` root
B-tree (+0x3C) to a leaf and join the leaf key to a `.vt` record stream
offset — a word→posting link with no Latin-candidate ambiguity.

**Evidence.** `FUN_10002ff0.c` (seek to the root @handle+0x3C);
`FUN_10002b70.c` (casefold descent; node u16 `0x80` flag = child list,
count &0x7F; child <3000 → `FUN_10002620` node resolve; 3000..0xC1C
virtual children (+0x48) under flags 0x1000/0x4000 → recursive
`FUN_10002ff0`; node `0x40` flag → `FUN_10002fc0`/`FUN_100028e0` —
extract `FUN_10002fc0.c` + `FUN_100028e0.c`; alias tables
`DAT_1002eca2..eca5` — dump their init); `FUN_10002620.c` +
`FUN_10001920.c` (pool[child]+2 key walk).

**Rust surface.** `trident_voc.rs::VocBTree { root: u32, pool: Vec<u32> }
::find(word) -> Option<VocLeaf { key, vt_off }>`.

**Effort:** L.

**Definition of done.** «конституція» (absent from the raw dict scan,
confirmed in v0.15.0) resolves via the B-tree; the leaf `vt_off` lands
inside the `.vt` record stream; real-file asserts; docs; guitest
rows=430.

## v0.32.0 — listA/listB dense-node payload semantics

**Pitch.** Decode the actual node payloads in the 4096 resync slots
(0x69000..0x77000) that the v0.18.x walker resynchronizes onto, and
reconcile the 722/2157 record counts.

**Evidence.** Byte dumps: slot heads 0x69000/0x6A000/…/0x77000 (node u16
header `0x80` flag + count &0x7F, key bytes, child u16); `FUN_10004370.c`
(free/validator — the in-memory layout oracle: NULL-terminated array of
word-struct pointers); cross-check node keys against `dict_words`, the
`.vt` stream, and the v0.27.0 rows.

**Rust surface.** `trident_voc.rs::VocDenseNodes { slots: Vec<Node> }`
(per-slot decode) + `validate() -> (ok, mismatches)`; the in-memory
word-struct layout documented alongside (clean-room, file-referenced).

**Effort:** L.

**Definition of done.** Every node key in the 15 slots is a valid LmCode
word (cross-checked against the dictionary + `.vt`); 722/2157 reconciled
(the delta formula documented, UA+EN); docs; guitest rows=430.

## v0.33.0 — per-word 'f' opcode-VM execution + loader-consumer triage

**Pitch.** Execute the 142 per-word `'f'` programs (0x1001..0x1004) and
close the two remaining loader-consumer gaps (`+0x5C` raw seek, `+0x64`
33-word table).

**Evidence.** `FUN_100044f0.c` (extracted today) + `FUN_10004440.c` +
`FUN_10004100.c`; `FUN_100032d0.c` (the loader's call list — which
consumer reads +0x5C / +0x64); run the 142 lmu.voc programs; invariant:
subtree word-count totals vs the listA/listB counts (722/2157 — or the
exact invariant the run reveals).

**Rust surface.** `trident_voc.rs::run_f_program(rec) -> FResult {
byte_len, word_count, data: Vec<u16> }`; `Morphology` carries an
`f_programs` execution report.

**Effort:** M–L.

**Definition of done.** All 142 programs terminate at the top-level
0x1002; the count invariant documented (UA+EN); +0x5C/+0x64 consumer
notes in module docs; guitest rows=430.
## v0.34.0 — `.g` edge semantics (the graph-template walker)

**Pitch.** Decode the `lmu.g` u16 edge stream (POINT1/POINT2/STEP/REPEAT
template graph) and identify the DLL's graph walker, ending the
"structurally decoded only" status from v0.16.0.

**Evidence.** lmu.g stream (head `2, 0x1C, 2, 0xC, 1, 0xDD0A, 0, 0x14C,
0xF000`, tail markers POINT1/POINT2/STEP/REPEAT), lmr.g (same shape,
template `lmr.gt`); walker candidates to triage first: `FUN_10019060.c` /
`FUN_10019300.c` (4–5 KB, `byte*` params), plus xref scans of
`FUN_10011580`/`FUN_10011600` (the template accessors used inside the
predicates); `probe_g.cjs`/`probe_g2.cjs` + a fresh `node -e` edge-walk
probe over the stream.

**Rust surface.** `g_graph.rs::GVertex`/`GEdge` + `GGraph::walk() ->
(vertices, edges, consumed: usize)`; the 134 `.gw` weights (0x8000 flag)
mapped onto edges.

**Effort:** L–XL.

**Definition of done.** The walker terminates on both real files;
vertex/edge counts documented; `consumed` == stream end (fingerprint);
docs (UA+EN graph diagram); guitest rows=430.

## v0.35.0 — exact Latin mapping (retire the ambiguity)

**Pitch.** The graph normalization of v0.34.0 resolves the н→N|H, в→V|W|B,
о→O|Q ambiguity: replace the capped BFS candidate expansion with a
deterministic exact mapping.

**Evidence.** The v0.34.0 normalization table; corpus: both mappings over
500+ `dict_words` stems; assert the exact candidate hits the `.vt` stream
for 100 % of stems (the v0.19.0 anchors BUV/PERE/SPV/HCH stay valid).

**Rust surface.** `trident.rs::latin_exact(bytes) -> String` (graph-
derived, uncapped); `latin_candidates` kept as a documented fallback for
unmappable letters only; `exact_postings` prefers the exact path.

**Effort:** M.

**Definition of done.** 100 % real-corpus `.vt` hit rate asserted;
`LATIN_VARIANTS` removed or demoted (documented either way); docs (the
confirmed mapping table, UA+EN); guitest rows=430.

## v0.36.0 — legacy-engine parity (`lmsup.dll` vs `lmsup32.dll`)

**Pitch.** Both "engines" are PE32 x86 (machine 0x446) — `lmsup.dll`
(179520 B) is a legacy 32-bit build, not 16-bit. Document the API-surface
delta (`lmsup32.dll`: exactly `LMaster_{Begin,End,Lang,Tran}`;
`lmsup.dll`: zero `LMaster_*`) and repair the broken export-dump helper.

**Evidence.** PE header dumps (done: machine 0x446 both); `node -e`
latin1 scan of `LMaster_\w+` in both images (done: 4 vs 0); repair
`dump_dll_exports.cjs` (both currently print `functions: 0 named: 16384`
— a garbage RVA walk); byte-scan shared table constants (stop markers,
WALK_BLOCK `0x1000`, casefold-table init) across the two `.text`s.

**Rust surface.** None (docs + fixtures); the trident verb gains the line
"lmsup.dll (legacy build, machine 0x446, no LMaster_* exports) ·
lmsup32.dll: LMaster_{Begin,End,Lang,Tran}"; inventory role text
corrected (console-only wording).

**Effort:** S–M.

**Definition of done.** API/constant delta table (UA+EN) in docs; the
repaired export-dump helper (0 exports → a clean "no export table"
report, no crash); docs; guitest rows=430.

## v0.37.0 — GUI: Trident search panel

**Pitch.** A native GUI panel surfacing the tsearch chain (query → LmCode
hex → Latin candidate → `.vt` record offset → joined docnums), reusing the
crate API, without adding self-test rows.

**Evidence.** The v0.25.0 tsearch output shape; the `--guitest` harness
(rows=430 baseline — the panel must not add rows; run the guitest before
and after).

**Rust surface.** `gui.rs`: Trident panel (query field + result list:
term → hex → candidate → record @off → docnums); calls
`crate::trident::TridentIndex` (no engine duplication in the GUI).

**Effort:** M–L.

**Definition of done.** Panel renders on the real base; guitest rows=430
unchanged (asserted in the changelog); bilingual GUI docs; console-only
publishing unchanged.

## v0.38.0 — export native doc ids (`export --trident-docs`)

**Pitch.** The export verb gains a flag that exports the tsearch-joined
docnums (with a native-id evidence column), closing the loop from search
to bundle.

**Evidence.** v0.24.0 join; `writers.rs` bundle shape
(`bundle-<count>-<first docnum>.<ext>`), the `--docs` list flag.

**Rust surface.** `cli.rs` + `writers.rs`: `export <q> --trident-docs`
(query → join → docnums → bundle with the native-id column).

**Effort:** M.

**Definition of done.** A sample run in the changelog (the bundle docnums
match the tsearch output for the same query); docs; guitest rows=430.

## v0.39.0 — `tsearch --fetch` (search → document text in one verb)

**Pitch.** Fetch the joined docnums into readable document text (title +
first paragraph) via the existing `db.rs` docnum path — proof that the
search→docnum→document chain works end-to-end.

**Evidence.** `db.rs::span`/`file_docs` (docnum → `.mb` chunk), the
`fields`/`printdoc` verb code paths; v0.25.0 join docnums.

**Rust surface.** `cli.rs`: `tsearch --fetch [N]` (top-N docnums → title
+ snippet) and `tsearch --print` (GDI printdoc of the joined docs).

**Effort:** M.

**Definition of done.** A «конституція»-class query prints titles that
read correctly for the subject (10-query manual check recorded in the
changelog, UA+EN); docs; guitest rows=430.

## v0.40.0 — hardening + production marking

**Pitch.** A 100-query oracle corpus (full tsearch chain vs ftsearch), an
error surface (`TridentError` instead of silent empties), and marking the
Trident path production (UA+EN).

**Evidence.** A 100-query corpus (UA+EN: statutory terms + proper
nouns); failure-mode table (terms absent from `.vt`, mixed-language
queries, docnum gaps, bases without a SOFT index); a second-base docnum-
stability check when one is available (otherwise the assumption is
documented).

**Rust surface.** `trident.rs::TridentError` enum (no panics on partial
data), error propagation through `search`; README + trident verb status
line: "Trident-маршрут: production (оракельно підтверджено, v0.40.0)".

**Effort:** M.

**Definition of done.** The agreement matrix documented (UA+EN) in the
changelog; zero panics on the corpus; release v0.40.0 (bilingual
CHANGELOG + `changes.md` in `releases/v0.40.0/`, console-only); guitest
rows=430.

### Beyond v0.40.0 (sketches, not scheduled)

- v0.41.0 — `.uvt` behavior: the engine's write-mode per-language update
  cache (absent in our base — synthesize on a base copy).
- v0.42.0 — Rust `LMaster_*` driver: load `lmsup32.dll` through an FFI
  facade and cross-check the DLL join against the Rust join on the
  100-query corpus.
- v0.43.0 — GUI: native-id export button (panel → the `--trident-docs`
  path).
- v0.44.0 — per-base index-rebuild tooling (write `.vt`/`.m` from the
  `.mb` scan).
- v0.45.0 — multi-base support (docnum mapping across bases, the
  `update.rs` interplay).

---

## Tasks (flat, ordered by priority)

30 items across v0.20.0–v0.40.0. Every item inherits the standing rules
(bilingual docs + `changes.md`, real-file test asserts, CLI verbs where
named, guitest baseline `rows=430` unchanged, console-only publishing,
clean-room wording).

- [ ] v0.20.0: extend `VmHeader` to the 16-entry table + `hdr_100` + u16 params @0x8C; real-file asserts (lmur.m/lmru.m table16)
- [ ] v0.20.0: `VmTail::parse` — the tail "N" section (lmur.m: N=3, "R"→[1], "N*"→[0x1C,0x3E], "*"→[0x3E,0x14], landing 0x726); trident verb line
- [ ] v0.21.0: `UseFile::parse` (204 B FREE, tail=0, zero body, both languages) + trident verb line
- [ ] v0.21.0: `trident dump <file> [off] [len]` hexdump sub-verb (console-only)
- [ ] v0.21.0: document the `.uvt` fact (engine opens `lm{lang}.uvt` 'W'-mode; absent in this base)
- [ ] v0.22.0: `FUN_100135d0` wildcard matcher in Rust (`*`/N/R/L + casefold default)
- [ ] v0.22.0: `VmTail::word_indices` — Latin candidate → tail pattern → u16 word idx («не»→NE→"N*"→[0x1C,0x3E] real assert)
- [ ] v0.23.0: `VmWordTable` (hdr+0x6C u16 offset array → record kind 0xFA/0x33) + `VmPredicateTable` (hdr+0x54)
- [ ] v0.23.0: trident verb table fingerprint (sizes + kind-byte histogram)
- [ ] v0.24.0: doc-number join (PAYOFF): u16 doc-id range vs base doc count + `.m` 0x29D7/0x100 mapping probe + 20-query ftsearch oracle
- [ ] v0.24.0: `TridentResult.docs` → `Vec<DocHit>` (joined docnums + u16 evidence)
- [ ] v0.25.0: tsearch v2 — join primary, `posting_slice` fallback-only, `--oracle` flag, label demotion
- [ ] v0.26.0: rus generalization — load lmr.voc / lmru.vt (14421 rec, tail 0xD2648) / lmru.m / lmr.g; `tsearch --lang u|r|auto`
- [ ] v0.26.0: rus real-file asserts + trident verb dual-language output
- [ ] v0.27.0: `.m` section walker (16-slot table, WALK_BLOCK 4096; every walkable slot lands on the next slot)
- [ ] v0.27.0: 0x1000 per-word NUL records + 0x88 field-name registry + 0xE row structs (0xD7/0xD8/0xD9 flag histogram)
- [ ] v0.28.0: 22-opcode `FUN_100177d0` catalog (UA+EN) + `evaluate(op,row)` tests on synthetic rows
- [ ] v0.29.0: `trident_vm.rs` VM core (key scan ≤0x3F, 0x40/0x41/0x42/0x45/0x48/0x49/0xFF, *param_4 counter) + `DAT_1002e362`/`DAT_1002e642` init xref
- [ ] v0.30.0: VM descent (0x43 node pool `DAT_1002db56`, 0x44, 0x46/0x47 via `FUN_10002320` mask, 0x4A/0x4B/0x4C, pool 2 `DAT_1002db62` + `FUN_100044c0`)
- [ ] v0.31.0: `VocBTree::find` — root @+0x3C descent (FUN_10002ff0→2b70→2620/1920), leaf → `.vt` record offset (the open leaf→.vt join)
- [ ] v0.32.0: listA/listB dense-node payload decode (slots 0x69000..0x77000; 722/2157 reconciled) + `FUN_10004370` validator oracle
- [ ] v0.33.0: 'f' opcode-VM execution (142 programs, word-count invariant) + `+0x5C`/`+0x64` consumer triage
- [ ] v0.34.0: `.g` edge semantics — graph walker (POINT1/POINT2/STEP/REPEAT) + `.gw` edge weights; triage `FUN_10019060`/`FUN_10019300`
- [ ] v0.35.0: exact Latin mapping (graph-derived) replaces the `LATIN_VARIANTS` BFS; 100 % corpus `.vt`-hit assert
- [ ] v0.36.0: legacy parity — PE32/0x446 both, `LMaster_*` 4 vs 0, repair `dump_dll_exports.cjs`, shared-constant scan
- [ ] v0.37.0: GUI Trident search panel (`gui.rs`), guitest rows=430 unchanged
- [ ] v0.38.0: `export --trident-docs` (writers.rs: joined docnums into the bundle)
- [ ] v0.39.0: `tsearch --fetch [N]` / `--print` (db.rs docnum → document text)
- [ ] v0.40.0: 100-query oracle corpus + `TridentError` surface + production marking (release)
- [ ] v0.40.0: second-base docnum-stability check (or the assumption documented when no second base exists)
