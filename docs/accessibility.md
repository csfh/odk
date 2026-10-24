# Accessibility in ODK

For people building applications with ODK. The rules ODK keeps for its own components are in `docs/spec/odk-interaction.md` (the contract) and `docs/spec/component-checklist.md` (the tests). This page says what that means for an application: what you get without asking, what you must do, and what ODK cannot do yet.

## What is and is not possible today

ODK sits on GPUI 0.2.2, which has **no accessibility tree**: no AccessKit, no platform bridge. A screen reader on Omarchy cannot read an ODK window. Everything below that says "reports" means the component states it through `odk_a11y::Describe` (a role, a name, selected, disabled, busy, invalid, required, a description, a value, and its items), which ODK's own tests read, and which is ready for the day GPUI has a tree to put it in. Nothing here has been checked with a screen reader or any platform accessibility API.

What works today without a tree is the keyboard, focus, contrast, motion and text that does not rely on colour. Those are tested.

## Keyboard

The whole of every ODK component works from the keyboard (checklist item 8). Tab and Shift+Tab move between controls; a group (radio buttons, tabs, a segmented control, a list) is one stop and the arrow keys, Home and End move inside it. Enter and Space press a button; Space toggles a checkbox; Escape closes what is open. The full table, by component, is in `docs/spec/odk-interaction.md`, "Keyboard". Your own handlers get one call per completed activation: a press released off the control, a held key, a disabled control or a busy one never call you.

Your part: do not build a control out of a bare `div` with `on_click`. It has no tab stop, no key, no ring and no name. Use `Button`, or `Link`.

## Focus

- The focused control has a ring 2 px wide, a gap clear of the control, in the theme's focus colour. It is drawn outside the control and moves nothing, so it never shifts layout, and it shows on any focus, pointer or keyboard. Its contrast against every surface a control may sit on is checked at 3:1 for every theme.
- A modal `Dialog` or `Sheet` keeps Tab inside, blocks the window behind it and gives focus back when it closes. A `Popover` and a side-panel `Sheet` are not modal: Tab can leave them, the window behind stays live, and Escape closes them. A `Tooltip`, a `HoverCard` and a `Toast` never take focus.
- A change of theme, density, corners or reduced motion loses nothing: focus, typed text, choices, open overlays.

Your part: do not move focus yourself with GPUI's `window.focus` in a way that skips the ring. Components use `move_focus`, which also redraws. Give a dialog an action the user can reach, and a way out other than the pointer.

`odk::ui::focused_component(window, cx)` names the control that has focus, in debug builds, for diagnostics; the gallery's Interaction page shows it.

## Names, labels and descriptions

- A component with a visible label is named by it.
- An icon-only control needs a name: `Button::icon(id, glyph, name)` or `.accessible_name(...)`. A glyph's own name ("×") is not a name. ODK's own checklist scans that no component reports an empty one (item 15). Give it a tooltip as well, so the name is visible.
- A `FormField` ties a label, a description and a message to its control, and marks it required or invalid by text and mark as well as by colour.
- A `Dialog` and `Sheet` take a title, which is their name; give them a description when the title does not say what is at stake.

## Colour and contrast

- Text is 4.5:1 on every surface it can be drawn on, and a mark that shows a state (a switch's thumb, a tab's indicator, a radio's dot, a progress bar, a focus ring) and an essential edge are 3:1. They are checked each time a theme is resolved; a theme that cannot reach them is refused and the last one stays (ADR 0041), so you cannot ship a combination that fails, and you do not check one.
- Disabled controls and decorative borders are exempt from those minimums, and are still told from enabled ones by a weaker colour and, for a button, by not responding.
- Colour is never the only signal. A tone has a mark (a glyph beside its text), a selection a fill or an underline, an invalid field a mark and a message. A monochrome theme stays monochrome and every one of these still reads.

Your part: use the components' variants and tones rather than your own colours. A colour of your own bypasses the checks. Words carry meaning: "Saved", not a green dot.

## Motion

Every transition runs on the foundation's durations and curve and never overshoots. With reduced motion on, every transition is instant, every state change still happens, a spinner stands still but stays visible, a progress bar still shows its value, and nothing waits for an animation. Reduced motion is on if either of these asks:

- the application: `odk::ui::set_reduced_motion(cx, true)`, for an application that offers the choice;
- the user, through the system: Hyprland's `animations:enabled` set to false, which is Omarchy's own place for it (`hl.config({ animations = { enabled = false } })` in `~/.config/hypr/looknfeel.lua`, ADR 0048). ODK applications follow it live when Hyprland reloads its configuration, which it does when that file is saved, and across a restart of Hyprland: the last answer stands while it is away and the new one is taken when it is back. A sandboxed application is told what `oma` saw when it launched or last synced; it cannot reach Hyprland, and must not (`hyprctl keyword` changes the setting without a reload, so that is only seen at the next one).

Neither turns the other off. Hover and press colours are not transitions; they change at once.

Your part: do not animate with GPUI's `with_animation` for anything a component already does, and when you animate something of your own, take its length from `odk::ui::foundation().motion.transition(...)`, which is zero when motion is reduced.

## Forms

Labels, helper text and validation messages belong in a `FormField`, which marks required fields and shows an error with a mark and text. A disabled control is skipped by Tab and ignores everything; a read-only field takes focus and its text can be selected and copied, which a disabled one's cannot. A busy `Button` keeps its place, size and focus and ignores presses until you stop it.

## Notifications

A `Toast` never takes focus and never interrupts typing. Its clock stops while the pointer or keyboard focus is on the stack, a persistent one stays until dismissed, and Escape dismisses the one that has focus. It is not announced: without an accessibility tree there is nothing to announce through, so a message that matters, such as an error that stops work, should also be where the user is looking (an `Alert` in the form, a message on the field), and should be persistent.

## Who does what

| ODK does | You do |
|---|---|
| Keyboard operation, roving focus, the ring, focus trap and restore in modals | Use the components; give every control a way to be reached |
| Contrast of every component on every theme | Use roles and tones, not colours of your own |
| One callback per activation; nothing from disabled or busy controls | Do not do work in a handler that you also do on focus or hover |
| Reduced motion, live | Offer `set_reduced_motion` if your application has settings; take your own animation's length from the motion tokens |
| `Describe` on every component | Names for icon-only controls; a title on every dialog |
| Marks and text beside colour | Words that say what happened |

## Not covered

No screen reader, no platform accessibility API, no toast announcements, no high-contrast mode beyond what a theme's colours give, no `prefers-reduced-motion` from the desktop beyond Hyprland's window animations setting (ADR 0048; GNOME's `enable-animations` and the freedesktop portal were probed on 2026-10-23 and are not read), no zoom beyond the user's text size (ADR 0043), and no right-to-left layout. Manual checks made so far are in `docs/checks.md`.
