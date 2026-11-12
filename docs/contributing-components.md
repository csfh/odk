# Adding to the design system

The contract for a new `odk-view` component, token or story. The normative list is `docs/spec/component-checklist.md` (numbered items, tested); this page is the order to work in and where each rule is checked. Read `CLAUDE.md` first: windows are never opened on the user's desktop, and ADRs win over specs.

## The contract

A new component:

| # | Rule | Checklist | Where it is checked |
|---|---|---|---|
| 1 | Takes sizes, radius, borders and type from foundation tokens | 2, 34, 37 | `tests/source.rs` (`no_unnamed_numbers_in_styles`, `components_draw_only_component_tokens`), `tests/generic.rs` (the text size) |
| 2 | Takes colour only from component or semantic tokens | 1, 34, 40 | `tests/source.rs` (`no_colour_literals`, `colour_is_resolved_in_the_theme_layers_and_nowhere_else`) |
| 3 | Never reads the palette or computes a colour, contrast or OKLCH while drawing | 34, 40 | same |
| 4 | Follows the density that applies to it (`Density`) | 34 | `odk-theme/tests/components.rs`, the component's own test file |
| 5 | Has the global corner radius: the radius token, never a number | 2, 34, 41 | `tests/source.rs` (`timing_and_corners_come_from_tokens`) |
| 6 | Uses a type role | 34, 37 | component tokens carry a `TypeRole`; `tests/generic.rs` |
| 7 | Takes durations and easing from `foundation().motion`; a `Duration` written as a number is only a named constant for a behaviour that is not motion | 19, 41 | `tests/source.rs` |
| 8 | Respects reduced motion: the end state at once, nothing loops. An overlay goes through `Paint::present` | 20, 21, 22, 39 | `tests/motion.rs` |
| 9 | Works from the keyboard as its role requires, and documents it in its doc comment | 8, 9, 10, 25 | the component's test file, `tests/generic.rs` (one Tab stop), `tests/source.rs` (a doc comment) |
| 10 | Shows a focus ring when interactive: `style::focus_ring`, outside the control | 11, 32 | `tests/generic.rs` (a ring at every stop, nothing moves), `style.rs` (the ring's geometry) |
| 11 | Means the same without colour: a mark, a shape, a label, an underline | 18 | the component's test file |
| 12 | Reacts to a theme change on the next frame and keeps what it holds | 3, 39 | `tests/generic.rs` (a theme change), `tests/motion.rs` |
| 13 | Does no colour maths, file or theme work in the view; no theme cache of its own | 3, 40, 42 | `tests/source.rs` (`process_wide_state_is_what_is_listed`) |
| 14 | Has tests: a sample in `tests/common/mod.rs`, behaviour in a test file of its own, its logic as unit tests | 27, 35, 36 | `tests/samples.rs` (every component has a sample) |
| 15 | Has a story of its own that shows every state, with worst-case content | 23, 28 | `stories/rune/<name>.rs` (a source scan checks it exists, item 28); by hand, `tools/headless-shot` |
| 16 | Is documented: a row in `docs/spec/odk-ui-components.md`, a line in `docs/migration.md` if it replaces something | 30 | review |
| 17 | Follows the API conventions: `new(id, ...)`, builders by value, `on_*` callbacks taking `&mut T`, `impl Describe`, exported from `lib.rs` | 14, 24, 26 | `tests/source.rs` (`every_component_implements_describe_and_draws_it`, every public type exported) |

Apply them according to what the component is. A `Separator` has no keyboard behaviour and a `Text` no focus ring; say which items do not apply in the change rather than skipping them. Performance is part of the contract: a component never asks for a redraw while idle or out of view (35), and has a sample in `tests/common/mod.rs` for the benchmarks (36).

## Order of work

1. **Design first when it is new.** A new component or a new rule needs a row in `odk-ui-components.md` and, if it decides something, an ADR (`domain-modeling`). Use `GLOSSARY.md`'s terms.
2. **Tokens.** If it needs a style, add it to the component layer (`crates/odk-theme/src/component/`) as a typed lookup: colours are `ColourRef`s into the semantic layer, geometry derives from the foundation. Add it to the catalogue so `validate` checks every theme, and write its test in `odk-theme/tests/components.rs`. Do not add a colour to the semantic layer for one component; ask whether an existing treatment fits.
3. **Component.** Read `hosted.components()` and `hosted.foundation()` once per view (`style::paint`); draw states from the tokens and the ring with `style::focus_ring`; Rune owns focus, so say only what is focusable; take state precedence from `States`; an overlay goes through `Paint::present`.
4. **Tests.** Add a sample to `tests/common/mod.rs` (the generic checks then run on it), give it a test file of its own, and break each rule it tests once by hand before trusting it. A gap is a row under "Known gaps".
5. **Story.** Every component has one, `stories/rune/<name>.rs` with the name in snake case (`DataTable` is `data_table`), as `stories/rune/button.rs` does, and a `[[bin]]` named `rune-<name>` for it in `stories/Cargo.toml`; keep it small enough to build well inside a frame. `tools/story rune-<name>` runs it in watch mode (the owner's tool). It is item 28: a new component without one fails `tests/source.rs`.
6. **Measure.** `tests/generic.rs` holds a build budget per sample; `tools/idle-cpu` measures a running story's idle CPU.
7. **Look at it.** Headlessly: `tools/headless-shot` on the story. Never in the user's session.
8. **Verify.** `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`, `cargo machete`, `DRAW_BUDGET_MS=30 cargo test --workspace --all-features --locked`, `tools/check-deps` after a dependency changes; update "Built so far" in `odk-ui-components.md`.

## Adding a token

A foundation value (a size, a duration) goes in `foundation.rs` as a named constant with its test in the same file and a line in `docs/spec/odk-foundation-tokens.md`; a number written there twice is a bug. A semantic colour needs the resolver's contrast guarantee: add it to `semantic.rs` with the pairs it must pass. A component style goes where step 2 above says. Nothing is added to the source layer: `colors.toml` is Omarchy's.

## Exceptions that stand

Each is allowed on purpose and listed where the check is, so a new one has to be argued for:

- `group_hover` and `group_active` in `switch.rs`, `radio.rs` and `menu_list.rs`: a child that reacts to its parent's state. The colours are component tokens; `interaction.rs` has no group variant.
- `PROCESS_STATE` in `checklist/source.rs`, by file and name: the theme store's three statics, the toast id counter and the overlay nesting depth (the debug-only focus-name registry is not counted).
- `MAY_READ_THE_SNAPSHOT`: `app.rs` and `layout.rs` draw the window's canvas and text.
- Named constants for widths fixed on purpose (a dialog 480 px, a toast 336 px, a menu at least 192 px) and for behaviour timings (`select.rs`'s typeahead reset, `spinner.rs`'s scheduler floor).
- Round parts (switch thumb and track, radio dot, status dot, spinner) are `Shape`s, not a larger radius.
- A story that shows tokens may draw swatches with `colour(..)`; stories are not `odk-view` source, so the source scans do not read them.
