# ODK component checklist

Normative. Every `odk-ui` component (`odk-ui-components.md`) must pass this checklist. It must not change a decision recorded in an ADR. Items keep their numbers: a new item is added at the end of its group with the next free number, and a removed one leaves a gap, so references such as "fails 11" stay valid.

A component is done when it passes every item below that applies to it. Mark an item that does not apply as such in review rather than skipping it. A component that is built but fails an item is listed under "Known gaps" until it is fixed.

## How it is tested

The tests live in `crates/odk-ui/src/checklist/`, one module per component plus `source.rs` for the rules every component shares, and run with `cargo test`. Each test names the items it checks.

- **Worst-case tests** (`content.rs`) check item 23 in small windows with the worst realistic content (long compounds, unbroken URLs, Japanese, emoji): nothing leaves its box or the window, and no button is pushed out of reach.
- **Stability tests** (`stability.rs`) check item 32: they measure every part of a component, and its neighbours, by the names components give their parts (`part_name`), then focus, hover, press, select and disable parts one by one and fail on any change in bounds.
- **Source scans** (`source.rs`) check items 1, 2, 12, 14, 15 (no empty names), 25, 26 and 31 for every component in its `COMPONENTS` list, and fail if a public component is missing from that list; item 34 for every file in its `ON_COMPONENT_TOKENS` list (the components on component tokens and the helpers they share), including the legacy helpers a component might reach through (`Space::`, `LINE_HEIGHT`, `transition()`, `focus_layer(`, `layer(`); a name such as `t.text.` counts only where it starts and ends, so `layout.text.` and `.redo()` do not.
- **Behaviour tests** drive each component on GPUI's headless test platform, pressing real keys through the bindings `run` installs. They check items 6–10, 13–17, 20–22 and 33 where they apply. Transitions are followed frame by frame on GPUI's test clock, which a test moves with `advance_clock`, reading how open each overlay was from `overlay::probe`, and item 23 as far as rendering odd content without failing.
- **Interaction tests** (`interaction.rs`, with the rest of `motion.rs` and `forms.rs`) check items 10, 38 and 39 over several controls at once: a press released elsewhere, a disabled control, a held key, a transition interrupted three ways, and a density, corner or motion change in the middle of use. They are the place to add a control to when it activates.
- **Performance tests** (`performance.rs`) check items 35 and 36: every component has a sample, a window of each sample, and of all of them together, is not drawn again while idle, and drawing one asks for no further frame. Redraws and frame requests are counted, not timed, so those tests are exact; every request goes through `interaction::request_frame`, which counts them, and a source scan in `source.rs` fails on any other (GPUI's test platform never runs the next frame's callbacks, so a component that asks for a frame at every draw is invisible to a count of redraws). `spinner.rs` holds the one component that moves on its own to a redraw per glyph. Each sample's window, a page of one of each, and all of them together are held to the item 36 thresholds on the least of several draws. How long a frame takes is the benchmarks' to say (`cargo bench -p odk-ui`), and `tools/idle-cpu COMMAND` measures a running program on a real renderer.
- **Overlays** (`overlays.rs`, `popover.rs`, `menu.rs`, `dialog.rs`, `sheet.rs`, `toast.rs`): dismissal that goes no further, layer order, one callback per request, a live theme change, the toast manager's clocks (`docs/spec/odk-overlays.md`).
- **By hand**: items 3, 4, 5, 11, 18 (beyond tone marks), 19, 23 (how it looks), 24, 28 and 29 need eyes on a story (`stories/`) or on a `tools/headless-shot` screenshot.

A known gap is a test marked `#[ignore = "known gap: <item>"]` that calls `assert_known_gap`, which fails unless the table below has a matching row. `cargo test -p odk-ui -- --ignored` runs them; each must fail until its gap is fixed.

## Theme

