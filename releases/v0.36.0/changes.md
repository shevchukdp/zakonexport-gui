# v0.36.0 - Changes / Зміни (2026-10-29)

## UK / Українська

- **GUI: Trident search panel (roadmap v0.37.0).** `gui.rs` — окреме
  модальне вікно «Тріант (tsearch)» (як preview/annex/update), відкривається
  кнопкою «Тріант…» на головному вікні:
  - **`btn_trident`** (рядок 3) відкриває **`trident_window`** — окремий
    `nwg::Window` (не видно за замовчуванням) з полем запиту, полем бази,
    кнопкою «Шукати» та багаторядковим результатом **`trident_result`**.
  - **`on_trident_open` / `on_trident_run` / `trident_chain(query, base)`** —
    показують tsearch-ланцюк: `запит → LmCode hex → Latin-кандидат → .vt
    record @off → native u16 doc-ids`. Використовують **crate API**
    (`crate::trident::TridentIndex` + новий публічний `lmcode_hex()`), **без
    дублювання рушія в GUI**.
  - Панель — окремий `nwg::Window`, тому її список результатів НЕ торкає
    `row_hits` → **guitest `rows=430` без змін** (asserted). Guitest тепер
    також перевіряє панель на реальній базі (`trident=...` у
    `gui_ready.txt`): на GK-базі (без Trident-індексу) → `trident=noindex`,
    на базі з індексом → `trident=ok(<кількість кандидатів>)`.
  - Новий публічний **`TridentIndex::lmcode_hex(term)`** — LmCode hex запиту
    (крок `запит → LmCode` ланцюжка).
- **190 юніт-тестів** (новий 1: `lmcode_hex_of_known_terms`). Guitest
  baseline `rows=430` без змін (+ новий маркер `trident=...`). Версія
  **0.35.0 → 0.36.0**.
- Далі (v0.37.0 / roadmap v0.38.0): `export --trident-docs`.

## EN / English

- **GUI: the Trident search panel (roadmap v0.37.0).** `gui.rs` — a
  separate modal window "Trident (tsearch)" (like preview/annex/update),
  opened by a "Trident…" button on the main window:
  - **`btn_trident`** (row 3) opens **`trident_window`** — a separate
    `nwg::Window` (hidden by default) with a query field, a base field, a
    "Search" button, and a multi-line **`trident_result`** area.
  - **`on_trident_open` / `on_trident_run` / `trident_chain(query, base)`** —
    surface the tsearch chain: `query -> LmCode hex -> Latin candidate -> .vt
    record @off -> native u16 doc ids`. They reuse the **crate API**
    (`crate::trident::TridentIndex` + the new public `lmcode_hex()`), with
    no engine duplication in the GUI.
  - The panel is a separate `nwg::Window`, so its result list never touches
    `row_hits` -> the guitest `rows=430` is unchanged (asserted). The guitest
    now also exercises the panel on the real base (`trident=...` in
    `gui_ready.txt`): on the GK base (no Trident index) -> `trident=noindex`,
    on an indexed base -> `trident=ok(<candidate count>)`.
  - A new public **`TridentIndex::lmcode_hex(term)`** — the LmCode hex of a
    query (the `query -> LmCode` step of the chain).
- **190 unit tests** (1 new: `lmcode_hex_of_known_terms`). The guitest
  baseline `rows=430` is unchanged (+ a new `trident=...` marker). Version
  **0.35.0 → 0.36.0**.
- Next (v0.37.0 / roadmap v0.38.0): `export --trident-docs`.
