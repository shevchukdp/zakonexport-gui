# v0.39.0 - Changes / Зміни (2026-10-29)

## UK / Українська

- **Hardening + production marking (roadmap v0.40.0).** Trident-маршрут
  позначено **production**:
  - **Статус-рядок** у слові `trident` (UA+EN): «Trident-маршрут: production
    (оракельно підтверджено, roadmap v0.40.0)» + EN-відповідник.
  - **100-запитний оракульний корпус** (UA+EN: статутні терміни + власні
    назви + стеми/префікси + міксовані/EN): **`corpus/trident_oracle_100.txt`**
    (100 рядків-запитів, у репозиторії) + нове **`tsearch --corpus <file>`**:
    кожен рядок проходить ПОВНУ tsearch-ланцюгу (encode → словник lmu.voc →
    точний `.vt` stem-join → native u16) і крос-чекається з ftsearch-оракулом.
    Оракул — **один `.mb`-прохід** (`oracle_multi`): новий **single-pass
    multi-pattern presence**-скан (3-байтний prefix-bucket + 256-бітний
    byte-set + короткий direct-чек для 2-байтових варіантів; worker-pool на
    файли) — замість одного проходу на варіант (≈200× менше руху даних;
    8.4 ГБ корпусу за ~2 хв замість 30+ хв).
  - **`TridentError`-поверхня** (`trident.rs`): замість тихих порожніх —
    іменовані режими **`NoIndex` / `NoMatch` / `EmptyJoin` / `OutOfRange`**
    (UA `Display`); **`classify(query, r, doc_count)`** (pure) +
    **`search_checked` / `search_auto_checked`** (пошук → `Result`);
    `tsearch` друкує явний рядок «Trident-помилка (v0.39.0): …» замість
    мовчазного «0 документів». Zero panics on partial data (тести).
- **Збіг-матриця 100-запитного корпусу (DoD, UA+EN).** `tsearch --corpus
  corpus\trident_oracle_100.txt --baza C:\Zakon.Local\Baza` (ukr; 102
  distinct термінів; оракул: 2388 `.mb`-файлів, один проход):
  **оракельно підтверджено (збіг ≥ 0.9) 11/100 · failure modes:
  OK 13 · NoMatch 67 · EmptyJoin 20 · OutOfRange 0 · zero-panic.**
  Зразок: не (стем) 967/2380 збіг 0.41 OK; митн (стем) 957/896 0.94
  підтверджено; МВС 1759/2385 0.74 OK; МОЗ 3367/2385 0.71 OK; ї 2511/2386
  0.95 підтверджено; конституція 0/97 NoMatch (packed-B-tree gap); закон
  0/2111 NoMatch; наказ (стем-префікс) 0/993 EmptyJoin; Київ 0/1657
  EmptyJoin; EN-проби 0/0 vacuous 1.00.
- **Failure-mode таблиця (DoD, UA+EN).** (1) **NoMatch** — термін відсутній
  у raw `.vt` byte-run (packed-B-tree word-set: «конституція», «закон»,
  «спів», «заст», «розк», «бук»); (2) **EmptyJoin** — термін у словнику
  (префікс-кандидат), але 0 `.vt`-record-ів несуть його стем; (3)
  **OutOfRange** — усі jointi u16 > doc_count (2005-простір ширший; на цій
  базі — 0 випадків, Identity-join); (4) **NoIndex** — базі немає SOFT\
  (guitest-база GK: `trident=noindex`). **Друга-база docnum-стабільність:**
  другої Trident-індексованої бази немає (GK-база — без SOFT\) → припущення
  задокументовано: Identity-bound `u16 ≤ doc_count` (613173), single-base
  корпус; Bounded-гіпотеза (u16 > doc_count) покрита синтетичним
  OutOfRange-тестом.
- **199 юніт-тестів** (нові 5: `classify_names_the_failure_modes`,
  `corpus_subset_zero_panic_on_real_base`, `corpus_file_has_100_queries`,
  `scan_present_early_break_matches_offsets`,
  `oracle_multi_multipattern_matches_variants`). Test suite швидша (115 s).
  Guitest baseline `rows=430` без змін. Версія **0.38.0 → 0.39.0**.
