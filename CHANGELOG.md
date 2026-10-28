# CHANGELOG / Журнал змін / Change history

Правило для кожної нової версії (standing rule, UA + EN):
1. Запис у цьому файлі (UA + EN) — до публікації;
2. Нотатки сторінки GitHub-релізу — UA + EN;
3. Файл `changes.md` (UA + EN) **в папці версії** `releases/<v>/`;
4. Бинарники: `releases/<v>/` у репо + ассети GitHub-релізу
   (`.github/workflows/release-assets.yml` або прямий POST на `uploads.github.com`).

Rule for every new version (UA + EN):
1. An entry in this file (UA + EN) — before publishing;
2. GitHub release notes — UK + EN;
3. A `changes.md` file (UK + EN) **inside the version folder** `releases/<v>/`;
4. Binaries: `releases/<v>/` in-repo + GitHub release assets
   (`.github/workflows/release-assets.yml` or direct POST to `uploads.github.com`).

---

## v0.19.0 — 2026-10-28

### UK
- **Точний leaf→`.vt` stem-join** (замінює пропорційний `posting_slice`
  як основний доказ; той лишається fallback):
  - `.vt` layout **експериментально декодовано**: 64B-заголовок
    (vendor «Trident Soft. Inc.» @0x00, name @0x18, `FREE` @0x38,
    u32 @0x3C = **хвіст** = кінець record-потіку) + **14160
    NUL-термінованих записів** `[u8 count][payload][NUL]`, зажатих
    `0x40..0xCCDE5` (`lmur.vt`; `lmru.vt`: хвіст `0xD2648` = len−4).
    Payload = **case-folded Latin-стеблі** («не»→`NE`, «по»→`PO`,
    «пере»→`PERE`, «спів»→`SPV`, «був»→`BUV`, «неч»→`HCH`) + u16-
    слова (документи 0..65535). Потік в тому ж порядку сортування,
    що й `.voc` словник.
  - новий модуль **`src/trident_vt.rs`**: `TridentFileHeader`,
    `VtRecordStream` (walk + `find_stem`), `VmHeader` (`.m`: 64B +
    u32-таблиця @0x40, `lmur.m`: `0x1000, 0x88, 0xBCB, 0x726, 0xA1,
    0x206D, 0x244B, 0x29D7`, хвіст @0x70E); документація DLL
    join-ланцюга (`FUN_10015340` ctx init, `FUN_100176c0`
    word-таблиця @ctx+0x44, `FUN_10015cd0` predicate-таблиця @ctx+0x30,
    `FUN_100177d0` doc-flag предикати з `.m`, `FUN_100134d0` tail
    scan, `FUN_100019f0` byte-VM).
  - **`src/trident.rs`**: `latin_candidates` (LATIN_VARIANTS —
    контекстно-двозначний Latin-маппінг із пріоритетними варіантами,
    cap 32) + `exact_postings(word)` (stem hit → record + його u16-
    список) + `TridentResult.joined: Vec<(candidate, record_off,
    Vec<u16>)>`; `search()` — exact-join як primary, `posting_slice` —
    fallback. «був» → `[a1 e3 a2]` → `BUV` → 2 records
    (@0x5362b / @0x84181 — підтверджені офсети).
- **`.voc` u16-параметри @0x80** (`trident_voc.rs`): `VocParams`
  (16 u16: `2,2,1,0x600,2,2,0x42,0x2000,0x20,0x108,0x104,0x10,2,4,
  0x80,0x400`); ідентифіковано: `[6]` = listA stop-marker `0x42`,
  `[8]` = sec_54 count (32), `[9]` = `lmr.gw` size (264),
  `[7]` = page-константа 0x2000.
- **CLI**: `trident` — нові блоки (`.vt record-потік: 14160
  record-ів 0x40..0xccde5 · 76225 Latin-run · 402210 u16-слів`;
  `.m: u32-таблиця + хвіст @0x70e`; `параметри @0x80: stop=0x42
  блок=0x2000 sec_54=32 gw=264`); `tsearch` — рядок точного join
  (record-офсет + u16-список).
- **125 юніт-тестів** (нові: `trident_vt` — header/stream/14160-
  walk/find_stem/`.m`; `VocParams` на реальному `lmu.voc`;
  `latin_candidates`/`exact_postings`). Guitest baseline `rows=430`
  без змін. Версія **0.18.1 → 0.19.0**.

### EN
- **Exact leaf→`.vt` stem-join** (replaces the proportional
  `posting_slice` as the primary evidence; it stays the fallback):
  - the `.vt` layout was **empirically decoded**: a 64-byte header
    (vendor «Trident Soft. Inc.» @0x00, name @0x18, `FREE` @0x38,
    u32 @0x3C = **tail** = the end of the record stream) plus **14160
    packed NUL-terminated records** `[u8 count][payload][NUL]` in
    `0x40..0xCCDE5` (`lmur.vt`; `lmru.vt`: tail `0xD2648` = len−4).
    Payload = **case-folded Latin stems** («не»→`NE`, «по»→`PO`,
    «пере»→`PERE`, «спів»→`SPV`, «був»→`BUV`, «неч»→`HCH`) + u16
    words (documents 0..65535). The stream keeps the `.voc`
    dictionary's sort order.
  - new module **`src/trident_vt.rs`**: `TridentFileHeader`,
    `VtRecordStream` (the walk + `find_stem`), `VmHeader` (`.m`:
    64-byte header + the u32 table @0x40 — `lmur.m`: `0x1000, 0x88,
    0xBCB, 0x726, 0xA1, 0x206D, 0x244B, 0x29D7`, tail @0x70E);
    docs of the DLL join chain (`FUN_10015340` context init,
    `FUN_100176c0` word table @ctx+0x44, `FUN_10015cd0` predicate
    table @ctx+0x30, `FUN_100177d0` doc-flag predicates over `.m`,
    `FUN_100134d0` the tail scan, `FUN_100019f0` the byte-VM).
  - **`src/trident.rs`**: `latin_candidates` (`LATIN_VARIANTS` — the
    context-dependent Latin mapping with preferred variants, capped
    at 32) + `exact_postings(word)` (stem hit → the record + its u16
    list) + `TridentResult.joined: Vec<(candidate, record_off,
    Vec<u16>)>`; `search()` runs the exact join first, the
    `posting_slice` remains the fallback. «був» → `[a1 e3 a2]` →
    `BUV` → 2 records (@0x5362b / @0x84181, the confirmed offsets).
- **`.voc` u16 parameters @0x80** (`trident_voc.rs`): `VocParams`
  (16 u16: `2,2,1,0x600,2,2,0x42,0x2000,0x20,0x108,0x104,0x10,2,4,
  0x80,0x400`); identified fields: `[6]` = the listA stop marker
  `0x42`, `[8]` = the sec_54 count (32), `[9]` = the `lmr.gw` size
  (264), `[7]` = the 0x2000 page constant.
- **CLI**: `trident` — new blocks (`.vt record stream: 14160
  records 0x40..0xccde5 · 76225 Latin runs · 402210 u16 words`;
  `.m: u32 table + tail @0x70e`; `params @0x80: stop=0x42 block=
  0x2000 sec_54=32 gw=264`); `tsearch` — the exact-join line (the
  record offset + the u16 list).
- **125 unit tests** (new: `trident_vt` — header / stream / the
  14160 walk / `find_stem` / `.m`; `VocParams` on the real
  `lmu.voc`; `latin_candidates` / `exact_postings`). The guitest
  baseline `rows=430` is unchanged. Version **0.18.1 → 0.19.0**.

---

## v0.25.0 — 2026-10-28

### UK
- **Rus-узагальнення: language-generic loader + `tsearch --lang`.** Усе
  попереднє було Ukrainian-primary; rus-сторона (`lmr.*`) — ті самі
  формати. Редизайн `trident.rs`:
  - новий `Lang { Ukr, Rus }` + приватний `LangCtx` (словник + `.vt`
    stream + u16-простір + fold-таблиця для ОДНОЇ мови); всі
    search-методи (`search_in`/`find_prefix_in`/`posting_slice_in`/
    `exact_postings_in`/`term_join_docs_in`) тепер language-generic.
  - `TridentIndex` несе rus-ядра: `rus_dict_words` (з `lmr.voc`) +
    `rus_vt_u16` (з `lmru.vt`); loader завантажує обидві мови.
  - нові публічні: `search_lang(q, lang)`, `search_auto(q)` (об'єднання
    ukr+rus), `term_join_docs_lang`, `encode_query_lang`,
    `exact_postings_lang`.
- **CLI `tsearch --lang u|r|auto`**: `r` — rus (fold.rus + lmr.*),
  `auto` — обидві мови (merge, з мітками). `trident` — новий рядок
  «language-індекси (v0.25.0): ukr lmu.voc=1002 / lmur.vt 419540 u16 ·
  rus lmr.voc=650 / lmru.vt 430854 u16 (14421 record-ів)».
- **Емпірика**: rus-ядра реально завантажуються (lmr.voc=650 слів,
  lmru.vt=14421 record-ів, fold.rus=32 пар); `tsearch --lang r` «не» →
  32 join record-ів (NET/HE в lmru.vt), «закон» → proportional
  fallback (474 док-ів). Обидві мови працюють «з коробки».
- **147 юніт-тестів** (нові 2: rus-core load — 14421 records + rus dict
  + fold.rus; `tsearch --lang r` rus-stem → join/fallback evidence +
  auto-merge). Guitest baseline `rows=430` без змін. Версія
  **0.24.0 → 0.25.0**.
- Далі (v0.26.0/roadmap v0.27.0): per-word metadata (u16 word table →
  word idx → doc-ids semantic walk), `lmu.g` edge semantics.

### EN
- **Rus generalization: language-generic loader + `tsearch --lang`.**
  Everything before was Ukrainian-primary; the rus side (`lmr.*`) uses
  the same formats. `trident.rs` reworked:
  - a new `Lang { Ukr, Rus }` + a private `LangCtx` (dictionary + `.vt`
    stream + u16 space + fold table for ONE language); all the search
    methods (`search_in` / `find_prefix_in` / `posting_slice_in` /
    `exact_postings_in` / `term_join_docs_in`) are now language-generic.
  - `TridentIndex` carries the rus core: `rus_dict_words` (from
    `lmr.voc`) + `rus_vt_u16` (from `lmru.vt`); the loader reads both
    languages.
  - new public: `search_lang(q, lang)`, `search_auto(q)` (ukr+rus
    merge), `term_join_docs_lang`, `encode_query_lang`,
    `exact_postings_lang`.
- **CLI `tsearch --lang u|r|auto`**: `r` — Russian (`fold.rus` +
  `lmr.*`), `auto` — both (merged, labelled). `trident` — a new
  «language-індекси (v0.25.0): ukr lmu.voc=1002 / lmur.vt 419540 u16 ·
  rus lmr.voc=650 / lmru.vt 430854 u16 (14421 records)» line.
