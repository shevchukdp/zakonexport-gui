# v0.34.0 - Changes / Зміни (2026-10-28)

## UK / Українська

- **Точне латинське мапування (roadmap v0.35.0, part 1).** `trident.rs`:
  - **`latin_exact(bytes) -> Option<String>`** — детерміноване, uncapped
    мапування: кожна літера → її *preferred* варіант (перший рядок
    `LATIN_VARIANTS`), цифри pass-through, немовне byte → `None` для
    всього слова. На відміну від `latin_candidates` (capped BFS по ВСІХ
    варіантах — задокументований fallback для неоднозначних літер), це
    **total function**: рівно один `String` на слово, без вибуху
    кандидатів.
  - **`LATIN_VARIANTS` — demoted (задокументовано):** рядок[0] = точне
    мапування (використовується `latin_exact`); решта альтернат =
    documented fallback, які `latin_candidates` досі розглядає для
    неоднозначних літер.
  - **`exact_postings_in`** тепер *prefer* точний шлях: точний кандидат
    (preferred-variant) перший, потім capped-BFS альтернатати, які він не
    покrywає.
  - **`exact_latin_hit_rate()`** — part-1 structural rate: over реальних
    `lmu.voc` stems (**390 mappable**) точний кандидат потрапляє в `.vt`
    stream на **101 (≈26 %)**. 100 % rate потребує part-2
    `DAT_1002f1f0` graph normalization (context-dependent letter
    overrides) — deferred. EN: the exact-candidate hit count over the real
    dictionary stems (the part-1 structural figure); the full 100 % awaits
    the part-2 graph normalization.
- **CLI `trident`**: новий рядок `latin-exact (v0.34.0, roadmap v0.35.0):
  390 mappable stems · exact .vt hit 101 (part-1 rate; 100% awaits
  DAT_1002f1f0 graph norm)` + usage + roadmap-сумарі.
- **188 юніт-тестів** (нові 2: `latin_exact` preferred-variant +
  ordering-invariant, real-corpus hit-rate). Guitest baseline `rows=430`
  без змін. Версія **0.33.0 → 0.34.0**.
- Далі (v0.35.0 / roadmap v0.36.0): legacy-engine parity (`lmsup.dll` vs
  `lmsup32.dll`).

## EN / English

- **The exact Latin mapping (roadmap v0.35.0, part 1).** `trident.rs`:
  - **`latin_exact(bytes) -> Option<String>`** — the deterministic,
    uncapped mapping: each letter → its *preferred* variant (the first row
    of `LATIN_VARIANTS`), digits pass through, an unmappable byte →
    `None` for the whole word. Unlike `latin_candidates` (the capped BFS
    over *all* variants — the documented fallback for the ambiguous
    letters), this is a **total function**: exactly one `String` per word,
    with no multiplicative candidate explosion.
  - **`LATIN_VARIANTS` — demoted (documented):** row[0] = the exact
    mapping (used by `latin_exact`); the remaining alternates = the
    documented fallback that `latin_candidates` still explores for the
    ambiguous letters.
  - **`exact_postings_in`** now *prefers* the exact path: the exact
    candidate (preferred-variant) comes first, then the capped-BFS
    alternates it didn't cover.
  - **`exact_latin_hit_rate()`** — the part-1 structural rate: over the
    real `lmu.voc` stems (**390 mappable**) the exact candidate lands in
    the `.vt` stream on **101 (≈26 %)**. The 100 % rate needs the part-2
    `DAT_1002f1f0` graph normalization (the context-dependent letter
    overrides) — deferred.
- **CLI `trident`**: a new `latin-exact (v0.34.0, roadmap v0.35.0): 390
  mappable stems · exact .vt hit 101 (part-1 rate; 100% awaits DAT_1002f1f0
  graph norm)` line + the usage + the roadmap summary.
- **188 unit tests** (2 new: the `latin_exact` preferred-variant +
  ordering-invariant, the real-corpus hit-rate). The guitest baseline
  `rows=430` is unchanged. Version **0.33.0 → 0.34.0**.
- Next (v0.35.0 / roadmap v0.36.0): the legacy-engine parity
  (`lmsup.dll` vs `lmsup32.dll`).
