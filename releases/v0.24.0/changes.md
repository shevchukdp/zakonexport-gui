# v0.24.0 - Changes / Зміни (2026-10-28)

## UK / Українська

- **tsearch v2: точний join як primary, позначені fallback-и.**
  Редизайн `TridentResult` / `search()`:
  - точний stem-join (`.vt` record → u16 doc-ids) — **primary**
    evidence (поле `joined`);
  - пропорційний `posting_slice` тепер **лише fallback** —
    заповнюється `docs`, коли точний join не розв'язав термін, і
    маркер `docs_fallback`; в CLI позначається «оцінка, пропорційний
    posting-slice» — жодна евристика не видається за точну;
  - новий `TridentIndex::term_join_docs(term)` — distinct u16 doc-ids
    (дедуп, без нулів, відсортовано) для одного терміна.
- **Новий прапорець `tsearch --oracle`** (кросчек проти ftsearch):
  `ftsearch::oracle_multi` рахує distinct-файли, що містять кожен
  термін (один `.mb`-прохід, нижній + title-case). Для кожного
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

## EN / English

- **tsearch v2: exact join as primary, labelled fallbacks.**
  `TridentResult` / `search()` reworked:
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
  `tsearch N docs · oracle M docs · agreement min/max` (a *count*
  agreement — the u16 doc-ids and the `.mb` files are different
  identifier spaces). Overall agreement ≥ 0.9 → the line says
  «оракельно підтверджено / oracle-confirmed», else «експеримент
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