- **Empirical**: the rus core loads for real (lmr.voc=650 words,
  lmru.vt=14421 records, fold.rus=32 pairs); `tsearch --lang r` «не»
  → 32 join records (NET/HE in lmru.vt), «закон» → proportional
  fallback (474 docs). Both languages work out of the box.
- **147 unit tests** (2 new: the rus-core load — 14421 records + the rus
  dict + fold.rus; `tsearch --lang r` rus-stem → join/fallback evidence
  + the auto merge). The guitest baseline `rows=430` is unchanged.
  Version **0.24.0 → 0.25.0**.
- Next (v0.26.0 / roadmap v0.27.0): per-word metadata (the u16 word
  table → word idx → doc-ids semantic walk), `lmu.g` edge semantics.

---

## v0.24.0 — 2026-10-28

### UK
- **tsearch v2: точний join як primary, позначені fallback-и.**
  Редизайн `TridentResult`/`search()`:
  - точний stem-join (`.vt` record → u16 doc-ids) — **primary**
    evidence (поле `joined`);
  - пропорційний `posting_slice` тепер **лише fallback** —
    заповнюється `docs`, коли точний join не розв'язав термін, і
    маркер `docs_fallback`; в CLI позначається «оцінка,
    пропорційний posting-slice» — жодна евристика не видається за
    точну;
  - новий `TridentIndex::term_join_docs(term)` — distinct u16 doc-ids
    (дедуп, без нулів, відсортовано) для одного терміна.
- **Новий прапорець `tsearch --oracle`** (кросчек проти ftsearch):
  `ftsearch::oracle_multi` рахує distinct-файли, що містять кожен
  термін (один проход по `.mb`, нижній + title-case). Для кожного
  терміна виводиться `tsearch N док-ів · oracle M док-ів · збіг
  min/max` (count-збіг: u16 doc-ids та `.mb`-файли — різні
  ідентифікаційні простори). Загальний збіг ≥ 0.9 → рядок «оракельно
  підтверджено», інакше «експеримент (пошук)».
