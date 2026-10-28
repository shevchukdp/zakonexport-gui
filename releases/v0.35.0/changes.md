# v0.35.0 - Changes / Зміни (2026-10-28)

## UK / Українська

- **Legacy-engine parity (roadmap v0.36.0).** `lmsup.dll` vs `lmsup32.dll` —
  уточнення roadmap-заяви «обидва PE32 x86 (0x446)» реальним
  image-format-dump'ом:
  - **`lmsup.dll` (179520 B) = NE (New Executive, 16-біт)** образ (сигнатура
    `NE`, NE machine `0x446`) — legacy Windows 3.x/9x рушій, **без
    `LMaster_*`** експортів (0 рядків `LMaster_`). Це НЕ «legacy 32-біт»
    (як було в roadmap) — це справжній 16-біт NE.
  - **`lmsup32.dll` (200704 B) = PE32 x86 (I386, `0x14c`)** — сучасний
    32-біт рушій з точним API `LMaster_{Begin,End,Lang,Tran}` (4 іменних
    експорти, підтверджено відремонтованим `dump_dll_exports.cjs`).
  - **Відремонтовано `dump_dll_exports.cjs`:** оригінал читав
    `NumberSections` з optional header (garbage-RVA walk) і двічі
    застосовував `rva2off` до name-table (лічив file offset як RVA) → обидва
    DLL друкували `functions: 0 named: 16384`. Тепер: детекція NE vs PE,
    bounds-checked розбір export directory (0 exports → чистий «no export
    table», без crash) + надійний latin1-скан `LMaster_\w+` (4 vs 0).
  - **Коректний inventory role text** + рядок `trident` + API/constant
    delta-таблиця (UA+EN) у модуль-доках.
- **189 юніт-тестів** (новий 1: `legacy_engine_parity_ne_vs_pe32_dlls`).
  Guitest baseline `rows=430` без змін. Версія **0.34.0 → 0.35.0**.
- Далі (v0.36.0 / roadmap v0.37.0): GUI Trident search panel.

## EN / English

- **Legacy-engine parity (roadmap v0.36.0).** `lmsup.dll` vs `lmsup32.dll` —
  the roadmap's "both PE32 x86 (0x446)" claim is refined by a real
  image-format dump:
  - **`lmsup.dll` (179520 B) = an NE (New Executive, 16-bit) image** (sig
    `NE`, NE machine `0x446`) — the legacy Windows 3.x/9x engine, **no
    `LMaster_*` exports** (0 `LMaster_` strings). It is NOT the "legacy
    32-bit" build the roadmap drafted — it is a genuine 16-bit NE image.
  - **`lmsup32.dll` (200704 B) = PE32 x86 (I386, `0x14c`)** — the current
    32-bit engine with the exact API `LMaster_{Begin,End,Lang,Tran}` (4
    named exports, confirmed by the repaired `dump_dll_exports.cjs`).
  - **Repaired `dump_dll_exports.cjs`:** the original read `NumberSections`
    from the optional header (a garbage-RVA walk) and double-applied
    `rva2off` to the name table (treating a file offset as an RVA) → both
    DLLs printed `functions: 0 named: 16384`. Now: NE-vs-PE detection, a
    bounds-checked export-directory parse (0 exports → a clean "no export
    table" report, no crash) + a reliable `LMaster_\w+` latin1 string scan
    (4 vs 0).
  - **Corrected `INVENTORY` role text** + a `trident` line + the API/constant
    delta table (UA+EN) in the module docs.
- **189 unit tests** (1 new: `legacy_engine_parity_ne_vs_pe32_dlls`). The
  guitest baseline `rows=430` is unchanged. Version **0.34.0 → 0.35.0**.
- Next (v0.36.0 / roadmap v0.37.0): the GUI Trident search panel.
