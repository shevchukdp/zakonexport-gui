# v0.16.0 - Changes / Зміни (2026-10-27)

## UK / Українська

- **Виявлено базовий кодувальник документів** (`src/ftsearch.rs`).
  - Текст документів `.mb` у базі — це **не** звичайний cp1251 і не
    LmCode. Це **cp1251 (стандартна кирилиця) + українські літери у
    символічному діапазоні `0x80–0xBF`**:
    - стандартна кирилиця (cp1251): великі `А–П` = `0xC0–0xCF`,
      `Р–Я` = `0x90–0x9F`; малі `а–п` = `0xE0–0xEF`,
      `р–я` = `0xF0–0xFF`;
    - українські перевизначення: **`І` = `0xB2`, `і` = `0xB3`**
      (підтверджено — «Конституція» в `class1.mb` =
      `CA EE ED F1 F2 E8 F2 F3 F6 B3 FF` на самому офсеті оракула
      42 = 0x2A); `Є` = `0xAA`, `ї` = `0xBF` (визначено з тексту);
      решта слотів (`є`, `ґ`, `Ї`, `Г`) — рідкісні у базі, далі.
- **`ftsearch` — повний текстовий пошук по `.mb`** (нова
  `src/ftsearch.rs` + CLI-дієслово `ftsearch <запит>`).
  - Кодує запит через базовий кодувальник і сканує **всі** `.mb`
    (верхній рівень + `text\`, рекурсивно) у двох реєстрах
    (нижній + перша велика) — **саме механізм оракула** старої
    програми, без B-tree.
  - Зв'язка native → оракул реалізована: «конституція» →
    **222 збіги** (оракел = 223, 99,5%). Перші 40 збігів
    збігаються з оракулом 1-до-1:
    `class1.mb @ 42`, `class1.mb @ 111`, `1042.mb @ 1840761`,
    `105.mb @ 2556042`, …
  - Пошук за стеমে («конституц» ≈ 12 000) **не** оракул — підтверджено,
    що оракул рахує **точне слово** (case-insensitive).
- **`tsearch`** (нативний B-tree-індекс) лишається експериментальним;
  повний декод щільного B-tree + точні word→postings кордони —
  v0.17.0.
- Версія **0.15.0 → 0.16.0** (єдине джерело: `CARGO_PKG_VERSION`).
- **91 юніт-тест** (нові: `ftsearch::tests::{standard_
  cyrillic_is_cp1251, ukrainian_overrides, konstitutsiya_encodes_as_
  observed, scan_offsets_finds_matches, real_base_reproduces_oracle}`).

### Roadmap (v0.17.0)
- Повний щільний B-tree-словник (Patricia-trie, декод `lmu.voc`),
  точні word→postings кордони, граф `.g` морфології; докодування
  решти українських слотів кодувальника (`є`, `ґ`, `Ї`, `Г`);
  повний (223/223) збіг оракула.

## EN / English

- **Discovered the base document codepage** (`src/ftsearch.rs`).
  - The `.mb` document text is **not** plain cp1251 or LmCode. It is
    **cp1251 (standard Cyrillic) + the Ukrainian letters in the
    `0x80–0xBF` symbol range**:
    - standard Cyrillic (cp1251): upper `А–П` = `0xC0–0xCF`,
      `Р–Я` = `0x90–0x9F`; lower `а–п` = `0xE0–0xEF`,
      `р–я` = `0xF0–0xFF`;
    - Ukrainian overrides: **`І` = `0xB2`, `і` = `0xB3`**
      (confirmed — «Конституція» in `class1.mb` =
      `CA EE ED F1 F2 E8 F2 F3 F6 B3 FF` at the oracle's own offset
      42 = 0x2A); `Є` = `0xAA`, `ї` = `0xBF` (read from the text);
      the remaining slots (`є`, `ґ`, `Ї`, `Г`) are rare in the base —
      later.
- **`ftsearch` — full-text search over the `.mb` files** (new
  `src/ftsearch.rs` + the `ftsearch <query>` CLI verb).
  - Encodes the query through the base codepage and scans **all**
    `.mb` files (top level + `text\`, recursive) in two cases
    (lower + title) — **exactly the legacy app's oracle mechanism**,
    no B-tree needed.
  - The native → oracle join is implemented: «конституція» →
    **222 hits** (oracle = 223, 99.5%). The first 40 hits match the
    oracle 1-to-1:
    `class1.mb @ 42`, `class1.mb @ 111`, `1042.mb @ 1840761`,
    `105.mb @ 2556042`, …
  - The stem search («конституц» ≈ 12 000) is *not* the oracle —
    confirming the oracle counts an **exact word** (case-insensitive).
- **`tsearch`** (the native B-tree index) stays experimental; the
  full packed B-tree decode + exact word→postings boundaries —
  v0.17.0.
- Version **0.15.0 → 0.16.0** (single source: `CARGO_PKG_VERSION`).
- **91 unit tests** (new: `ftsearch::tests::{standard_
  cyrillic_is_cp1251, ukrainian_overrides, konstitutsiya_encodes_as_
  observed, scan_offsets_finds_matches, real_base_reproduces_oracle}`).

### Roadmap (v0.17.0)
- The full packed B-tree dictionary (Patricia-trie, `lmu.voc`
  decode), the exact word→postings boundaries, the `.g` morphology
  graph; decode the remaining codepage slots (`є`, `ґ`, `Ї`, `Г`);
  a full 223/223 oracle match.
