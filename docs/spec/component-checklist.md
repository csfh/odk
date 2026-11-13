# ODK component checklist

Normative. Every `odk-view` component (`odk-ui-components.md`) must pass this checklist. It must not change a decision recorded in an ADR. Items keep their numbers: a new item is added at the end of its group with the next free number, and a removed one leaves a gap, so references such as "fails 11" stay valid.

A component is done when it passes every item below that applies to it. Mark an item that does not apply as such in review rather than skipping it. A component that is built but fails an item is listed under "Known gaps" until it is fixed.

## How it is tested

The tests live in `crates/odk-view/tests/` (and unit tests beside the code), on Rune's headless runtime: nothing opens a window. They run with `cargo test`; each says which items it checks.

- **Source scans** (`source.rs`) check items 1, 2, 14, 34, 40, 41 and 42 and the one-time rules (a doc comment on every public component, every public type exported, a story for every component, the process-wide state that is listed). They read the crate's files, so a literal, a colour worked out, a timing or a corner written into a component fails them.
- **Generic tests** (`generic.rs`, over the sample registry in `common/mod.rs`, one sample for every public component, which `samples.rs` keeps complete) check, for every component at once: items 3 (a theme change), 4 (dark and light palettes), 9 (one Tab stop), 11 (a ring at every stop), 15 (no empty names), 23 (worst-case content), 32 (no state change moves anything), 35 (idle), 36 (a build budget per sample and a thousand controls) and 37 (the text size).
- **Behaviour tests**, one file per component or family, drive keys and the pointer through Rune's runtime and read its element list (`Runtime::elements`): items 6 to 10, 13 to 17, 20 and 38 where they apply. `motion.rs` holds items 20 to 22 and 39 over the overlays, a `Switch` and a `Progress`; `nested.rs` item 33.
- **By hand**: items 5, 18 (beyond tone marks), 19, 24, 28's looks and 29 need eyes on the component's story (`stories/rune/`, or a `tools/headless-shot` screenshot of it).

`docs/checks.md` says, for each slice of the port, which items a test covers, which a rule of Rune gives for free or cannot give, and what was broken by hand to see a test fail.

## Theme

1. Every colour comes from a token. The only literal colour allowed is fully transparent.
2. Every size, width and duration comes from a token or from a named constant in the component, with a comment saying it is ODK's own (ADR 0035). No unnamed number goes into a style.
3. Tokens are read in the view, so a theme change shows on the next frame.
4. It looks right in a dark and a light stock theme.
37. It follows the user's text size (ADR 0043) because every size it draws comes from a token that scales with it; a width fixed on purpose (a dialog's, a toast's) is a named constant. Check it at 9 px and at 20 px text: nothing clips, overlaps or leaves its box. Item 2's source scan finds a size that does not come from a token.
34. Once it is on component tokens (`docs/plan.md`, "Design language migration", step 7 on), it draws only them: every colour, length, corner radius and duration comes from `odk::ui::components()` and `odk::ui::foundation()`, looked up once per view (`style::paint`). It never reads the Palette, names a palette colour, computes a colour or a contrast, or has a corner radius of its own, and it takes state precedence from `States` (`docs/spec/odk-component-tokens.md`, "Rules for components"). Its ring is `style::focus_ring`. Does not apply to a component that has not moved yet.
40. No colour is worked out where it is drawn (Phase 12). In no file of `odk-view` outside the host's own (`HOST` in `tests/source.rs`) does code name the palette, do colour maths (OKLCH, contrast, mixing, alpha) or read the theme snapshot. Item 34 says the same of the component files; this one covers every file, so a helper cannot become a second resolver. Tested in `tests/source.rs`.
41. No timing and no corner is written into a component (Phase 12). A `Duration` is built from a number only in a named constant, and then for a behaviour of the component's own (a typeahead's reset), never for motion, which is `foundation().motion`; no corner takes a number but zero (`.clip_rounded(`, `.rounded(`, `.radius(`), so every corner is the radius token or 0 (ADR 0042). A literal radius is also caught by item 2.
42. No component keeps theme data of its own (Phase 12). Process-wide state (`static`, `thread_local!`, `LazyLock`, `OnceLock`) outside tests and debug builds is the entries of `PROCESS_STATE` in `tests/source.rs`, each with what it is; a new one has to be argued for there. A component that needs a theme value reads `components()` or `foundation()` in the view.
43. Not applicable on Rune: a group is `col()` or `row()`, Rune's own primitives, or a `Layout` builder where the gap is a `Space` (ADR 0088 was for GPUI's `v_flex`).

## States

5. Each interactive part draws its normal, hover, pressed and focused states, and selected and disabled where they apply, from the `control` tokens, or from its component tokens once on them (item 34).
6. A part that can be disabled ignores the pointer and the keyboard, leaves the Tab order, draws its text in `text.disabled` and reports itself disabled. A composite control's arrow keys skip a disabled item.
7. A part that starts slow work can show that it is busy (as `Button::loading` does) and reports itself busy.
32. No state change moves anything: hovering, pressing, focusing, selecting or disabling a part keeps every part, and everything around the component, exactly where it was. A focus ring or a selection mark is drawn without taking space.

## Keyboard and focus