1. Every colour comes from a token. The only literal colour allowed is fully transparent.
2. Every size, width and duration comes from a token or from a named constant in the component, with a comment saying it is ODK's own (ADR 0035). No unnamed number goes into a style.
3. Tokens are read inside `render`, so a theme change shows on the next frame.
4. It looks right in a dark and a light stock theme.
37. It follows the user's text size (ADR 0043) because every size it draws comes from a token that scales with it; a width fixed on purpose (a dialog's, a toast's) is a named constant. Check it at 9 px and at 20 px text: nothing clips, overlaps or leaves its box. Item 2's source scan finds a size that does not come from a token.
34. Once it is on component tokens (`docs/plan.md`, "Design language migration", step 7 on), it draws only them: every colour, length, corner radius and duration comes from `odk::ui::components()` and `odk::ui::foundation()`, looked up once per render. It never reads the legacy `tokens()` or the Palette, names a palette colour, computes a colour or a contrast, or has a corner radius of its own (a round part is a `Shape`), and it takes state precedence from `States` (`docs/spec/odk-component-tokens.md`, "Rules for components"). Its hover, press and focus go through `interaction.rs`. Does not apply to a component that has not moved yet.
40. No colour is worked out where it is drawn (Phase 12). In no file of `odk-ui` outside the store in `theme.rs` does code name the palette, do colour maths (OKLCH, contrast, mixing, alpha) or read a palette colour by name; only `app.rs`, `layout.rs` and `theme.rs` read the theme snapshot, the first two for the window's canvas. Item 34 says the same of the component files; this one covers every file, so a helper cannot become a second resolver. Tested in `checklist/source.rs`.
41. No timing and no corner is written into a component (Phase 12). A `Duration` is built from a number only in a named constant, and then for a behaviour of the component's own (a typeahead's reset), never for motion, which is `foundation().motion`; no rectangle takes GPUI's named corner sizes (`rounded_md()`, ...), so every corner is the radius token, 0 or 2 px (ADR 0042). A literal radius is also caught by item 2.
42. No component keeps theme data of its own (Phase 12). Process-wide state (`static`, `thread_local!`, `LazyLock`, `OnceLock`) outside tests and debug builds is the theme store in `theme.rs` and the entries of `PROCESS_STATE` in `checklist/source.rs`, each with what it is; a new one has to be argued for there. A component that needs a theme value reads `components()` or `foundation()` in `render`.
43. Vertical and horizontal groups are a `Stack` or a `Row` (ADR 0088). In no source file of `odk-ui` outside `stack.rs` (a file's `tests` module and the checklist tests are not scanned) does code call `v_flex(` or `h_flex(`, or name `.flex_col()` or `.flex_row()`, so a group cannot be a third kind of thing. A bare `.flex()` that centres one element in a component's own drawing is not a group and is allowed. Tested in `checklist/source.rs`.

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
12. Every focus change it makes goes through `move_focus`, so the ring redraws.
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
31. It takes the time from GPUI's clock (`cx.background_executor().now()`), never from `Instant::now()`, so tests can move time on.
39. A transition interrupted by another state change, by a theme change, by reduced motion turned on, or by the component going, ends where the state is and leaves nothing running: no frame is asked for on its behalf afterwards.

## Content

23. Long labels, an empty list, one item, many items and non-Latin text truncate or wrap without breaking the layout around it (ADR 0058: prose wraps, a one-row control ends in `…` through `truncate_end`, a truncated string stays in `Describe` and in a tooltip when the control measures overflow, and a decoration changes that default).

## API

24. It follows the conventions in `odk-ui-components.md`: `new(id, ...)` and builder methods, `variant` and `size` where they make sense, `.child` for content (a builder for content that is shown later, such as a tab's panel or an overlay's body), `value`, `default_value` and `on_change` for state, and `open` and `on_close` for a controlled overlay.
25. Its doc comment states its keyboard behaviour.
26. It is exported from `odk_ui` and its prelude, and so reachable through `odk`.

## Performance

35. A window that shows it with nothing happening to it is never drawn again. GPUI draws the whole window whenever any view in it is dirty, so a component that asks for redraws it does not need makes everything beside it pay. What animates on its own asks for a redraw only while it is in view and only when what it shows changes, with a timer at that moment and not a frame request (`Spinner`; a tween or a transition asks only while it moves); with reduced motion it asks for none.
36. It has a sample in `samples::SAMPLES` (a representative instance, as many as make a window busy with it), and so a benchmark in `crates/odk-ui/benches/frame.rs`. A window busy with its sample, and a page with one of every component, are drawn from scratch within 4 ms, a quarter of a 60 Hz frame; all the components together cost at most three times what they cost apart, and at most 100 ms for about a thousand controls. Each is tested, in debug builds as in release. A component's own cost is its element count and the state it owns: keep both low, and share a handle rather than cloning an `Entity` into every listener of every frame.

## Proof

27. Unit tests cover its logic: state changes, key handling and what it reports through `Describe`.
28. Every state, including the worst-case content of item 23, has been looked at, in a story (`stories/`, run with `tools/story`) or in a `tools/headless-shot` screenshot.
29. It has been walked through with the keyboard, headlessly in its checklist tests or by the user in a story, and the result is recorded in `docs/checks.md`. Agents never open a window on the user's desktop (`CLAUDE.md`).
30. `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and `tools/check-deps` pass, and "Built so far" in `odk-ui-components.md` is updated.

## Known gaps

Where built components fail the checklist, by item number. Remove a line when it is fixed.

| Component | Fails | What is wrong |
|---|---|---|
| | | None at present. |
