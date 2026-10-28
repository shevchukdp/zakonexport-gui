# v0.26.0 - Changes / Зміни (2026-10-28)

## UK / Українська

- **`.m` секційний walker (roadmap v0.27.0).** `trident_vt.rs`:
  новий `VmSectionKind { U16Records, NulRecords, WordRows,
  LenFlagIndex, Small, Packed, Unknown }` + `VmSectionWalk` (slot,
  off, head_u32, kind, records, landed, lands_on_next); `VmWordRows`
  (slot 0: 16-байтові word-rows + `field_0c_hist`), `VmFieldRegistry`
  (0x88-блок: marker u32 + 5 u32-офсетів); `VmSections::parse` —
  класифікує ВСІ 16 слотів .m-таблиці в file-order.
  `TridentIndex.m_sections` / `rus_m_sections` (збираються в
  `open()`).
- **Емпірична корекція 0x88**: registry — НЕ u16-counted NUL-string
  секція, а малий u32-конфіг-блок. Marker u32 = `0x00FFE703` (ukr) /
  `0x00FFEF02` (rus) — 0xFF-слово в бітах 16..23 = блок-маркер,
  низкий байт = sub_count (3 ukr / 2 rus); далі 5 u32-офсетів у
  B-tree/row-регіони (ukr: 0x1900/0x1A00/0x1400/0x2400/0x2D00;
  rus: 0x1100/0x1200/0x0D00/…). String-декодування чекає на
  B-tree walk (v0.29+).
- **Tiling**: точно-плитлять файл лише u16 word-таблиці (ukr 0x37CB
  count=254 → САМО 0x39C9; rus 0x3001 count=256 → САМО 0x3203);
  `tiling_count() == 1` в обох файлах — packed B-tree-регіони
  переплітаються (land на краю регіону / хвості файлу).
- **Word-rows (slot 0)**: ukr 0x1000..0x1405 = 64×16B + 5B
  leftover, УСІ 64 rows non-zero; rus 0xC20..0xF5D = 51×16B + 13B,
  УСІ 51 non-zero. Дельта: 64/51 rows ≠ 1002/650 dict-слів — це
  структурний decode; 224+B `0xE`-rows (+0xD7/+0xD8/+0xD9 flags of
  `FUN_100177d0`) — після B-tree (v0.29+).
- **slot 7 (len-flag key-index)**: ukr 0x29D7 = 16 NUL-ів +
  (u8 len, u8 flag) пари; rus 0x25C1 = ascending u16-офсетний run +
  NUL + пари.
- **CLI `trident`**: 2 нові рядки `m-секції (v0.26.0, ukr/rus …)` —
  kind-гистограма 16 слотів + tiling-count + word-rows + 0x88
  registry. Usage + roadmap-сумарі оновлені.
- **152 юніт-тестів** (нові 5: real-file walks ukr/rus + synthetic
  word-rows/registry/tiling-table). Guitest baseline `rows=430` без
  змін. Версія **0.25.0 → 0.26.0**.
- Далі (v0.27.0 / roadmap v0.28.0): `FUN_100177d0` 22-opcode
  predicate-каталог (UA+EN) + evaluable-subset проти v0.26.0 rows.

## EN / English

- **`.m` section walker (roadmap v0.27.0).** `trident_vt.rs`: new
  `VmSectionKind { U16Records, NulRecords, WordRows, LenFlagIndex,
  Small, Packed, Unknown }` + `VmSectionWalk` (slot, off, head_u32,
  kind, records, landed, lands_on_next); `VmWordRows` (slot 0: the
  16-byte word rows + `field_0c_hist`), `VmFieldRegistry` (the 0x88
  block: marker u32 + five u32 offsets); `VmSections::parse` —
  classifies ALL 16 slots of the `.m` table in file order.
  `TridentIndex.m_sections` / `rus_m_sections` (built in `open()`).
- **Empirical 0x88 correction**: the registry is NOT a u16-counted
  NUL-string section — it is a small u32 config block. The marker u32
  = `0x00FFE703` (ukr) / `0x00FFEF02` (rus): the 0xFF word in bits
  16..23 is the block marker, the low byte is the sub_count (3 ukr /
  2 rus); then five u32 offsets into the B-tree / row regions (ukr:
  0x1900/0x1A00/0x1400/0x2400/0x2D00; rus: 0x1100/0x1200/0x0D00/…).
  The string decode awaits the B-tree walk (v0.29+).
- **Tiling**: only the u16 word tables tile the file exactly (ukr
  0x37CB count=254 → exactly 0x39C9; rus 0x3001 count=256 → exactly
  0x3203); `tiling_count() == 1` in both files — the packed B-tree
  regions interleave (they land at the region end / file tail).
- **Word rows (slot 0)**: ukr 0x1000..0x1405 = 64×16B + 5 B
  leftover, ALL 64 rows non-zero; rus 0xC20..0xF5D = 51×16B + 13 B,
  ALL 51 non-zero. Delta: 64/51 rows ≠ 1002/650 dict words — this is
  the structural decode; the 224+ B `0xE` rows (+0xD7/+0xD8/+0xD9
  flags of `FUN_100177d0`) come with the B-tree walk (v0.29+).
- **Slot 7 (len-flag key index)**: ukr 0x29D7 = 16 NULs + (u8 len,
  u8 flag) pairs; rus 0x25C1 = an ascending u16 offset run + NUL +
  pairs.
- **CLI `trident`**: two new `m-секції (v0.26.0, ukr/rus …)` lines —
  the kind histogram over the 16 slots + the tiling count + the word
  rows + the 0x88 registry. The usage + roadmap summary are updated.
- **152 unit tests** (5 new: the real-file walks ukr/rus + the
  synthetic word-rows/registry/tiling-table). The guitest baseline
  `rows=430` is unchanged. Version **0.25.0 → 0.26.0**.
- Next (v0.27.0 / roadmap v0.28.0): the `FUN_100177d0` 22-opcode
  predicate catalog (UA+EN) + the evaluable subset against the
  v0.26.0 rows.
