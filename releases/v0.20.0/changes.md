# v0.20.0 - Changes / Зміни (2026-10-28)

## UK / Українська

- **Tail-секція `.vt` декодована — і вона ПОРОЖНЯ в old-format
  файлах.** Нова `VtTailScan` (порт `FUN_100134d0`: у хвісті `u8
  N` + N записів `[NUL-термінований рядок][u16-список, 0x0000]`):
  на реальному `lmur.vt`/`lmru.vt` **N = 0** — tail-покажець
  лежить всередині останніх 4 байтів файлу (сам u32 LE).
  Висновок: u16 word/predicate-таблиці, яких потребує join-ядро
  (`ctx+0x30`/`ctx+0x44` з `FUN_100176c0`/`FUN_10015cd0`), DLL
  read-mode виводить **з B-tree `.voc`**, а не з хвоста `.vt` —
  наступний RE-крок (v0.21+).
- **Wildcard-матчер `tail_match` портовано** (порт
  `FUN_100135d0`):
  - `*` — зірки їдуть на `L−M` (L = довжина рештка subject, M =
    довжина рештка pattern; fail при `L<M`) — збережена
    decompiled-логіка з `lmsup32.dll`;
  - `N` — будь-який байт;
  - `R` — клас розділювачів `{; : & ,}` + quirk `-` (приймається
    лише як 1-байтний решток) — збережено з декомпиляції;
  - `L` — літерний байт (приблизно ASCII/Cyrillic; точна
    property-таблиця `FUN_10011580` — окрема екстракція);
  - інший байт pattern — case-folded рівність (фолдяться обидва
    боки — надмножина DLL-поведення, коли ключі вже uppercase).
  Синтетичні тести на кожну семантику.
- **`.use` usage-прапорці декодовано** (нова `UseFile`):
  `lmu.use`/`lmr.use` (по 204 Б) = той самий 64B-заголовок +
  **140-байтовий нульовий body** — usage-прапорці в цій базі не
  ввімкнені. CLI виводить стан.
- **Руська база тепер завантажується** (поля `TridentIndex`):
  - `lmru.vt` → `VtRecordStream`: **14421 record-ів**
    `0x40..0xd2648`, walk приходить точно в хвіст; руські
    case-folded стеблі в тому ж Latin-алфавіті (`NET`, `PER`,
    `PO` підтверджені в потоці);
  - `lmru.m` → `VmHeader`: u32-таблиця `0xC20, 0x88, 0xBD8, 0x56B,
    0x99, 0x1ACF, 0x2000, 0x25C1`, хвіст @0x53E;
  - руський join (словник `lmr.voc` + rus-варіант `tsearch`) —
    далі.
- **CLI `trident`** — нові блоки:
  - `lmru.vt: lmRU.vt — rus posting record stream: 14421 record-ів
    0x40..0xd2648`;
  - `lmru.m: lmRU.m — rus метадані · u32-таблиця @0x40 (8) · хвіст
    @0x53e`;
  - `lmU.use: 204 Б (body 140 Б, усі 0 — usage-прапорці (не
    ввімкнені в цій базі))`;
  - `tail-scan (FUN_100134d0) @0xccde5: N=0 (old-format .vt —
    хвіст порожній; u16 word/predicate-таблиці виводяться з .voc
    B-tree — далі (v0.21+))`.
- **130 юніт-тестів** (нові: `VtTailScan` реальні/синтетичні,
  `tail_match` wildcard-семантика, `UseFile`, rus-стріми
  `lmru.vt`/`lmru.m`). Guitest baseline `rows=430` без змін.

## EN / English

- **The `.vt` tail section decoded — and it is EMPTY in the
  old-format files.** New `VtTailScan` (a port of `FUN_100134d0`: a
  `u8 N` at the tail + `N` `[NUL-terminated string][u16 list,
  0x0000]` records): on the real `lmur.vt`/`lmru.vt` **N = 0** —
  the tail pointer sits inside the last 4 file bytes (the LE u32
  itself). Conclusion: the u16 word/predicate tables the join
  core needs (`ctx+0x30`/`ctx+0x44` per `FUN_100176c0` /
  `FUN_10015cd0`) are derived by the DLL read-mode **from the
  `.voc` B-tree, not the `.vt` tail** — the next RE step
  (v0.21+).
- **The wildcard matcher `tail_match` is ported** (a port of
  `FUN_100135d0`):
  - `*` — the stars advance by `L−M` (L = the subject remainder,
    M = the pattern remainder; fail when `L<M`) — the decompiled
    logic preserved;
  - `N` — any byte;
  - `R` — the separator class `{; : & ,}` + the `-` quirk
    (accepted only as a 1-byte remainder) — preserved from the
    decompilation;
  - `L` — a letter byte (approximated as ASCII/Cyrillic; the
    exact `FUN_10011580` property table is a separate
    extraction);
  - any other pattern byte — a case-folded equality (both sides
    folded — a superset of the DLL behavior when the keys are
    already upper-case).
  Synthetic tests over every semantic.
- **The `.use` usage flags decoded** (new `UseFile`): `lmu.use` /
  `lmr.use` (204 B each) = the same 64-byte header + an
  **all-zero 140-byte body** — the usage flags are not enabled in
  this base. The CLI reports the state.
- **The Russian base now loads** (`TridentIndex` fields):
  - `lmru.vt` → `VtRecordStream`: **14421 records**
    `0x40..0xd2648`, the walk lands exactly on the tail; the
    Russian case-folded stems share the Latin index alphabet
    (`NET`, `PER`, `PO` confirmed in the stream);
  - `lmru.m` → `VmHeader`: the u32 table `0xC20, 0x88, 0xBD8,
    0x56B, 0x99, 0x1ACF, 0x2000, 0x25C1`, tail @0x53E;
  - the Russian join (the `lmr.voc` dictionary + a rus `tsearch`)
    comes next.
- **CLI `trident`** — new blocks:
  - `lmru.vt: lmRU.vt — rus posting record stream: 14421 records
    0x40..0xd2648`;
  - `lmru.m: lmRU.m — rus metadata · u32 table @0x40 (8) · tail
    @0x53e`;
  - `lmU.use: 204 B (body 140 B, all zero — the usage flags are
    not enabled in this base)`;
  - `tail-scan (FUN_100134d0) @0xccde5: N=0 (old-format .vt — the
    tail is empty; the u16 word/predicate tables come from the
    .voc B-tree — next (v0.21+))`.
- **130 unit tests** (new: `VtTailScan` real/synthetic,
  `tail_match` wildcard semantics, `UseFile`, the Russian streams
  `lmru.vt`/`lmru.m`). The guitest baseline `rows=430` is
  unchanged.
