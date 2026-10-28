# v0.25.0 - Changes / Зміни (2026-10-28)

## UK / Українська

- **Rus-узагальнення: language-generic loader + `tsearch --lang`.** Усе
  попереднє було Ukrainian-primary; rus-сторона (`lmr.*`) — ті самі
  формати. Редизайн `trident.rs`:
  - новий `Lang { Ukr, Rus }` + приватний `LangCtx` (словник + `.vt`
    stream + u16-простір + fold-таблиця для ОДНОЇ мови); всі
    search-методи (`search_in` / `find_prefix_in` / `posting_slice_in` /
    `exact_postings_in` / `term_join_docs_in`) тепер language-generic;
  - `TridentIndex` несе rus-ядра: `rus_dict_words` (з `lmr.voc`) +
    `rus_vt_u16` (з `lmru.vt`); loader завантажує обидві мови;
  - нові публічні: `search_lang(q, lang)`, `search_auto(q)` (об'єднання
    ukr+rus), `term_join_docs_lang`, `encode_query_lang`,
    `exact_postings_lang`.
- **CLI `tsearch --lang u|r|auto`**: `r` — rus (fold.rus + lmr.*),
  `auto` — обидві мови (merge, з мітками (ukr)/(rus)). `trident` —
  новий рядок «language-індекси (v0.25.0): ukr lmu.voc=1002 слів /
  lmur.vt 419540 u16 · rus lmr.voc=650 слів / lmru.vt 430854 u16
  (14421 record-ів) — tsearch --lang u|r|auto».
- **Емпірика**: rus-ядра реально завантажуються (lmr.voc=650 слів,
  lmru.vt=430854 u16 / 14421 record-ів, fold.rus=32 пар);
  `tsearch --lang r` «не» → 32 join record-ів (NET/HE в lmru.vt),
  «закон» → proportional fallback (474 док-ів, позначений «оцінка»).
  Обидві мови працюють «з коробки».
- **147 юніт-тестів** (нові 2: `rus_index_loads_the_rus_core` —
  14421 records + rus dict + fold.rus; `tsearch_lang_r_resolves_a_rus_stem`
  — rus-stem → join/fallback evidence + auto-merge). Guitest baseline
  `rows=430` без змін. Версія **0.24.0 → 0.25.0**.
- Далі (v0.26.0 / roadmap v0.27.0): per-word metadata (u16 word table →
  word idx → doc-ids semantic walk), `lmu.g` edge semantics.

## EN / English

- **Rus generalization: language-generic loader + `tsearch --lang`.**
  Everything before was Ukrainian-primary; the rus side (`lmr.*`) uses
  the same on-disk formats. `trident.rs` reworked:
  - a new `Lang { Ukr, Rus }` + a private `LangCtx` (dictionary + `.vt`
    stream + u16 space + fold table for ONE language); all the search
    methods (`search_in` / `find_prefix_in` / `posting_slice_in` /
    `exact_postings_in` / `term_join_docs_in`) are now language-generic;
  - `TridentIndex` carries the rus core: `rus_dict_words` (from
    `lmr.voc`) + `rus_vt_u16` (from `lmru.vt`); the loader reads both
    languages;
  - new public surface: `search_lang(q, lang)`, `search_auto(q)`
    (ukr+rus merge), `term_join_docs_lang`, `encode_query_lang`,
    `exact_postings_lang`.
- **CLI `tsearch --lang u|r|auto`**: `r` — Russian (`fold.rus` +
  `lmr.*`), `auto` — both languages (merged, tagged (ukr)/(rus)).
  `trident` — a new «language-індекси (v0.25.0): ukr lmu.voc=1002
  words / lmur.vt 419540 u16 · rus lmr.voc=650 words / lmru.vt 430854
  u16 (14421 records) — tsearch --lang u|r|auto» line.
- **Empirical**: the rus core loads for real (lmr.voc=650 words,
  lmru.vt=430854 u16 / 14421 records, fold.rus=32 pairs);
  `tsearch --lang r` «не» → 32 join records (NET/HE in lmru.vt),
  «закон» → proportional fallback (474 docs, labelled «estimate»). Both
  languages work out of the box.
- **147 unit tests** (2 new: `rus_index_loads_the_rus_core` — 14421
  records + the rus dict + fold.rus; `tsearch_lang_r_resolves_a_rus_stem`
  — a rus stem → join/fallback evidence + the auto merge). The guitest
  baseline `rows=430` is unchanged. Version **0.24.0 → 0.25.0**.
- Next (v0.26.0 / roadmap v0.27.0): per-word metadata (the u16 word
  table → word idx → doc-ids semantic walk), `lmu.g` edge semantics.
