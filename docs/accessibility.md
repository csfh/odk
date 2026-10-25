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
- A modal `Dialog` or `Sheet` keeps Tab inside, blocks the window behind it and gives focus back when it closes. A `Popover` and a side-panel `Sheet` are not modal: Tab can leave them, the window behind stays live, and Escape closes them. A `Tooltip` and a `HoverCard` never take focus. A `Toast` does not take focus when it appears; its stack is a tab stop, so the keyboard can pause it and dismiss it.
- A dialog or sheet body scrolls the focused control into view. A side panel or a toast that would hide the focused control completely shifts enough to leave a sliver of it visible.
- A change of theme, density, corners or reduced motion loses nothing: focus, typed text, choices, open overlays.

Your part: do not move focus yourself with GPUI's `window.focus` in a way that skips the ring. Components use `move_focus`, which also redraws. Give a dialog an action the user can reach. Leave Escape, the scrim or the close button on: a dialog with all three off is refused.

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

A `Toast` never takes focus when it appears, and never interrupts typing. Its stack is a tab stop, so Tab reaches it even when a toast has no button. Its clock stops while the pointer is over the stack or the stack has focus, a persistent one stays until dismissed, and Escape dismisses the newest toast while the stack itself has focus (or the toast that has focus, when that is a button). It is not announced: without an accessibility tree there is nothing to announce through, so a message that matters, such as an error that stops work, should also be where the user is looking (an `Alert` in the form, a message on the field), and should be persistent.

## Who does what

| ODK does | You do |
|---|---|
| Keyboard operation, roving focus, the ring, focus trap and restore in modals | Use the components; give every control a way to be reached |
| Contrast of every component on every theme | Use roles and tones, not colours of your own |
| One callback per activation; nothing from disabled or busy controls | Do not do work in a handler that you also do on focus or hover |
| Reduced motion, live | Offer `set_reduced_motion` if your application has settings; take your own animation's length from the motion tokens |
| `Describe` on every component | Names for icon-only controls; a title on every dialog |
| Marks and text beside colour | Words that say what happened |

## Text size

The user's text size is `[font] base-size` in `shell.toml`, kept to 9–20 px (ADR 0043). The default is 12 px, so the range is 75% to 167% of the default. Borders, the focus ring, radii, durations, the 24 px target minimum and a fixed overlay width do not scale. An `Autosize` overlay scales its measure and stays inside the window (ADR 0058). WCAG 1.4.4 asks for text to resize to 200% without loss of content or function. ODK's ceiling is 167%, and that clamp is unchanged: 20 px is as large as `omarchy display text size` accepts, and a larger size is not offered. There is no separate page zoom.

When a control truncates, the full string stays in `Describe` and, when the control measured an overflow, in a tooltip on hover and on keyboard focus. The tooltip adds no tab stop. Text that is not a tab stop gets the hover tooltip only. That is visual recovery. It does not meet 1.3.1, 4.1.2 or 1.4.4, and ADR 0036 is unchanged: there is still no accessibility tree.

## WCAG 2.2

This is not a conformance claim. It records, for the WCAG 2.2 success criteria that bear on ODK's components, which ones the components meet in what is tested, which do not apply to a toolkit, and which are open. Nothing here was checked with a screen reader.

**Met, in the behaviour the checklist tests.** 1.4.1 Use of Color: a link (a button link, a breadcrumb, a menu's link) is underlined at rest, and the current page is not a link; a tone has a mark; a number that is not a number says "Enter a number" as well as showing `!`. 1.4.3 Contrast (Minimum) and 1.4.11 Non-text Contrast: text is checked at 4.5:1 and essential non-text at 3:1 when a theme is resolved, and a theme that cannot reach them is refused. 2.1.1 Keyboard and 2.4.3 Focus Order: every component works from the keyboard, and a composite control is one tab stop. 2.1.2 No Keyboard Trap: a dialog with Escape, the scrim and the close button all turned off is refused. A dialog that turns only the scrim off still closes from the keyboard. 2.4.7 Focus Visible: the focused part draws a ring. 2.2.1 Timing Adjustable: a toast's clock pauses while the pointer is over the stack or the stack has focus, which is also while the stack is expanded, and Escape dismisses a toast from the keyboard, including one with no action button. 2.4.11 Focus Not Obscured (Minimum): a dialog or sheet body scrolls the focused control into view, and a non-modal sheet or a toast shifts so it does not completely cover the focused control. 2.5.8 Target Size (Minimum): field actions and checkbox, radio and switch rows are at least 24 px, and that floor does not shrink with the text size. Reduced motion is honoured as the Motion section says; that is not a claim about 2.3.3, which is AAA.

**Do not apply to the toolkit.** 2.5.7 Dragging Movements: no component requires a drag for its operation. 3.2.6 Consistent Help, 3.3.7 Redundant Entry and 3.3.8 Accessible Authentication: ODK does not provide an application's help, a multi-step form or a sign-in flow. A password field accepts paste and does not place the password on the clipboard. An application that adds any of those has to meet the criterion itself. 2.4.13 Focus Appearance is AAA (a 2 px perimeter and a 3:1 contrast of the indicator's own pixels) and is not implemented as that criterion states it.

**Open.** 1.3.1 Info and Relationships, 4.1.2 Name, Role, Value and 4.1.3 Status Messages need a platform accessibility tree. Components implement `Describe`, and the tests read it, but GPUI 0.2.2 exposes no tree, so a screen reader gets nothing. Truncation does not close any of those: the full string stays in `Describe` and in a tooltip when the control measured an overflow (ADR 0058), and ADR 0036 is unchanged. 1.4.4 Resize Text is not met: the ceiling is 167% of the 12 px default, as Text size says, and the clamp is unchanged. A tooltip on a truncated control is not a substitute for 1.4.4. 1.4.12 Text Spacing is not met: letter spacing cannot be drawn (it is always 0) and line height is the type role's, not the user's.

## Not covered

No screen reader, no platform accessibility API, no toast announcements, no high-contrast mode beyond what a theme's colours give, no `prefers-reduced-motion` from the desktop beyond Hyprland's window animations setting (ADR 0048; GNOME's `enable-animations` and the freedesktop portal were probed on 2026-10-23 and are not read), no zoom past the 9–20 px text size (ADR 0043; 20 px is 167% of 12, not 1.4.4's 200%), and no right-to-left layout. Manual checks made so far are in `docs/checks.md`.