- Далі (v0.40.0 / roadmap v0.41.0): `.uvt` behavior (write-mode
  per-language update cache — synthesize on a base copy).

## EN / English

- **Hardening + production marking (roadmap v0.40.0).** The Trident route
  is marked **production**:
  - **Status line** in the `trident` verb (UA+EN): "Trident route:
    production (oracle-confirmed, roadmap v0.40.0)".
  - **100-query oracle corpus** (UA+EN: statutory terms + proper nouns +
    stems/prefixes + mixed/EN): **`corpus/trident_oracle_100.txt`** (100
    query lines, in-repo) + the new **`tsearch --corpus <file>`**: every
    line runs the FULL tsearch chain (encode → lmu.voc dictionary → exact
    `.vt` stem-join → native u16) and is cross-checked against the ftsearch
    oracle. The oracle is a SINGLE `.mb` pass (`oracle_multi`): the new
    **single-pass multi-pattern presence** scan (a 3-byte prefix-bucket + a
    256-bit byte set + a short direct check for 2-byte variants; a worker
    pool over the files) — instead of one pass per variant (~200× less data
    moved; the 8.4 GB corpus in ~2 minutes instead of 30+).
  - **`TridentError` surface** (`trident.rs`): instead of silent empties,
    the named modes **`NoIndex` / `NoMatch` / `EmptyJoin` / `OutOfRange`**
    (UA `Display`); **`classify(query, r, doc_count)`** (pure) +
    **`search_checked` / `search_auto_checked`** (search → `Result`);
    `tsearch` prints an explicit "Trident error (v0.39.0): …" line instead
    of a silent "0 documents". Zero panics on partial data (tested).
- **The 100-query agreement matrix (DoD record, UA+EN).** `tsearch --corpus
  corpus\trident_oracle_100.txt --baza C:\Zakon.Local\Baza` (ukr; 102
  distinct terms; oracle: 2388 `.mb` files, one pass): **oracle-confirmed
  (agreement ≥ 0.9) 11/100 · failure modes: OK 13 · NoMatch 67 · EmptyJoin
  20 · OutOfRange 0 · zero panics.** Sample rows: ne (stem) 967/2380
  agreement 0.41 OK; mityn (stem) 957/896 0.94 confirmed; MVS 1759/2385
  0.74 OK; MOZ 3367/2385 0.71 OK; yi 2511/2386 0.95 confirmed;
  constitution 0/97 NoMatch (packed-B-tree gap); law 0/2111 NoMatch;
  naky (stem prefix) 0/993 EmptyJoin; kyiv 0/1657 EmptyJoin; EN probes
  0/0 vacuous 1.00.
- **Failure-mode table (DoD, UA+EN).** (1) **NoMatch** — the term is
  absent from the raw `.vt` byte run (the packed-B-tree word set:
  «конституція», «закон», «спів», «заст», «розк», «бук»); (2)
  **EmptyJoin** — the term is in the dictionary (a prefix candidate), but
  no `.vt` record carries its stem; (3) **OutOfRange** — every joined u16
  is wider than the base's docnum space (0 cases on this base — the
  Identity join); (4) **NoIndex** — the base has no SOFT\ (the guitest GK
  base: `trident=noindex`). **Second-base docnum-stability:** no second
  Trident-indexed base is available (the GK base has no SOFT\) → the
  assumption is documented: the Identity bound `u16 ≤ doc_count` (613173),
  a single-base corpus; the Bounded hypothesis (u16 > doc_count) is
  covered by the synthetic OutOfRange test.
- **199 unit tests** (5 new: `classify_names_the_failure_modes`,
  `corpus_subset_zero_panic_on_real_base`, `corpus_file_has_100_queries`,
  `scan_present_early_break_matches_offsets`,
  `oracle_multi_multipattern_matches_variants`). The test suite is faster
  (115 s). The guitest baseline `rows=430` is unchanged. Version
  **0.38.0 → 0.39.0**.
- Next (v0.40.0 / roadmap v0.41.0): `.uvt` behavior (the engine's
  write-mode per-language update cache — synthesize on a base copy).