- **Емпірика 20-запитного корпусу** (20 реальних юридичних термінів,
  один `.mb`-скан, 2388 файлів): per-term count-збіг у діапазоні
  **0.00..0.55**; **загальний збіг 0.08** → верб залишається
  «експеримент (пошук)». Точний stem-join розв'язує лише короткі
  стеми/префікси (.vt Latin-стems): «не»=0.41, «по»=0.55,
  «був»=0.14, «пере»=0.27, «зак»=0.23; повні юридичні слова
  (закон, суд, договір, постанова, указ, кабінет, апеляція,
  скасування, затвердження, регулювання, податок, бюджет,
  конституція, рішення, спів) → 0 документів → збіг 0.00 (точний join
  дає підмножину oracle-документів; повні слова через Latin-run
  substring-матч не розв'язуються — далі в roadmap v0.29+ B-tree).
- **145 юніт-тестів** (нові 3: `OracleTerm::agreement`-класи,
  `oracle_multi` distinct-file-лічильник, `term_join_docs`
  дедуп/без-нулів/сортування). Guitest baseline `rows=430` без змін.
  Версія **0.23.0 → 0.24.0**.
- Далі (v0.26.0): rus-узагальнення `tsearch --lang`; семантичне
  підтвердження u16↔docloc docnums (5 spot-check `fields <docnum>`).

### EN
- **tsearch v2: exact join as primary, labelled fallbacks.**
  `TridentResult`/`search()` reworked:
  - the exact stem-join (`.vt` record → u16 doc-ids) is the
    **primary** evidence (the `joined` field);
  - the proportional `posting_slice` is now a **fallback only** —
    `docs` is filled only when the exact join cannot resolve a term,
    flagged by `docs_fallback`; the CLI labels it «estimate,
    proportional posting-slice» so no heuristic is presented as exact;
  - new `TridentIndex::term_join_docs(term)` — the distinct u16
    doc-ids for one term (de-duplicated, zero dropped, sorted).
- **New `tsearch --oracle` flag** (cross-check vs ftsearch):
  `ftsearch::oracle_multi` counts the distinct files containing each
  term (one `.mb` pass, lower + title case). For each term it prints
  `tsearch N docs · oracle M docs · agreement min/max` (a
  *count* agreement — the u16 doc-ids and the `.mb` files are
  different identifier spaces). Overall agreement ≥ 0.9 → the line
  says «оракельно підтверджено / oracle-confirmed», else «експеримент
  (пошук)».
- **20-query corpus empirical** (20 real legal terms, one `.mb` scan,
  2388 files): per-term count agreement in **0.00..0.55**;
  **overall agreement 0.08** → the verb stays «експеримент (пошук) /
  experimental (search)». The exact stem-join resolves only short
  stems/prefixes (the `.vt` Latin stems): «не»=0.41, «по»=0.55,
  «був»=0.14, «пере»=0.27, «зак»=0.23; the full legal words (закон,
  суд, договір, постанова, указ, кабінет, апеляція, скасування,
  затвердження, регулювання, податок, бюджет, конституція, рішення,
  спів) → 0 documents → 0.00 agreement (the exact join yields a subset
  of the oracle docs; full words are not resolved by the Latin-run
  substring match — a v0.29+ B-tree roadmap item).
- **145 unit tests** (3 new: the `OracleTerm::agreement` classes, the
  `oracle_multi` distinct-file counter, `term_join_docs`
  dedup/zero-drop/sort). The guitest baseline `rows=430` is unchanged.
  Version **0.23.0 → 0.24.0**.
- Next (v0.26.0): the rus generalization `tsearch --lang`; semantic
  confirmation of the u16↔docloc docnums (5 `fields <docnum>`
  spot-checks).

---

## v0.23.0 — 2026-10-28

### UK
- **Doc-number join: native u16 ↔ поточні docnums** (нова
  `src/trident_join.rs`, `DocJoin` / `JoinKind`). Пробиє простір
  u16-подингів `.vt` (dense-потік `TridentIndex.vt_u16`) проти
  поточного doc-лічильника бази (`render::Base::doc_count()`,
  `docloc.db`). Класифікація:
  - **Identity** — max u16 ≤ doc_count: u16-ідентифікатори
    кандидат-поточні docnums (без таблиці-мапінгу);
  - **Bounded** — простір 2005 ширший; openable = u16 ≤ docnum;
  - **Unknown** — поточну базу не завантажено.
- **Емпірика цієї бази (v0.23.0)**: у16-простір `.vt` сягає 65535
  (~46k distinct, чисті per-record u16 від 257), а поточна
  docloc-база = **613173 док-ів** (`docloc.db` 3065877 B →
  (len−12)/5; контейнер `doc\doc.mb`). Простір 2005 цілком вкладає
  в поточний docnum-діапазон → проба класифікує **Identity** (усі
  u16 — кандидат-поточні docnums). (Легасі `text\*.mb` = 2363
  файлів — це вхід для ftsearch-оракула, не docloc-база.)
- **CLI `tsearch`** — новий рядок `join native→docnum (v0.23.0)`:
  у16-простір + distinct/total vs поточна база + kind; per-record
  **openable-субмножина** (≤ docnum) для топ-4 join-кандидатів;
  фінальний рядок «результат» тепер динамічний (показує actual
  kind, а не hard-coded Bounded).
- **142 юніт-тестів** (нові 5: identity/bounded/unknown-класи,
  openable-субмножина, реальний lmur.vt-потік → Identity). Guitest
  baseline `rows=430` без змін. Версія **0.22.0 → 0.23.0**.
- Наступний RE-крок (v0.24.0): oracle-кросчек tsearch vs ftsearch
  (чи u16 docnums = docloc docnums семантично), потім rus
  узагальнення `tsearch --lang`.

### EN
- **Doc-number join: native u16 ↔ current docnums** (new
  `src/trident_join.rs`, `DocJoin` / `JoinKind`). Probes the `.vt`
  u16 posting space (the dense `TridentIndex.vt_u16` stream) against
  the current base doc count (`render::Base::doc_count()`,
  `docloc.db`). Classification:
  - **Identity** — max u16 ≤ doc_count: the u16 ids are candidate
    current docnums (no mapping table);
  - **Bounded** — the 2005 space is wider; openable = u16 ≤ docnum;
  - **Unknown** — the current base did not load.
- **Empirical (this base, v0.23.0)**: the u16 space of `.vt` reaches
  65535 (~46k distinct; clean per-record u16s from 257), while the
  current docloc base = **613173 docs** (`docloc.db` 3065877 B →
  (len−12)/5; `doc\doc.mb` container). The 2005 space fits
  entirely inside the current docnum range → the probe classifies
  **Identity** (every u16 is a candidate current docnum). (The
  legacy `text\*.mb` = 2363 files is the ftsearch-oracle input, not
  the docloc base.)
- **CLI `tsearch`** — a new `join native→docnum (v0.23.0)` line:
  the u16 space + distinct/total vs the current base + the kind; a
  per-record **openable subset** (≤ docnum) for the top-4 join
  candidates; the final «результат» line is now dynamic (shows the
  actual kind, not a hard-coded Bounded).
- **142 unit tests** (5 new: the identity/bounded/unknown classes,
  the openable subset, and the real lmur.vt stream → Identity). The
  guitest baseline `rows=430` is unchanged. Version **0.22.0 →
  0.23.0**.
- Next RE step (v0.24.0): the tsearch vs ftsearch oracle
  cross-check (are the u16 docnums the docloc docnums, semantically),
  then the rus generalization `tsearch --lang`.

---

## v0.22.0 — 2026-10-28

### UK
- **U16 word-таблиця структурно декодована** (нова `VmWordTable`,
  `.m` slot 11): секція = `[u16 count][count u16 records @base+2]`.
  Count = **кількість слів словника** (254 ukr / 256 rus). Перша
  половина записів (126/128) — малі уростальні offset-значення
  (step 2, 0x0100..0x01FA ukr / 0x0102.. rus), друга половина —
  i16-від'ємні значення (high byte 0xFF) з **kind-маркерами 0xFFFA
  (61 ukr / 64 rus)** та фінальним boundary-записом (**0x8000 ukr /
  0x7F00 rus**). Це і є ті «kind-byte records 0xFA/0x33» з старого
  blocked item: 0xFA присутній як значення запису 0xFFFA, а 0x33 —
  один байт у потоку `.m` (пошук — далі).
- **U16 predicate-таблиця структурно декодована** (нова
  `VmPredicateTable`, `.m` slot 5): секція = `[u16 offsets ×
  160/168][sentinel 0xCECA][packed u8-op/u16-arg records]`.
  Offset-run уростає (non-strictly) і **завершується sentinel
  0xCECA в ОБОХ мовах** — міжмовний інваріант. Після sentinel —
  packed-рекорди (у ukr перший байт = 0x16), які ходить
  `FUN_10015cd0`; семантика ланцюжка — наступний RE-крок.
- **`TridentIndex`** несе всі 4 таблиці (ukr/rus): `word_table`,
  `pred_table`, `rus_word_table`, `rus_pred_table`.
- **CLI `trident`** — нові fingerprint-рядки: `u16 word-таблиця
  @0x37cb: 254 records — 126 offset-like (останній 0x1fa) + 128 i16 ·
  0xFFFA=61 · boundary=0x8000`, `u16 pred-таблиця @0x206d: 160
  offset-ів (0x13e..0x2316) + sentinel 0xceca · records @хвіст: 16 cb
  1a cb …` (+ rus-аналоги).
- **137 юніт-тестів** (нові: реальні word/pred-таблиці обох мов —
  перші 8 offset-ів, kind-розподіли, sentinel, truncation). Guitest
  baseline `rows=430` без змін. Версія **0.21.0 → 0.22.0**.

### EN
- **The u16 word table structurally decoded** (new `VmWordTable`,
  `.m` slot 11): the section is `[u16 count][count u16 records
  @base+2]`. The count is the **dictionary word count** (254 ukr /
  256 rus). The first half of the records (126/128) are small
  ascending offset-like values (step 2, 0x0100..0x01FA ukr /
  0x0102.. rus), the second half are i16-negative values (high byte
  0xFF) with the **0xFFFA kind markers (61 ukr / 64 rus)** and a
  final boundary record (**0x8000 ukr / 0x7F00 rus**). These are
  the "kind-byte records 0xFA/0x33" of the old blocked item: 0xFA
  is present as the 0xFFFA record value, and 0x33 is a single
  stream byte in the `.m` (its hunt comes next).
- **The u16 predicate table structurally decoded** (new
  `VmPredicateTable`, `.m` slot 5): the section is `[u16 offsets ×
  160/168][0xCECA sentinel][packed u8-op/u16-arg records]`. The
  offset run is non-strictly ascending and **ends at the 0xCECA
  sentinel in BOTH languages** — a cross-language invariant. After
  the sentinel sit the packed records (ukr first byte = 0x16) that
  `FUN_10015cd0` walks; the chain semantics are the next RE step.
- **`TridentIndex`** carries all four tables (ukr/rus):
  `word_table`, `pred_table`, `rus_word_table`, `rus_pred_table`.
- **CLI `trident`** — new fingerprint lines: `u16 word-таблиця
  @0x37cb: 254 records — 126 offset-like (останній 0x1fa) + 128 i16
  · 0xFFFA=61 · boundary=0x8000`, `u16 pred-таблиця @0x206d: 160
  offset-ів (0x13e..0x2316) + sentinel 0xceca · records @хвіст: 16
  cb 1a cb …` (+ the rus analogues).
- **137 unit tests** (new: the real word/predicate tables of both
  languages — the first 8 offsets, the kind distributions, the
  sentinel, the truncation cases). The guitest baseline `rows=430`
  is unchanged. Version **0.21.0 → 0.22.0**.

---

## v0.21.0 — 2026-10-28

### UK
- **Повний заголовок `.m` декодовано (v0.21.0 finding): u32-таблиця
  має **16 слотів, а не 8** (v0.19.0 читав першу половину).** Емпіричні
  дампи `lmur.m`/`lmru.m` зафіксували ролі слотів: **slot 5 = u16
  predicate-таблиця** (0x206D ukr / 0x1ACF rus — саме `ctx+0x44`, яку
  ходить `FUN_100176c0`/`FUN_10015cd0`), **slot 11 = u16 word-таблиця**
  (0x37CB ukr / 0x3001 rus — `ctx+0x30`). Висновок, що розблоковує
  blocked item #1: u16 word/predicate-таблиці лежать **у файлах `.m`,
  а не в хвості `.vt`** (old-format `.vt` хвости порожні — v0.20.0) і не
  виводяться з B-tree `.voc`.
- **Tail N-секція `.m` НЕ порожня (на відміну від `.vt`)** — `VmHeader`
  тепер парсить її вбудовано (`VtTailScan`): `lmur.m` @0x70E: **N=3**
  (`"R"→[0x1]`, `"N*"→[0x1C,0x3E]`, `"*"→[0x3E,0x14]`), `lmru.m`
  @0x53E: **N=4** (додатковий запис `"*N"→[0x24,0x0E,0x26,0x1D,0x3E]`).
  Це wildcard-ключі, які `tail_match` (v0.20.0) доповнює у повний
  word → u16 word-index join.
- **Два похідні поля заголовка зафіксовано міжмовним інваріантом**:
  `u32@0x84 = pred_off − 7` (0x2066+7=0x206D; 0x1AC8+7=0x1ACF) та
  `u32@0x80 = pred_off − 19` (обидва поля різняться на 12). Семантика
  0xE-структур (`0x8C..`) — наступний RE-крок (v0.22+).
- **U16 word-таблиця структурно ідентифікована**: `[u16 count]` +
  `count` відносних u16 офсетів, які рівно заповнюють секцію до
  вказівника slot 12 (ukr: count=0xFE=254, секція 0x37CB..0x39C9;
  rus: count=0x100=256, секція 0x3001..0x3203). Predicate-таблиця —
  чистий уростальний u16-масив (ukr 0x206D: 0x013E 0x0154 0x0164
  0x0170 …). Повний structural decode (kind-байти 0xFA/0x33) — v0.22.0.
- **CLI `trident dump <файл> [офс] [довжина]`** — новий console-only
  hex-dump sub-verb (16 байтів/рядок + latin1; `<файл>` шукатиметься в
  `SOFT\`, потім в корені бази). Прискорює кожен наступний RE-крок.
- **Документація по `.use`/`.uvt` (RE-факти з `lmsup32.dll`)**:
  `lmC.use` (C = мова) відкривається **в write-режимі**
  (`FUN_10011b20(..., 0x40, 0, 'W')`, fallback `FUN_10011e10(...,
  0x40, 0xCC, 'W')` — саме 204-байтова форма наших файлів) → `.use` це
  **write-back cache**, а не read-стрім (у read-only базі — нульовий);
  `.uvt` відкривається лише `'W'` (а `.vt` — `'R'`) → update-cache
  зарезервовано й ніколи не писалося.
- **CLI `trident`** — оновлені блоки: 16-slot таблиці + slot5/slot11 +
  u80/u84 + tail N-секції для `lmur.m`/`lmru.m`; tail-scan рядок тепер
  вказує, що u16 таблиці виявлено в `.m`.
- **132 юніт-тестів** (нові: повна 16-slot таблиця + tail N-секції для
  обох мов, u80/u84-інваріанти, count+offsets shape word-таблиці,
  ascending predicate-масив). Guitest baseline `rows=430` без змін.
  Версія **0.20.0 → 0.21.0**.

### EN
- **The full `.m` header decoded (v0.21.0 finding): the u32 table has
  **16 slots, not 8** (the v0.19.0 parse was the first half).** The
  empirical dumps of `lmur.m`/`lmru.m` pin the slot roles: **slot 5 =
  the u16 predicate table** (0x206D ukr / 0x1ACF rus — the `ctx+0x44`
  table `FUN_100176c0`/`FUN_10015cd0` walks) and **slot 11 = the u16
  word table** (0x37CB ukr / 0x3001 rus — `ctx+0x30`). Conclusion that
  unblocks blocked item #1: the u16 word/predicate tables live **in
  the `.m` files, not the `.vt` tail** (the old-format `.vt` tails are
  empty — v0.20.0) and are not derived from the `.voc` B-tree.
- **The `.m` tail N-section is NOT empty (unlike `.vt`)** —
  `VmHeader` now parses it inline (`VtTailScan`): `lmur.m` @0x70E:
  **N=3** (`"R"→[0x1]`, `"N*"→[0x1C,0x3E]`, `"*"→[0x3E,0x14]`),
  `lmru.m` @0x53E: **N=4** (the extra record
  `"*N"→[0x24,0x0E,0x26,0x1D,0x3E]`). These are the wildcard keys
  that `tail_match` (v0.20.0) completes into the word → u16
  word-index join.
- **Two derived header fields pinned by the cross-language
  invariant**: `u32@0x84 = pred_off − 7` (0x2066+7=0x206D;
  0x1AC8+7=0x1ACF) and `u32@0x80 = pred_off − 19` (the two fields
  differ by 12). The 0xE-structure semantics (`0x8C..`) — the next RE
  step (v0.22+).
- **The u16 word table structurally identified**: a `[u16 count]` +
  `count` relative u16 offset array that exactly fills the section up
  to the slot-12 pointer (ukr: count=0xFE=254, section
  0x37CB..0x39C9; rus: count=0x100=256, section 0x3001..0x3203). The
  predicate table is a plain ascending u16 array (ukr 0x206D: 0x013E
  0x0154 0x0164 0x0170 …). The full structural decode (the 0xFA/0x33
  kind bytes) is v0.22.0.
- **CLI `trident dump <file> [off] [len]`** — a new console-only
  hex-dump sub-verb (16 bytes/line + latin1; `<file>` resolves against
  `SOFT\`, then the base root). Accelerates every later RE step.
- **`.use`/`.uvt` documentation (RE facts from `lmsup32.dll`)**:
  `lmC.use` (C = the language char) is opened **in write mode**
  (`FUN_10011b20(..., 0x40, 0, 'W')`, fallback `FUN_10011e10(...,
  0x40, 0xCC, 'W')` — the exact 204-byte form of our files) → `.use`
  is a **write-back cache**, not a read stream (zero in this
  read-only base); `.uvt` opens `'W'` only (vs `.vt` `'R'`) → the
  update cache is reserved and never written.
- **CLI `trident`** — updated blocks: the 16-slot tables + slot5/
  slot11 + u80/u84 + the tail N-sections for `lmur.m`/`lmru.m`; the
  tail-scan line now points to the u16 tables found in the `.m`.
- **132 unit tests** (new: the full 16-slot table + the tail
  N-sections for both languages, the u80/u84 invariants, the
  count+offsets word-table shape, the ascending predicate array). The
  guitest baseline `rows=430` is unchanged. Version **0.20.0 →
  0.21.0**.

---

## v0.20.0 — 2026-10-28

### UK
- **Tail-секція `.vt` декодована (v0.20.0 finding): в old-format
  файлах ВОНА ПОРОЖНЯ.** `VtTailScan` (`FUN_100134d0`: у хвісті
  `u8 N` + N записів `[NUL-рядок][u16-список 0x0000]`) на реальному
  `lmur.vt`/`lmru.vt` дає **N = 0** — tail-покажець лежить всередині
  останніх 4 байтів файлу (сам u32 LE). Отже u16 word/predicate-
  таблиці, яких потребує join-ядро (`ctx+0x30`/`ctx+0x44`,
  `FUN_100176c0`/`FUN_10015cd0`), DLL read-mode виводить **не з
  `.vt` хвоста**, а з B-tree `.voc` — наступний RE-крок (v0.21+).
- **Wildcard-матчер `tail_match` портовано** (`FUN_100135d0`):
  `*` (зірки їдуть на `L−M`, fail при `L<M` — збережена
  decompiled-логіка), `N` = будь-який байт, `R` = клас
  розділювачів `{; : & ,}` + quirk `-` (лише 1-байтний решток),
  `L` = літера (приблизно ASCII/Cyrillic; property-таблиця
  `FUN_10011580` — окрема екстракція), інший байт = case-folded
  рівність (fold обидох боків — надмножина DLL-поведення).
  Синтетичні тести на всіх семантиках.
- **`.use` usage-прапорці декодовано** (`UseFile`): `lmu.use`/
  `lmr.use` = 64B-заголовок + **140B нульового body** (прапорці в
  цій базі не ввімкнені; `lmu.use` name = `lmU.use`).
- **Руська база завантажується** (`TridentIndex`): `lmru.vt`
  (`VtRecordStream`: **14421 record-ів** `0x40..0xd2648`, walk
  приходить в хвіст; стеблі `NET`/`PER`/`PO` підтверджені),
  `lmru.m` (`VmHeader`: u32-таблиця `0xC20, 0x88, 0xBD8, 0x56B,
  0x99, 0x1ACF, 0x2000, 0x25C1`, хвіст @0x53E). Руський join
  (словник `lmr.voc` + rus tsearch) — далі.
- **CLI `trident`** — нові блоки: `lmru.vt: 14421 record-ів
  0x40..0xd2648`, `lmru.m: u32-таблиця + хвіст @0x53e`, `lmU.use:
  204 Б (body 140 Б, усі 0)`, `tail-scan (FUN_100134d0) @0xccde5:
  N=0 (old-format .vt — хвіст порожній; u16 таблиці — з .voc
  B-tree, v0.21+)`.
- **130 юніт-тестів**. Guitest baseline `rows=430` без змін.
  Версія **0.19.0 → 0.20.0**.

### EN
- **The `.vt` tail section decoded (v0.20.0 finding): in the
  old-format files it is EMPTY.** `VtTailScan` (`FUN_100134d0`: a
  `u8 N` at the tail + `N` `[NUL string][u16 list 0x0000]`
  records) yields **N = 0** on the real `lmur.vt` / `lmru.vt` —
  the tail pointer sits inside the last 4 file bytes (the LE u32
  itself). So the u16 word/predicate tables the join core needs
  (`ctx+0x30`/`ctx+0x44`, `FUN_100176c0`/`FUN_10015cd0`) are
  derived by the DLL read-mode **from the `.voc` B-tree, not the
  `.vt` tail** — the next RE step (v0.21+).
- **The wildcard matcher `tail_match` is ported** (`FUN_100135d0`):
  `*` (stars advance by `L−M`, fail on `L<M` — the decompiled
  logic preserved), `N` = any byte, `R` = the separator class
  `{; : & ,}` + the `-` quirk (only a 1-byte remainder), `L` = a
  letter (approximated as ASCII/Cyrillic; the `FUN_10011580`
  property table is a separate extraction), any other byte = a
  case-folded equality (both sides folded — a superset of the
  DLL behavior). Synthetic tests over all the semantics.
- **The `.use` usage flags decoded** (`UseFile`): `lmu.use` /
  `lmr.use` = the 64-byte header + an **all-zero 140-byte body**
  (the flags are not enabled in this base; `lmu.use` name =
  `lmU.use`).
- **The Russian base loads** (`TridentIndex`): `lmru.vt`
  (`VtRecordStream`: **14421 records** `0x40..0xd2648`, the walk
  lands on the tail; the `NET`/`PER`/`PO` stems confirmed),
  `lmru.m` (`VmHeader`: the u32 table `0xC20, 0x88, 0xBD8, 0x56B,
  0x99, 0x1ACF, 0x2000, 0x25C1`, tail @0x53E). The Russian join
  (the `lmr.voc` dictionary + the rus `tsearch`) comes next.
- **CLI `trident`** — new blocks: `lmru.vt: 14421 records
  0x40..0xd2648`, `lmru.m: u32 table + tail @0x53e`, `lmU.use:
  204 B (body 140 B, all zero)`, `tail-scan (FUN_100134d0)
  @0xccde5: N=0 (old-format .vt — the tail is empty; the u16
  tables come from the .voc B-tree, v0.21+)`.
- **130 unit tests**. The guitest baseline `rows=430` is
  unchanged. Version **0.19.0 → 0.20.0**.

---

## v0.18.1 — 2026-10-27

### UK
- **Блок-розмір I/O-шару підтверджено: `WALK_BLOCK` = 4096** (було
  кандидат 512 в v0.18.0):
  - `DAT_1002ad98` (`FUN_10021940()`) **не має запису в `.text`**
    (byte-scan варіантів `C7 05` / `89 0D` / `A3` з абсолютним
    disp32 — 0 записів, лише 5 зчитувань `8B 35`/`A1`); отже глобальна
    зберігає початкове значення образу `.data` = `0x00001000` =
    **4096** (page-sized буфер читання);
  - незалежне підтвердження даними: resync-лондінги listA
    (packed B-tree) при блок=4096 лежать **точно** на 4096-алінованих
    node-slots `0x69000, 0x6A000, … 0x77000` (15 слотів, step 0x1000);
  - при 4096: listA обходить 388 resync-записів, listB — 382 (обидва
    landings `0x78000`, за EOF); чисті секції (f/ff/афікси/33-слів/
    +0x54/суфікси) не залежать від блоку — лондінги не змінюються.
- **B-tree пошуковий ланцюг відстежено** (розбирання
  `lmsup32.dll`, документація `trident_voc.rs` уточнена): root
  u16-таблиця `+0x3C` = **корінь packed B-tree** (не opcode-VM):
  `FUN_10002ff0` (seek до `handle+0x3C`) → `FUN_10002b70` (case-fold
  descent; прапорець `0x80` вузла = список дітей, кількість `&0x7f`)
  → `FUN_10002620`/`FUN_10001920` (node-ID → запис через 4-байтовий
  **node pointer pool**, `+2` = ключ) → `FUN_100018b0`/`FUN_100019f0`
  (payload: стоп NUL/`0xFF`/`0x42`, `0x49` = skip-маркер). Точні
  leaf→`.vt` posting-офсети — наступний крок (v0.19.0).
- Версія **0.18.0 → 0.18.1** (єдине джерело: `CARGO_PKG_VERSION`).
- 114 юніт-тестів (асерти listA/listB оновлено під 4096: 388/382
  resync-записи, 15 4096-алінованих слотів); guitest baseline
  `rows=430` без змін.

### EN
- **I/O-layer block size confirmed: `WALK_BLOCK` = 4096** (was the
  512 candidate in v0.18.0):
  - `DAT_1002ad98` (`FUN_10021940()`) has **no store in `.text`**
    (byte-scan of the `C7 05` / `89 0D` / `A3` forms with absolute
    disp32 — zero stores, only five `8B 35`/`A1` loads); the global
    therefore keeps its `.data` image initial value `0x00001000` =
    **4096** (the page-sized read buffer);
  - independent data-driven confirmation: the listA (packed B-tree)
    resync landings at block=4096 sit **exactly** on the 4096-aligned
    node slots `0x69000, 0x6A000, … 0x77000` (15 slots, step 0x1000);
  - at 4096: the listA walk covers 388 resync records, listB 382
    (both land at `0x78000`, just past EOF); the clean sections
    (f/ff/affixes/33-word/+0x54/suffixes) are block-independent —
    their landings are unchanged.
- **The B-tree search chain traced** (disassembled `lmsup32.dll`;
  the `trident_voc.rs` docs corrected): the root u16 table `+0x3C` is
  a **packed B-tree root** (not an opcode-VM): `FUN_10002ff0` (seek
  to `handle+0x3C`) → `FUN_10002b70` (case-fold descent; the node's
  `0x80` flag = the child list, count `&0x7f`) →
  `FUN_10002620`/`FUN_10001920` (a node id resolves to a record via
  the 4-byte **node pointer pool**, `+2` = the key) →
  `FUN_100018b0`/`FUN_100019f0` (payload walk: stop at NUL / `0xFF` /
  `0x42`, `0x49` = a skip marker). The exact leaf → `.vt` posting
  offsets remain the next step (v0.19.0).
- Version **0.18.0 → 0.18.1** (single source: `CARGO_PKG_VERSION`).
- 114 unit tests (the listA/listB assertions updated for 4096:
  388/382 resync records, 15 4096-aligned slots); the guitest
  baseline `rows=430` is unchanged.

---

## v0.18.0 — 2026-10-27

### UK
- **Граф лем-граматики `lmu.g` структурно декодовано** (нова
  `src/g_graph.rs`, підключена до `TridentIndex.graph` + CLI-дієслово
  `trident`):
  - заголовок: 0x00–0x17 вендор «Trident Software, Inc.» + NUL;
    0x18–0x1E «grammar»; 0x22 мовний символ ('U'/'R'); 0x24–0x2F
    12-байтовий slot шаблону («TEMPLATE.TXT» / «lmr.gt»);
  - u16-потік графа @0x60 (lmu.g: 31796 u16, head
    `0002 001c 0002 000c 0001 dd0a 0000 014c…`);
  - хвіст: NUL-терміновані маркери **POINT1/POINT2/STEP/REPEAT**
    (всі чотири — в обох файлах);
  - **`.gw` weight-стрим** (lmu.gw: 134 u16; лише 4 значення з
    прапором `0x8000` — термінатори; last-six
    `8053 8054 8057 8058 0000 0000`).
- **Record-walker `.voc` — `FUN_10004230`** (розширення
  `src/trident_voc.rs`, `TridentIndex.morphology.walks` + новий блок
  CLI `trident` «записники»):
  - довжини записів по режимах (розбирання `lmsup32.dll`):
    `'n'` = `record_len_n` (по-байтовий скан до першого `0x00`,
    ключове: DLL рухає `short*` **по 1 байту** —
    `(short *)((int)p + 1)`), `'f'` = `vm_record_len`, `0xff` =
    `record_len_ffff` (u16-потік до `0xFFFF`, термінатор
    включено), `'e'` = `record_len_e` (byte-VM: `J`+1,
    `F`/`G`/`C`/`E`+2, стоп `0xFF`);
  - `walk_section_records` — правило просування DLL: NUL після
    запису → `+1` (подвійний NUL), інакше **block-resync** до
    межі блоку (`block` = `FUN_10021940()` = `DAT_1002ad98`,
    розмір буфера I/O-шару; точні лондінги незалежні від
    розміру блоку в межах 128..=4096, кандидат 512);
  - section→mode мапа з завантажувача `FUN_100032d0`:
    `+0x48`→'f', `+0x58`→ff, `+0x4C/0x50/0x54/0x60/0x64/0x68/0x6C/
    0x70`→'n', `+0x5C`→raw seek;
  - **підтверджені точні кордони** (lmu.voc): f @`0x75253`
    (142) → `0x764C5`; ff @`0x764C5` (12) → `0x765B1`; афікси
    @`0x765B2` (7) → `0x765E1`; 33-слів @`0x765E1` → `0x7667A`;
    +0x54 @`0x76691` (32) → `0x76714`; суфікси @`0x76714` (722)
    → `0x77B78` — кожен лондінг = точний офсет наступної
    секції; listA (722) обходить усі свої записи, listB (2157)
    обходить 819 і resync'ається за кінець файлу (`0x78000`) —
    payload-семантика B-tree-вузлів (join word→.vt) лишається
    наступним кроком (v0.19.0).
- Версія **0.17.0 → 0.18.0** (єдине джерело: `CARGO_PKG_VERSION`).
- **114 юніт-тестів** (нові: `trident_voc::tests::{record_len_…
  ×4, walk_records_land_on_the_verified_section_boundaries,
  walk_landings_are_block_size_independent, morphology_walks_
  cover_the_walkable_slots, section_mode_maps_the_loader_slots}`
  + `g_graph::tests::{…}` ×6); guitest baseline `rows=430` без
  змін.

### EN
- **The `.g` grammar graph decoded structurally** (new
  `src/g_graph.rs`, wired into `TridentIndex.graph` + the
  `trident` CLI verb):
  - the header: 0x00–0x17 vendor «Trident Software, Inc.» + NUL;
    0x18–0x1E «grammar»; 0x22 the language char ('U'/'R');
    0x24–0x2F the 12-byte template slot («TEMPLATE.TXT» /
    «lmr.gt»);
  - the graph u16 stream @0x60 (lmu.g: 31796 u16, head
    `0002 001c 0002 000c 0001 dd0a 0000 014c…`);
  - the tail: NUL-terminated **POINT1/POINT2/STEP/REPEAT**
    markers (all four present in both files);
  - **the `.gw` weight stream** (lmu.gw: 134 u16; only 4 values
    carry the `0x8000` flag — the terminators; last six
    `8053 8054 8057 8058 0000 0000`).
- **The `.voc` record walker — `FUN_10004230`** (extended
  `src/trident_voc.rs`, `TridentIndex.morphology.walks` + a new
  `trident` CLI «записники» block):
  - the per-mode record lengths (disassembled `lmsup32.dll`):
    `'n'` = `record_len_n` (byte scan to the first `0x00`; the
    key point: the DLL advances the `short*` **one byte at a
    time** — `(short *)((int)p + 1)`), `'f'` = `vm_record_len`,
    `0xff` = `record_len_ffff` (u16 stream to `0xFFFF`, the
    terminator included), `'e'` = `record_len_e` (byte VM:
    `J`+1, `F`/`G`/`C`/`E`+2, stop at `0xFF`);
  - `walk_section_records` — the DLL's advance rule: a NUL after
    the record → `+1` (the double NUL), otherwise a **block
    resync** to the block boundary (`block` = `FUN_10021940()` =
    `DAT_1002ad98`, the I/O layer's buffer size; the verified
    landings are block-size-independent for 128..=4096, 512 is
    the candidate);
  - the section→mode map from the `FUN_100032d0` loader:
    `+0x48`→'f', `+0x58`→ff, `+0x4C/0x50/0x54/0x60/0x64/0x68/
    0x6C/0x70`→'n', `+0x5C`→raw seek;
  - **the verified exact boundaries** (lmu.voc): f @`0x75253`
    (142) → `0x764C5`; ff @`0x764C5` (12) → `0x765B1`;
    affixes @`0x765B2` (7) → `0x765E1`; 33-word @`0x765E1` →
    `0x7667A`; +0x54 @`0x76691` (32) → `0x76714`; suffixes
    @`0x76714` (722) → `0x77B78` — each landing is exactly the
    *next* section's offset; the listA (722) walk covers all of
    its records, the listB (2157) walk covers 819 and
    resynchronizes just past the file end (`0x78000`) — the
    B-tree node payload semantics (the word→.vt join) remain the
    next step (v0.19.0).
- Version **0.17.0 → 0.18.0** (single source: `CARGO_PKG_VERSION`).
- **114 unit tests** (new: `trident_voc::tests::{record_len_…
  ×4, walk_records_land_on_the_verified_section_boundaries,
  walk_landings_are_block_size_independent, morphology_walks_
  cover_the_walkable_slots, section_mode_maps_the_loader_slots}`
  + `g_graph::tests::{…}` ×6); the guitest baseline `rows=430`
  is unchanged.

### Roadmap (v0.19.0)
- Точні word→postings кордони: payload B-tree-вузлів listA/listB
  (join у `.vt`-посилання) — споживчий код пошуку в
  `lmsup32.dll`; семантика ребер `.g`-графа (template-walk
  споживач u16-потіку); блок-розмір `DAT_1002ad98` (кандидат
  512, запис у I/O-шарі).

## v0.17.0 — 2026-10-27

### UK
- **Точна `.voc`-таблиця секцій + морфологічні таблиці** (нова
  `src/trident_voc.rs`, підключена до `TridentIndex.morphology` +
  CLI-дієслово `trident`):
  - `VocSectionTable` — 17 u32-записів @0x3C (підтверджено
  по-байтовому в `lmu.voc`): root=`0x6832F`, listA=`0x68368`
  (722 записів), listB=`0x6CA96` (2157), f-записи=`0x75253` (142),
  суфікси=`0x76714` (722), афікси=`0x765B2` (7), таблиця 33 слів
  `0x765E1`, двозначники `0x7668F` (у lmu.voc порожня);
  - **opcode-VM, довжина в БАЙТАХ** (розбирання `lmsup32.dll`
  Ghidra-12.1): `vm_program_byte_len` (`FUN_100044f0`, u16-потік,
  depth-лічильник `0x1001`/`0x1002`), `vm_record_scan_len`
  (`FUN_100044c0`, +скан до першого `0xFF`), `vm_record_len`
  (`FUN_100044a0`, `L + 3*count + 1`);
  - **морфологія lmu.voc декодована**: 7 афіксів
  (`най(1), по-(3), по(4), не(2), спів(6), анти(5), псевдо(5)`),
  722 суфікси/кінцівки (у т.ч. дефісними «ам-», «і-»), подвійний
  NUL-термінатор записів; «конституція» як сирі LmCode-байти в
  lmu.voc **не існує** (лише в щільному B-tree) — доведено;
  - `lm_word_display` — показ слів таблиць (дефіс, `!`-маркери,
    LmCode-літери) без `·2d·`.
- **Кодувальник бази докодовано** (`src/ftsearch.rs`):
  - **`є` = `0xBA`** — підтверджено сканом `.mb`: слова
    «застосується» (29 збігів) і «затверджується» (34)
    збігаються лише з байтом `0xBA` у позиції є;
  - `ґ`/`Ґ` складаються в слоти твердого **г/Г (`0xE3`/`0xC3`)** —
    база зберігає «загальний» саме так; `Ї` — в збережений
    нижній `ї` (`0xBF`) (присвійні в базі — нижнім реєстром);
  - **drift 222/223 оракула документовано**: 204/223 офсетів
    збігаються точно; 19 офсетів оракула з'їхали (слова в
    `758.mb` 198873→199101, `177.mb` +311, `words.mb` перебудовано)
    + 18 нових позицій — механізм точного слова **правильний**,
    223/223 на поточній базі недосяжний (снимок оракула старіший).
- 100 юніт-тестів (нові: `trident_voc::tests::{…}` ×7 +
  `ftsearch::tests::{zahalnyi_…, zastosuye_tsya_…}`).

### EN
- **Exact `.voc` section table + morphology tables** (new
  `src/trident_voc.rs`, wired into `TridentIndex.morphology` +
  the `trident` CLI verb):
  - `VocSectionTable` — 17 u32 entries @0x3C (verified byte-by-byte
    in `lmu.voc`): root=`0x6832F`, listA=`0x68368` (722 records),
    listB=`0x6CA96` (2157), f-records=`0x75253` (142),
    suffixes=`0x76714` (722), affixes=`0x765B2` (7), 33-word table
    `0x765E1`, digraphs `0x7668F` (empty in lmu.voc);
  - **opcode-VM byte-length functions** (disassembled
    `lmsup32.dll` with Ghidra 12.1): `vm_program_byte_len`
    (`FUN_100044f0`, u16 stream, `0x1001`/`0x1002` depth
    counter), `vm_record_scan_len` (`FUN_100044c0`, +scan to the
    first `0xFF`), `vm_record_len` (`FUN_100044a0`,
    `L + 3*count + 1`);
  - **lmu.voc morphology decoded**: 7 affixes
    (`най(1), po-(3), po(4), не(2), спів(6), анти(5), псевдо(5)`),
    722 suffix/endings (incl. hyphenated «ам-», «і-»), the double
    NUL record terminator; «конституція» as raw LmCode bytes does
    **not exist** in lmu.voc (packed B-tree only) — proven;
  - `lm_word_display` — table-word rendering (hyphens, `!` markers,
    LmCode letters) without `·2d·` placeholders.
- **Base codepage completed** (`src/ftsearch.rs`):
  - **`є` = `0xBA`** — confirmed by an `.mb` scan: the words
    «застосується» (29 hits) and «затверджується» (34) match only
    with `0xBA` in the є-position;
  - `ґ`/`Ґ` fold onto the hard-г/Г slots (`0xE3`/`0xC3`) — the base
    stores «загальний» that way; `Ї` folds onto the stored lower
    `ї` (`0xBF`) (possessives are stored lower-case);
  - **the 222/223 oracle drift is documented**: 204/223 offsets
    match exactly; 19 oracle offsets drifted (a `758.mb` word moved
    198873→199101, `177.mb` +311, `words.mb` rebuilt) + 18 new
    positions — the exact-word mechanism is **correct**, 223/223 is
    not reachable on the current base (the oracle snapshot is
    older).
- 100 unit tests (new: `trident_voc::tests::{…}` ×7 +
  `ftsearch::tests::{zahalnyi_…, zastosuye_tsya_…}`).

---

## v0.16.0 — 2026-10-27

### UK
- **Виявлено базовий кодувальник документів** (`src/ftsearch.rs`) —
  текст документів `.mb` у базі — це **не** звичайний cp1251 і не
  LmCode, а cp1251 (стандартна кирилиця) з українськими літерами у
  символічному діапазоні `0x80–0xBF`:
  - стандартна кирилиця (cp1251): великі `А–П`=`0xC0–0xCF`,
    `Р–Я`=`0x90–0x9F`; малі `а–п`=`0xE0–0xEF`, `р–я`=`0xF0–0xFF`;
  - українські перевизначення: **`І`=`0xB2`, `і`=`0xB3`**
    (підтверджено — «Конституція» в `class1.mb` = `CA EE ED F1 F2 E8
    F2 F3 F6 B3 FF` на самому офсеті оракула 42 = 0x2A); `Є`=`0xAA`,
    `ї`=`0xBF` (визначено з тексту); решта слотів (`є`, `ґ`, `Ї`, `Г`)
    — рідкісні, далі.
- **`ftsearch` — повний текстовий пошук по `.mb`** (нова
  `src/ftsearch.rs` + CLI-дієслово `ftsearch <запит>`): кодує запит
  через базовий кодувальник і сканує **всі** `.mb`-файли
  (верхній рівень + `text\`, рекурсивно) у двох реєстрах (нижній +
  перша велика) — **саме механізм оракула** старої програми, без
  B-tree. Зв'язка native→оракел реалізована: «конституція» →
  **222 збіги** (оракел = 223, 99,5%), і перші 40 збігів збігаються
  з оракулом 1-до-1 (`class1.mb @ 42`, `1042.mb @ 1840761`, …).
  Пошук за стеমে («конституц» = ~12 000) **не** оракул — підтверджено,
  що оракул — точне слово (case-insensitive).
- **Дієслово `tsearch`** (нативний B-tree-індекс) лишається
  експериментальним; повний декод щільного B-tree + word→postings
  кордони — v0.17.0. 91 юніт-тест.

### EN
- **Discovered the base document codepage** (`src/ftsearch.rs`) — the
  `.mb` document text is **not** plain cp1251 or LmCode: it is cp1251
  (standard Cyrillic) with the Ukrainian letters placed in the
  `0x80–0xBF` symbol range:
  - standard Cyrillic (cp1251): upper `А–П`=`0xC0–0xCF`, `Р–Я`
    =`0x90–0x9F`; lower `а–п`=`0xE0–0xEF`, `р–я`=`0xF0–0xFF`;
  - Ukrainian overrides: **`І`=`0xB2`, `і`=`0xB3`** (confirmed —
    «Конституція» in `class1.mb` = `CA EE ED F1 F2 E8 F2 F3 F6 B3 FF`
    at the oracle's own offset 42 = 0x2A); `Є`=`0xAA`, `ї`=`0xBF`
    (read from the text); the remaining slots (`є`, `ґ`, `Ї`, `Г`) are
    rare — later.
- **`ftsearch` — full-text search over the `.mb` files** (new
  `src/ftsearch.rs` + the `ftsearch <query>` CLI verb): encodes the
  query through the base codepage and scans **all** `.mb` files
  (top level + `text\`, recursive) in two cases (lower + title) —
  **exactly the legacy app's oracle mechanism**, no B-tree needed. The
  native→oracle join is implemented: «конституція» → **222 hits**
  (oracle = 223, 99.5%), and the first 40 hits match the oracle
  1-to-1 (`class1.mb @ 42`, `1042.mb @ 1840761`, …). The stem search
  («конституц» ≈ 12 000) is *not* the oracle — confirming the oracle is
  an exact-word (case-insensitive) match.
- The `tsearch` verb (native B-tree index) stays experimental; the full
  packed B-tree decode + word→postings boundaries — v0.17.0. 91 unit
  tests.

---

## v0.15.0 — 2026-10-27

### UK
- **Trident: ЗАГАЛЬНИЙ формат записів словника**
  (`src/trident.rs::extract_dict_words`) — формат запису
  `[tag][word NUL][flag]` (tag 0x84/0x8d, flag 0x00/0x01), а не лише
  суворий `[0x84][word][00 00]`. Вилучення словника з `lmu.voc`
  зросло з **78 до ~1002 унікальних стем** (O(n)-скан з обмеженим
  12-байтовим пошуком вперед). Словник тепер містить реальні
  українські стеми — «зувати», «раст»/«ряст», «зват», «дити»,
  «овувати» — разом із морфологічними клас-кодами (`зувати0`,
  `титиь33`).
- **`.gw` — морфологічна таблиця ваг** (`read_gw_u16`):
  `lmu.gw` (268 B = 134 u16) / `lmr.gw` (264 B = 132 u16) —
  плоский little-endian u16-масив без заголовка; бит `0x8000` —
  прапорець (ставиться на останніх записах, напр. `8053 8054 8057
  8058`). Завантажується в `TridentIndex::gw_u16` (структурне
  декодування; повний семантичний декод потребує графу `.g` —
  roadmap).
- **Б-дерево / word-структури задокументовано** (з `lmsup32.dll`):
  пошук слова `FUN_10021520` = лінійний скан **NULL-термінованого
  масиву вказівників на word-структури**; word-байти NUL-терміновані
  за offset +1 у структурі (≥0xe1 байт, поля: +0xc0 ptr, +0xd5/+0xd7
  flag-байти, +0xdd ptr); порівняння — `FUN_1001bca0` (case-fold:
  кожен байт через `FUN_10011930(x,0x80)`). «конституція»
  (`aa ae ad e1 …`) **немає** як сирого байт-рядка в `lmu.voc`
  (0 збігів) — повний набір слів у **щільному (Patricia-) B-дереві**,
  декод якого — наступний крок.
- **`tsearch` влучає реальні стеми**: «раст» → 279 документів,
  «зват» → 294 документів (раніше 0). «п30» → 310 (було 2670 —
  пропорційна оцінка тепер ділиться на повний словник 1002 слова, а
  не 78). «конституція» → 0 (щільне B-дерево + join native-ID →
  docnum поточної бази — далі в roadmap). 86 юніт-тестів.

### EN
- **Trident: GENERAL dictionary record format**
  (`src/trident.rs::extract_dict_words`) — the record format is
  `[tag][word NUL][flag]` (tag 0x84/0x8d, flag 0x00/0x01), not only
  the strict `[0x84][word][00 00]`. Extraction from `lmu.voc` grew
  from **78 to ~1002 unique stems** (O(n) scan with a bounded 12-byte
  forward look). The dictionary now contains real Ukrainian stems —
  «зувати», «раст»/«ряст», «зват», «дити», «овувати» — together with
  morphological class-codes (`зувати0`, `титиь33`).
- **`.gw` — morphology weight table** (`read_gw_u16`):
  `lmu.gw` (268 B = 134 u16) / `lmr.gw` (264 B = 132 u16) — a flat
  little-endian u16 array with no header; the `0x8000` bit is a flag
  (set on the last entries, e.g. `8053 8054 8057 8058`). Loaded into
  `TridentIndex::gw_u16` (structural decode; the full semantic decode
  needs the `.g` grammar graph — roadmap).
- **B-tree / word-struct layout documented** (from `lmsup32.dll`):
  word lookup `FUN_10021520` = linear scan of a **NULL-terminated
  array of word-struct pointers**; word bytes are NUL-terminated at
  offset +1 in the struct (≥0xe1 bytes; fields +0xc0 ptr, +0xd5/+0xd7
  flag bytes, +0xdd ptr); comparison — `FUN_1001bca0` (case-fold:
  each byte through `FUN_10011930(x,0x80)`). «конституція»
  (`aa ae ad e1 …`) is **absent** as a raw byte run in `lmu.voc`
  (0 hits) — the full word set lives in the **packed (Patricia)
  B-tree**, whose decode is the next increment.
- **`tsearch` now hits real stems**: «раст» → 279 docs, «зват» → 294
  docs (was 0). «п30» → 310 (was 2670 — the proportional estimate now
  divides by the full 1002-word dictionary, not 78). «конституція» →
  0 (packed B-tree + native-ID → current docnums join — next on the
  roadmap). 86 unit tests.

---

## v0.14.0 — 2026-10-26

### UK
- **Trident: ПОВНА таблиця відповідності знаків**
  (`src/trident.rs::LM_CHAR_TABLE`) — тепер відомий повний
  cp1251-порядок слотів LmCode, підтверджений трима незалежними
  джерелами:
  1. `windows.kod` кодує cp1251-байти → слоти: `c0–df → 80–9f`
     (А–Я), `e0–ef → a0–af` (а–п), `f0–ff → e0–ef` (р–я),
     додаткові `a5/b4/aa/ba/b2/b3/af/bf → f2–f9`;
  2. DLL `lmsup32.dll` містить QWERTY-алфавітну таблицю, що
     розшифровується в ті самі слоти;
  3. реальні слова `lmu.voc` («нема», «бути», «Мин») читаються
     коректним українським **лише** в цьому порядку.
  Повний діапазон: великі 0x80–0x9F, малі 0xA0–0xAF і 0xE0–0xEF,
  українські 0xF2–0xF9 (Ґ/ґ=0xF2/0xF3, Є/є=0xF4/0xF5, І/і=0xF6/0xF7,
  Ї/ї=0xF8/0xF9). Попередні «якорі» з секції `G:` lm.a
  (А=0x84, О=0x96, У=0x9C, Щ=0x9F) виявилися помилковим
  прочитанням міжмовних якорів; цифри та Latin — ASCII.
- **`tsearch` показує кодування + розшифровку**: для кожного
  терміну `код: «термін» → [hex-байти] → «розшифрування»`.
  «конституція» тепер кодується на **100%** (було ~45%):
  `[aa ae ad e1 e2 a8 e2 e3 e6 f7 ef]` → «конституція» (повний
  раунд-трип). `п30` → 2670 Trident-документів (точний збіг з
  попереднім, але тепер без хибного с=0xAF); `с3` → 0 (власне слово).
- Дода `lm_byte_to_char` / `decode_lm_word` (зворотний декодер) і
  `TridentIndex::encode_query` (терміни → LmCode-байти).
- Версія **0.13.0 → 0.14.0** (єдине джерело: `CARGO_PKG_VERSION`).
- 84 юніт-тести (+нові: `char_map_is_the_full_cp1251_correspondence`,
  `lm_byte_to_char_round_trips`, `decode_lm_word_reproduces_real_
  dictionary_words`; оновлено таблицю знаків і покриття).

### EN
- **Trident: FULL character correspondence**
  (`src/trident.rs::LM_CHAR_TABLE`) — the complete cp1251-based
  LmCode slot order is now known, confirmed by three independent
  sources:
  1. `windows.kod` maps cp1251 bytes → slots: `c0–df → 80–9f`
     (А–Я), `e0–ef → a0–af` (а–п), `f0–ff → e0–ef` (р–я), extras
     `a5/b4/aa/ba/b2/b3/af/bf → f2–f9`;
  2. the `lmsup32.dll` QWERTY alphabet table decodes to the same
     slots;
  3. the real `lmu.voc` words («нема», «бути», «Мин») read as
     correct Ukrainian **only** under this order.
  Full range: upper 0x80–0x9F, lower 0xA0–0xAF and 0xE0–0xEF,
  Ukrainian 0xF2–0xF9 (Ґ/ґ=0xF2/0xF3, Є/є=0xF4/0xF5, І/і=0xF6/0xF7,
  Ї/ї=0xF8/0xF9). The earlier `G:`-section "anchors" (А=0x84,
  О=0x96, У=0x9C, Щ=0x9F) were a misread of the cross-language
  anchors; digits and Latin are ASCII.
- **`tsearch` now shows the encoding + decode**: per term
  `code: «term» → [hex bytes] → «decoding»`. «конституція» now
  encodes at **100%** (was ~45%):
  `[aa ae ad e1 e2 a8 e2 e3 e6 f7 ef]` → «конституція» (full
  round-trip). `п30` → 2670 Trident docs (exact match, now without
  the spurious с=0xAF); `с3` → 0 (its own word).
- Added `lm_byte_to_char` / `decode_lm_word` (the reverse decoder)
  and `TridentIndex::encode_query` (terms → LmCode bytes).
- Version **0.13.0 → 0.14.0** (single source: `CARGO_PKG_VERSION`).
- 84 unit tests (+ new: `char_map_is_the_full_cp1251_correspondence`,
  `lm_byte_to_char_round_trips`, `decode_lm_word_reproduces_real_
  dictionary_words`; char table + coverage tests updated).

### Roadmap (v0.15.0)
- Морфологія `.g`/`.gw` (граф/ваги), B-tree-словник (повний
  перелік слів, не лише 78 записів-референтів) і зв'язка
  native-ID → номери документів поточної бази (оракел «конституція»
  = 223 хіти). / The `.g`/`.gw` morphology graphs, the full B-tree
  dictionary (not just the 78 reference records) and the
  native-ID → current docnums join (the «конституція» oracle = 223
  hits).

---

## v0.13.0 — 2026-10-26

### UK
- **Trident: експериментальний нативний рушій** (`src/trident.rs`) —
  декод частин нативного FTS-індексу старого застосунку в папці
  `SOFT\` бази:
  - `windows.kod` (683 Б): кодова таблиця, 80 рядків
    `<2hex-code> <sep> <byte>` — значення ≥0x80 записано сирийм
    байтом, <0x80 — двома ASCII-hex цифрами; сепаратори `=`
    (мапінг) і `>` (fold);
  - `lm.a` (904 Б): case-fold таблиці, текстові секції `E:`
    (Latin identity A…Z), `G:` (міжмовні якорі: A↔84/a4,
    O↔96/e6, U↔9c/ec, Щ↔9f/ef), `R:`/`U:` — по 32 fold-пари
    (в U головний діапазон пропускає 9a/9b/9d, додаткові пари
    f4f5/f6f7/f8f9);
  - записи словника `[0x84][word][00 00]` (lmu.voc — 78 записів)
    і u16 posting-потік `.vt` (419540 u16, ідентифікатори
    документів 0..65535 — native IDs 2005-індексу).
- **`tsearch <query>`** (CLI, експеримент): терміни запиту
  кодуються через LmCode-таблицю знаків (підтверджені якорі
  А/О/У/Щ + цифри + Latin; с/і — виведені за частотою у
  словнику, підтвердження — v0.14.0), збігаються зі словником
  lmu.voc (префікси + стем-обрізки 1–3 хвостики), postings
  читаються з lmur.vt. Результат — native Trident-ідентифікатори
  документів + покриття (% закодованих знаків запиту). Зв'язка
  native-ID з номерами документів поточної бази — v0.14.0
  (разом з морфологією .g/.gw і точними word→postings кордонами).
- **Команда `trident` розширена** — інвентар 16 файлів (додано
  windows.kod, lm.a, lmu.use, lmr.use до попередніх 12) +
  декодована статистика: число kod-кодів, R/U fold-пари,
  записи словника, vt-потік (min..max ідентифікаторів).
- **GUI**: чекбокс «Trident-рушій (нативний індекс,
  експеримент)» — за позначення пошук йде через нативний індекс
  (SOFT\); рядки показують native IDs (стовпець файлу
  «trident (native id)», реквізитні стовпці — даші, доки не
  зроблена зв'язка номерів); без SOFT\-індексу — повідомлення в
  статус-барі. Чекбокс вимкнено за замовчуванням → звичайний
  пошук, guitest незмінний.
- Версія **0.12.0 → 0.13.0** (єдине джерело: `CARGO_PKG_VERSION`).
- 82 юніт-тести (нові: `trident::tests` ×11 — kod, fold,
  розділи lm.a, таблиця знаків, покриття, словник, vt,
  префіксний пошук + інтеграційний на реальному індексі).

### EN
- **Trident: experimental native engine** (`src/trident.rs`) —
  decodes parts of the legacy app's native FTS index in the base's
  `SOFT\` folder:
  - `windows.kod` (683 B): the code table, 80 lines
    `<2hex-code> <sep> <byte>` — values ≥0x80 stored as a raw
    byte, <0x80 as two ASCII-hex digits; separators `=`
    (mapping) and `>` (fold);
  - `lm.a` (904 B): case-fold tables, text sections `E:` (Latin
    identity A…Z), `G:` (cross-language anchors: A↔84/a4,
    O↔96/e6, U↔9c/ec, Щ↔9f/ef), `R:`/`U:` — 32 fold pairs each
    (U skips 9a/9b/9d in the main range; extra pairs
    f4f5/f6f7/f8f9);
  - dictionary records `[0x84][word][00 00]` (lmu.voc — 78
    records) and the u16 posting stream from `.vt` (419540 u16,
    doc ids 0..65535 — native ids of the 2005-era index).
- **`tsearch <query>`** (CLI, experimental): query terms are
  encoded through the LmCode character table (confirmed anchors
  A/O/U/Щ + digits + Latin; С/І inferred from dictionary
  frequency, to be confirmed in v0.14.0), matched against the
  lmu.voc dictionary (prefix + stem trimming, 1–3 tails),
  postings are read from lmur.vt. The result is native Trident
  document ids plus a coverage % of encodable query characters.
  Joining the native ids to the current base's docnums — v0.14.0
  (with the .g/.gw morphology and the exact word→postings
  boundaries).
- **Extended `trident` verb** — a 16-file inventory (added
  windows.kod, lm.a, lmu.use, lmr.use to the previous 12) +
  decoded stats: kod codes, R/U fold pairs, dictionary records,
  vt stream (min..max doc ids).
- **GUI**: a "Trident-рушій (нативний індекс, експеримент)"
  checkbox — when checked, search runs through the native index
  (SOFT\); rows show the native ids (file column
  "trident (native id)", field columns dashed until the docnum
  join lands); without a SOFT\ index — a status-bar message.
  Unchecked by default → the classic search, guitest unchanged.
- Version **0.12.0 → 0.13.0** (single source: `CARGO_PKG_VERSION`).
- 82 unit tests (new: `trident::tests` ×11 — kod, fold, lm.a
  sections, the char table, coverage, dict, vt, prefix search +
  a real-index integration test).

---

## v0.12.0 — 2026-10-26

### UK
- **Підсвітка слів запиту в експорті** — новий флаг CLI `--hl`
  (аліас `--highlight`) для `export` + чекбокс «підсвітити слова
  запиту» в GUI (поруч із «зв'язка»). Слова запиту (≥2 знаків,
  регістронезалежно) позначаються в експортованому тексті: md
  `<mark>…</mark>`; html `<mark class="hl">` + CSS
  `mark.hl{background:#ffe86b}`; rtf жовлий хайлайт
  (`\highlight6`/`\highlight0`); docx — окремі біги з
  `<w:highlight w:val="yellow"/>`. pdf/csv — у наступних версіях.
  Ядро: `highlight::Hl` (слова → char-діапазони → партиціонування
  рядка; найдовше слово перетворює, діапазони не перетинаються);
  порожній `Hl` = хайлайт вимкнено (гарячий шлях, без кошту).
- **Партійний друк** — `print <docnum> [docnum ...]`: кілька
  документів (пробіл/кома, діапазони `1-5` через той самий
  розбір, що й `--docs`) друкуються один за одним; системний
  діалог друку — на кожен документ, статус друкується окремим
  рядком («надіслано в принтер» / «відмінено» / помилка).
- **Команда `trident`** — інвентар нативних FTS-індексів
  старої бази: `SOFT\` (lmu.voc / lmr.voc словники, lmur.vt /
  lmru.vt postings, lm*.m метадані, lmu.g / lmr.g графи,
  lmu.gw / lmr.gw ваги, lmsup.dll / lmsup32.dll рушії — 12
  файлів з розмірами). Без папки SOFT (наприклад, БАЗА GK) —
  коректне повідомлення, що індексів немає і пошук йде власним
  рушієм експортера. Повний Trident-рушій («пошук як у головному
  застосунку») — у розробці (v0.13.0).
- Випущено функцію section-headings (0 зі 120 зразкових
  документів — ознак не знайдено; зовільнює експорт).
- Версія **0.11.0 → 0.12.0** (єдине джерело: `CARGO_PKG_VERSION`).
- 71 юніт-тест (нові: `highlight::tests` ×6, hl-тести
  writer'ів ×6: md/html/rtf/docx/bundle) + GUI-самотест
  `--guitest` (тепер звітує `marks=N` — кількість `<mark>` в
  експортованих файлах, і перевіряє чекбокс підсвітки).

### EN
- **Search-term highlighting in exports** — new CLI flag `--hl`
  (alias `--highlight`) for `export` + a "підсвітити слова
  запиту" checkbox in the GUI (next to the bundle checkbox).
  Query terms (≥2 chars, case-insensitive) are marked in the
  exported text: md `<mark>…</mark>`; html `<mark class="hl">` +
  CSS `mark.hl{background:#ffe86b}`; rtf yellow highlight
  (`\highlight6`/`\highlight0`); docx — separate runs with
  `<w:highlight w:val="yellow"/>`. pdf/csv — a later version.
  Core: `highlight::Hl` (terms → char ranges → string partition;
  longest match wins, ranges do not overlap); empty `Hl` =
  highlighting off (hot path, zero cost).
- **Batch print** — `print <docnum> [docnum ...]`: several
  documents (space/comma separated, `1-5` ranges via the same
  parser as `--docs`) print one after another; the system print
  dialog appears per document and the per-document outcome
  ("sent to printer" / "cancelled" / error) is printed.
- **`trident` command** — inventory of the legacy base's native
  FTS indexes: `SOFT\` (lmu.voc / lmr.voc dictionaries, lmur.vt /
  lmru.vt postings, lm*.m metadata, lmu.g / lmr.g graphs,
  lmu.gw / lmr.gw weights, lmsup.dll / lmsup32.dll engines — 12
  files with sizes). When the SOFT folder is absent (e.g. the
  GK base) it reports that gracefully: no indexes, search runs
  on the exporter's own engine. The full Trident engine
  ("search like the main app") is in development (v0.13.0).
- The section-headings feature was dropped (0 of 120 sampled
  documents show any heading markers; it only slowed exports).
- Version **0.11.0 → 0.12.0** (single source: `CARGO_PKG_VERSION`).
- 71 unit tests (new: `highlight::tests` ×6, writer hl tests
  ×6: md/html/rtf/docx/bundle) + the GUI self-test `--guitest`
  now reports `marks=N` (count of `<mark>` in the exported
  files) and checks the highlight checkbox.

## v0.11.0 — 2026-10-26

### UK
- **«Зв'язка» (зв'язка / bundle): всі експортовані документи в ОДИН файл** —
  CLI `--single` (аліас `--zvizka`) для `export`/`bulk` + чекбокс «зв'язка
  (1 файл на всі документи)» в GUI: md/rtf/docx/html/pdf (csv і так один
  файл). Ім'я: `bundle-<кількість>-<перший docnum>.<ext>`. Кожен формат:
  шапка «Зв'язка — N документ(ів)» + нумерований Зміст (назви + № документів);
  роздільники: md `---`, rtf `\page`, docx `w:br w:type="page"` (кожен
  документ з нової сторінки), html `<section id>` + клікабельні якорі +
  CSS `page-break-before`. **PDF: `/Outlines` (зміст-заатчеки PDF)** — кожен
  документ з нової сторінки, entry з `/Dest` на початковий документ.
- **Авто-вибір шрифту PDF** (`pdf::select_font`): кандидатні
  `arial → times → cambria → georgia → tahoma → verdana`
  (`C:\Windows\Fonts`); обирається перший шрифт із 100% покриттям вжитих
  код-поінтів, інакше — найкраще покриття, інакше arial.ttf. «Stale»-мапінги
  (gid поза межами власної таблиці гліфів) лічать за НЕ покриті. Файли
  шрифтів парсяться один раз на процес (кеш `OnceLock`). `write_pdf` та
  `write_pdf_bundle` тепер вибрають шрифт автоматично (у зв'язці — об'єднання
  кодів усіх документів).
- Версія **0.10.0 → 0.11.0** (єдине джерело: `CARGO_PKG_VERSION`).
- 59 юніт-тестів (нові: структура зв'язок md/rtf/docx/html/pdf,
  `select_font` ×2, `font_coverage`) + GUI-самотест `--guitest` (тепер
  включає зв'язку: `files=1` в звіті).
- Нюанс: на розробничій машині жоден кандидатний шрифт не покриває весь
  кириличний набір (stale arial + відсутній regular Cambria), тож селектор
  обирає arial (рівність покриття → перший у порядку пріоритетів); на
  машині зі звичайним Arial вибір падає на arial із повним покриттям.

### EN
- **“Зв'язка” (bundle): all exported documents into ONE file** — CLI
  `--single` (alias `--zvizka`) for `export`/`bulk` + a “зв'язка (1 файл)”
  checkbox in the GUI: md/rtf/docx/html/pdf (csv is already a single file).
  Name: `bundle-<count>-<first docnum>.<ext>`. Every format gets a
  “Зв'язка — N documents” header + a numbered table of contents (titles +
  doc numbers); separators: md `---`, rtf `\page`, docx `w:br w:type="page"`
  (each document starts on a fresh page), html `<section id>` + clickable
  anchors + CSS `page-break-before`. **PDF: `/Outlines` (PDF
  bookmarks/TOC)** — each document on a fresh page, entries with `/Dest`
  pointing at its start page.
- **Automatic PDF font selection** (`pdf::select_font`): candidates
  `arial → times → cambria → georgia → tahoma → verdana`
  (`C:\Windows\Fonts`); the first font with 100% coverage of the used
  codepoints wins, else the best coverage, else arial.ttf. “Stale” mappings
  (gid beyond the font's own glyph table) count as NOT covered. Font files
  are parsed once per process (`OnceLock` cache). `write_pdf` and
  `write_pdf_bundle` now auto-pick the font (bundle: the union of all
  documents' codepoints).
- Version **0.10.0 → 0.11.0** (single source: `CARGO_PKG_VERSION`).
- 59 unit tests (new: bundle structure md/rtf/docx/html/pdf,
  `select_font` ×2, `font_coverage`) + GUI self-test `--guitest` (now
  includes the bundle mode: `files=1` in the report).
- Note: on the dev machine no candidate font covers the full Cyrillic set
  (stale arial + missing regular Cambria), so the selector returns arial
  (coverage tie → first in preference order); on a machine with a standard
  Arial, arial is chosen with full coverage.

---

## v0.10.0 — 2026-10-25

### UK
- **GDI-друк** (Win32, без зовнішніх додатків): системний діалог друку (`PrintDlgW`)
  + GDI-цикл сторінок; форматування збережене: рядки ВЕЛИКОЮ — жирним, пункти
  списків «•  », центрований запис; відміна в діалозі — результат `Ok(Cancelled)`,
  не помилка. CLI: `zakonexport-cli print <docnum>`; GUI: кнопка «Друк» в вікні
  перегляду (статус: «Друк: завдання надіслано / відмінено / Помилка друку: …»).
- **PDF-експорт** — 6-й формат (CLI `--format pdf` + вибір формату в GUI):
  PDF 1.4, A4 12pt/ведення 15pt, DEFLATE-потоки, вбудований сабсет системного
  Arial (`C:\Windows\Fonts\arial.ttf`: cmap F4/F12, sfnt, FontFile2, кламп
  позагілсці gid на .notdef). Чистий Rust, без зовнішніх PDF-засобів.
- Версія **0.9.0 → 0.10.0** (єдине джерело: `CARGO_PKG_VERSION`).
- 51 юніт-тест (writers, pdf, db, render, …) + GUI-самотест `--guitest`.
- Нюанш шрифту: на розробничій машині системний `arial.ttf` не містить
  кириличних гліфів (GDI підставляє з'єднані шрифти); райтер клампає такі
  коди на `.notdef`. На машині зі звичайним Arial (повна кирилиця) обмеження
  не проявляється.

### EN
- **GDI printing** (Win32, no external apps): system print dialog (`PrintDlgW`)
  + GDI page loop; formatting preserved — ALL-CAPS lines bold, “•  ” list
  markers, centered lines; dialog cancel is an outcome `Ok(Cancelled)`, not an
  error. CLI: `zakonexport-cli print <docnum>`; GUI: “Print” button in the
  preview window (status: “Druk: sent / cancelled / print error: …”).
- **PDF export** — the 6th format (CLI `--format pdf` + GUI format combo):
  PDF 1.4, A4 12pt / 15pt leading, DEFLATE streams, embedded system-Arial
  subset (`C:\Windows\Fonts\arial.ttf`: cmap F4/F12, sfnt, FontFile2,
  out-of-range glyph ids clamped to `.notdef`). Pure Rust, no external
  PDF tooling.
- Version **0.9.0 → 0.10.0** (single source: `CARGO_PKG_VERSION`).
- 51 unit tests (writers, pdf, db, render, …) + GUI self-test `--guitest`.
- Font quirk: on the dev machine the system `arial.ttf` carries no Cyrillic
  glyphs (GDI substitutes linked fonts); the writer clamps those codes to
  `.notdef`. On a machine with a standard full-Cyrillic Arial this does not
  apply.

---

## v0.9.0 — 2026-10-25

### UK
- **Windows x64 бинарники** (CLI + GUI) **з іконкою**: `releases/v0.9.0/` у репо
  та як ассети GitHub-релізу.
- Ядро: пряме **read-only** читання баз Закону (`.db`/`.mb`, cp1251 + fold-кодек),
  паралельний пошук (rayon), кирилично-безпечний (Unicode-UI, `--queryfile`).
- Експорт з оформленням (капс-жирний, «•  », центровані рядки):
  `md` / `rtf` / `docx` / `html` / `csv` (5 форматів).
- Фільтри по реквізитах (`--status`, `--issuer`, `--type`, дати, `--title`),
  пошуки по номеру (`--number`, `--docs`, `--pick`), signal-додатки
  (`--attach`, `signal <docnum>`), `bulk`, `stats`.
- GUI (native-windows-gui): пошук в робочому потоці, перегляд, вибір формату,
  COM folder picker, `--guitest`.

### EN
- **Windows x64 binaries** (CLI + GUI) **with icon**: in-repo `releases/v0.9.0/`
  and as GitHub release assets.
- Core: direct **read-only** access to Zakon DB (`.db`/`.mb`, cp1251 + fold
  codec), parallel search (rayon), Cyrillic-safe (Unicode UI, `--queryfile`).
- Formatting-preserving export (caps-bold, “•  ” markers, centered lines) in
  `md` / `rtf` / `docx` / `html` / `csv` (5 formats).
- Filters by document fields (`--status`, `--issuer`, `--type`, dates,
  `--title`), number search (`--number`, `--docs`, `--pick`), signal
  attachments (`--attach`, `signal <docnum>`), `bulk`, `stats`.
- GUI (native-windows-gui): worker-thread search, preview, format combo,
  COM folder picker, `--guitest`.

---

## v0.8.0 / v0.7.0 / v0.6.0 / v0.4.0 — архівні (дата не збережена / date unknown)

### UK
- Архівні збірки GUI (zip), pre-git ера: детального журналу змін не зберігалося
  (історія версій не велася). Бинарники збережені користувачем і публікуються
  в `releases/zakonexport-gui-<v>.zip` та як ассети відповідних GitHub-релізів.

### EN
- Archived GUI builds (zip), pre-git era: detailed change lists were not
  preserved (version history did not exist yet). The binaries were kept by the
  user and are published under `releases/zakonexport-gui-<v>.zip` and as assets
  of the matching GitHub releases.

---

## v0.1 – v0.3 — перші версії (архівних бинарників немає / no archived binaries)

### UK
- Перші ітерації ядра: читання `.mb`, пошук, базові формати експорту.
  Бинарники не збереглися.

### EN
- Early core iterations: `.mb` reading, search, basic export formats.
  No archived binaries.