8. Everything it does can be done from the keyboard alone.
9. Tab stops on it once. A composite control has one tab stop and moves between its items with the arrow keys, Home and End.
10. Enter and Space activate, and Escape closes whatever it opened.
11. The focused part shows a ring in `accent.base` (or `text.primary` on an accent fill) without moving anything around it. Once on component tokens, the ring is its style's `focus`: `focus_width` wide, `focus_offset` outside the part, with the part's corner radius.
12. Not applicable on Rune: Rune owns focus and draws the ring; a component only says what is focusable (and `tests/generic.rs` holds that a ring is drawn at every stop).
13. An overlay that takes focus moves it in on open and gives it back on close. A modal one keeps Tab inside itself.
33. An overlay opened from inside a modal one (a `Dialog` or `Sheet`) is drawn above it and works there: it takes clicks and keys, is not cut off by the modal's panel or its scrolling body, and Escape closes it before it closes the modal (ADR 0039).
38. One completed activation is one callback, and only a completed one: a press released off the control, a key held down (repeating), a disabled control and a busy one report nothing, and the press that closes an overlay goes no further (ADR 0045). Checked over the controls together in `checklist/interaction.rs`.

## Accessibility

14. It implements `Describe` with the right `Role`.
15. Its name is not empty. A part labelled only by a glyph takes an `accessible_name`.
16. It reports its state: selected, disabled, busy.
17. A composite control reports each item (`item_accessibles`).
18. Colour is never the only signal. A tone also has a mark, and a selection also has a fill or an underline. A link is underlined at rest (a button link, a breadcrumb link, a menu's link); the current page is not a link and is not underlined. A number that is not a number says "Enter a number" as well as showing the field's mark.

A pointer target is at least 24 px on each side (WCAG 2.5.8), or the space around it makes up the difference. `geometry.target_min` is 24 px and does not scale with the text size. Field actions (clear, reveal, steppers) use it as a floor, so at the smallest text size a compact one can extend past its field rather than shrink below 24 px. A checkbox, radio or switch row is at least 24 px wide and tall. Vertical radios add gap when the text line is shorter than that, which at 9 px text is 9 px between the rows.

## Motion

19. It animates only with the foundation's `motion` durations, with the foundation's easing.
20. When `motion.reduced` is set, it shows its end state at once and nothing loops.
21. Anything that disappears runs an exit, and keeps whatever it needs to draw that exit (a sheet keeps its side).
22. A transition reversed partway starts from where it is, without jumping.
31. It takes the time from Rune's clock (what the poll and a handler are given), never from `Instant::now()`, so tests can move time on.
39. A transition interrupted by another state change, by a theme change, by reduced motion turned on, or by the component going, ends where the state is and leaves nothing running: no frame is asked for on its behalf afterwards.

## Content

23. Long labels, an empty list, one item, many items and non-Latin text truncate or wrap without breaking the layout around it (ADR 0058: prose wraps, a one-row control ends in `…` through `truncate_end`, a truncated string stays in `Describe` and in a tooltip when the control measures overflow, and a decoration changes that default).

## API

24. It follows the conventions in `odk-ui-components.md`: `new(id, ...)` and builder methods, `variant` and `size` where they make sense, `.child` for content (a builder for content that is shown later, such as a tab's panel or an overlay's body), `value`, `default_value` and `on_change` for state, and `open` and `on_close` for a controlled overlay.
25. Its doc comment states its keyboard behaviour.
26. It is exported from `odk_ui` and its prelude, and so reachable through `odk`.

## Performance

35. A window that shows it with nothing happening to it is never drawn again. Rune draws a frame only when something is dirty or woken, so a component that asks for wakes it does not need makes the window pay. What animates on its own asks for a redraw only while it is in view and only when what it shows changes, with one `Tx::wake_after` at that moment (`Spinner`'s poll; a transition asks only while it moves); with reduced motion it asks for none.
36. It has a sample in `tests/common/mod.rs`'s `SAMPLES` (a representative instance), which `tests/samples.rs` keeps complete. A window busy with its sample is built and laid out from scratch within 4 ms, a quarter of a 60 Hz frame, and about a thousand controls within 100 ms (`DRAW_BUDGET_MS` loosens the first on a busy machine); `cargo bench -p odk-view --bench frame` prints the figures for every sample. A component's own cost is its element count and the state it owns: keep both low.

## Proof

27. Unit tests cover its logic: state changes, key handling and what it reports through `Describe`.
28. It has a story of its own, `stories/<name>.rs` (the component's name in snake case, built by the `stories` crate as a binary of that name and run with `tools/story <name>`), that shows every state, including the worst-case content of item 23.
29. It has been walked through with the keyboard, headlessly in its checklist tests or by the user in its story, and the result is recorded in `docs/checks.md`. Agents never open a window on the user's desktop (`CLAUDE.md`).
30. `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and `tools/check-deps` pass, and "Built so far" in `odk-ui-components.md` is updated.

## Known gaps

Where built components fail the checklist, by item number. Remove a line when it is fixed.

| Component | Fails | What is wrong |
|---|---|---|

None since 2026-11-13, when item 23 was closed for `Checkbox`, `Switch` and `ToggleGroup` (`tests/item_23.rs`).
