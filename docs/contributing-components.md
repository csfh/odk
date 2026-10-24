# Adding to the design system

The contract for a new `odk-ui` component, token or gallery page. The normative list is `docs/spec/component-checklist.md` (numbered items, tested); this page is the order to work in and where each rule is checked. Read `CLAUDE.md` first: windows are never opened on the user's desktop, and ADRs win over specs.

## The contract

A new component:

| # | Rule | Checklist | Where it is checked |
|---|---|---|---|
| 1 | Takes sizes, radius, borders and type from foundation tokens | 2, 34, 37 | `checklist/source.rs` (`no_unnamed_numbers_in_styles`, `components_on_component_tokens_draw_only_them`) |
| 2 | Takes colour only from component or semantic tokens | 1, 34, 40 | `source.rs` (`no_colour_literals`, `colour_is_resolved_in_the_theme_layers_and_nowhere_else`) |
| 3 | Never reads the palette or computes a colour, contrast or OKLCH while drawing | 34, 40 | same |
| 4 | Follows the density that applies to it (`Density`, `Size` as a step) | 34 | `odk-theme/tests/components.rs` (`controls_are_28_34_and_40_high_and_follow_their_density`, `theme_by_density_by_radius_matrix`), the component's checklist module |
| 5 | Has the global corner radius: the radius token, never a number or GPUI's named corners | 2, 34, 41 | `source.rs` (`timing_and_corners_come_from_tokens`), `components.rs` (`corners_are_exactly_0_or_exactly_2...`) |
| 6 | Uses a type role | 34, 37 | component tokens carry a `TypeRole`; `text_and_headings_take_their_type_roles` |
| 7 | Takes durations and easing from `foundation().motion`; a `Duration` written as a number is only a named constant for a behaviour that is not motion | 19, 41 | `source.rs`, `checklist/motion.rs` |
| 8 | Respects reduced motion: the end state at once, nothing loops | 20, 39 | `checklist/motion.rs`, `interaction.rs` |
| 9 | Works from the keyboard as its role requires, and documents it in its doc comment | 8, 9, 10, 25 | `checklist/<component>.rs`, `source.rs` (`keyboard_behaviour_is_documented`) |
| 10 | Shows a focus ring when interactive: `interaction.rs`'s `focus_ring`, outside the control | 11, 12, 32 | `checklist/interaction.rs`, `source.rs` (`focus_moves_through_move_focus`) |
| 11 | Means the same without colour: a mark, a shape, a label, an underline | 18 | `components.rs` (`status_is_never_colour_alone`), the component's module |
| 12 | Reacts to a theme change on the next frame and keeps what it holds | 3, 39 | `checklist/forms.rs`, `overlays.rs`, `toast.rs`, `navigation.rs` (`a_theme_change_keeps_...`) |
| 13 | Does no colour maths, file or theme work in `render`; no theme cache of its own | 3, 40, 42 | `source.rs` (`process_wide_state_is_the_theme_store_and_what_is_listed`) |
| 14 | Has tests: behaviour in `checklist/<component>.rs`, its entry in `COMPONENTS` in `source.rs`, its logic as unit tests | 27 | `source.rs` (`every_component_is_listed`) |
| 15 | Is in the gallery, in every state, with worst-case content | 23, 28 | `examples/gallery` tests (`cargo test -p gallery`) |
| 16 | Is documented: a row in `docs/spec/odk-ui-components.md`, a section in `docs/gallery.md`, a line in `docs/migration.md` if it replaces something | 30 | review |
| 17 | Follows the API conventions: `new(id, ...)`, builders by value, `on_*` callbacks, `impl Describe`, in the prelude | 14, 24, 26 | `source.rs` (`every_component_implements_describe`, `every_component_is_in_the_prelude`) |

Apply them according to what the component is. A `Separator` has no keyboard behaviour and a `Text` no focus ring; say which items do not apply in the change rather than skipping them. Performance is part of the contract: a component never asks for a redraw while idle or out of view (35), and has a sample in `samples.rs` for the benchmarks (36).

## Order of work

1. **Design first when it is new.** A new component or a new rule needs a row in `odk-ui-components.md` and, if it decides something, an ADR (`domain-modeling`). Use `GLOSSARY.md`'s terms.
2. **Tokens.** If it needs a style, add it to the component layer (`crates/odk-theme/src/component/`) as a typed lookup: colours are `ColourRef`s into the semantic layer, geometry derives from the foundation. Add it to the catalogue so `validate` checks every theme, and write its test in `odk-theme/tests/components.rs`. Do not add a colour to the semantic layer for one component; ask whether an existing treatment fits.
3. **Component.** Read `components()` and `foundation()` once per render; draw states with `interaction.rs` (`pointer_states`, `focus_ring`, `progress`); move focus with `move_focus`; take state precedence from `States`.
4. **Checklist tests.** Add the component to `COMPONENTS` and `ON_COMPONENT_TOKENS` in `checklist/source.rs`, give it a module under `checklist/`, and break each rule it tests once by hand before trusting it. A gap is an `#[ignore = "known gap: N"]` test that calls `assert_known_gap`, and a row under "Known gaps".
5. **Gallery.** Put it on the page of its family with its states; pages stay small enough to draw well inside a frame. Run `tools/idle-cpu --all` after a gallery change.
6. **Measure.** `cargo bench -p odk-ui -- <Name>` before and after a change to how it draws.
7. **Look at it.** Headlessly: `tools/headless-shot`, or `tools/visual-scenarios` for the reference set (`docs/gallery.md`, "Reference screenshots"). Never in the user's session.
8. **Verify.** `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `tools/check-deps`; update "Built so far" in `odk-ui-components.md`.

## Adding a token

A foundation value (a size, a duration) goes in `foundation.rs` as a named constant with its test in the same file and a line in `docs/spec/odk-foundation-tokens.md`; a number written there twice is a bug. A semantic colour needs the resolver's contrast guarantee: add it to `semantic.rs` with the pairs it must pass. A component style goes where step 2 above says. Nothing is added to the source layer: `colors.toml` is Omarchy's.

## Exceptions that stand

Each is allowed on purpose and listed where the check is, so a new one has to be argued for:

- `group_hover` and `group_active` in `switch.rs`, `radio.rs` and `menu_list.rs`: a child that reacts to its parent's state. The colours are component tokens; `interaction.rs` has no group variant.
- `PROCESS_STATE` in `checklist/source.rs`, by file and name: the theme store's three statics, the toast id counter and the overlay nesting depth (the debug-only focus-name registry is not counted).
- `MAY_READ_THE_SNAPSHOT`: `app.rs` and `layout.rs` draw the window's canvas and text.
- Named constants for widths fixed on purpose (a dialog 480 px, a toast 336 px, a menu at least 192 px) and for behaviour timings (`select.rs`'s typeahead reset, `spinner.rs`'s scheduler floor).
- Round parts (switch thumb and track, radio dot, status dot, spinner) are `Shape`s, not a larger radius.
- The gallery draws swatches of tokens with `colour(..)`; its colour, resolver, compare and diagnostics pages read the palette because showing it is their job.
