---
status: accepted
---

# Motion is declarative, on the foundation's tokens, and a decoration

ODK already moves a switch thumb, a tab indicator, a progress value, a spinner and the overlays. Each one uses the foundation's durations and the one curve, and each one stops when the user turns Hyprland's animations off. What an application cannot say is how a thing should enter, leave, follow a change of layout, or wait its turn, and the three drivers that do it today (`Presence`, `tween`, `progress`) each know one of those and nothing of the others.

This record adds that, in the spirit of [Motion](https://motion.dev) (formerly Framer Motion): a target, an initial state and an exit, a curve or a spring, a layout change, and a delay between children. It does not add a dependency, a new setting, or a second source for a duration. Durations and curves stay the foundation's (ADR 0035, ADR 0040). The user's request for reduced motion stays Hyprland's `animations:enabled` (ADR 0048). Nothing new is written into `shell.toml`.

Accepted by the owner on 2026-10-27 with every recommendation it made; they are the decisions at the end of this record. Nothing of it is built yet: until the driver lands, components keep the motion `docs/spec/odk-interaction.md` describes.

## Number

**0059.** Checked on 2026-10-25 against every published branch: `origin/main` and PR #4 (`cursor/settle-adrs-0046-0049-aaab`) and PR #5 (`cursor/wcag-22-fixes-d504`) stop at 0049; `origin/atmos-v1.0.0-rc` uses 0050 and 0051; PR #3 (`cursor/single-file-oma-adrs-60ba`) uses 0052 to 0055; PR #6 (`cursor/odk-cli-facade-adr-1222`) uses 0056. The owner's decorations record (ADR 0057) and the truncation record that builds on it (ADR 0058) were written beside it and landed with pull request #8. This record does not write 0057 or 0058.

## What is taken from Motion

Motion's React API is a component with `initial`, `animate` and `exit` targets, a `transition` that is either a duration and an easing or a spring, `AnimatePresence` so a removed element can play `exit`, `layout` and `layoutId` for layout and shared-element moves, variants with `stagger` and `delayChildren` for orchestration, and gesture props (`whileHover`, `whileTap`, `whileFocus`, `whileDrag`, `whileInView`). A target change retargets from the current value. `useReducedMotion` reads the platform's request.

| Motion idea | Here |
|---|---|
| `initial` / `animate` / `exit` | A component's default, or a `Transition` decoration. The end state is the component's own state; the decoration says how it gets there. |
| `AnimatePresence` | `Presence`, already how overlays leave (`overlay::Presence`). As a decoration it is how an application's own element opts in. |
| Duration and easing | Foundation tokens only. An application names a token. It does not pass a `Duration`. |
| Spring | One foundation spring, overdamped, for layout. Stock components stay on the curve. |
| `layout` / `layoutId` | A `Layout` decoration, and a `Shared` name reserved for later. |
| Stagger, sequence | `Stagger` and `Sequence` decorations. Named variants that flow down a tree are not built. |
| Gestures | Not built. Hover, press and focus stay instant colour changes. No stock component requires a drag. |
| Reduced motion | ADR 0048, defined by category below. |

## Reduced motion

This follows ADR 0048 (accepted), which supersedes ADR 0046. The user's request is Hyprland's `animations:enabled` read as false, over the socket, and followed on `configreloaded`. When Hyprland does not answer, the user has not asked, and motion is not reduced. The key in the user's own `~/.config/omarchy/shell.toml` is never a source. A sandboxed application cannot open the socket and must not; `odkd` reads the answer outside every sandbox and tells every application, a sandboxed one too (ADR 0062). The application's `odk::ui::set_reduced_motion(true)` still adds. An application's `false` never cancels the user. Unknown is not reduced.

**Reduced means the end state on the frame the state changes.** Every transition duration the driver would have run is zero, and every loop stops. A fade is a transition, so a fade is instant too. There is no crossfade-only mode. Hyprland's own `animations.enabled = false` disables every animation, fades included, and the contract already says the same thing (`MotionTokens::transition` is zero, checklist item 20). A crossfade would still be an animation a closing overlay would hold the frame for, and it would leave focus on a surface that had not yet reached its end.

| Category | Full motion | Reduced |
|---|---|---|
| Move (slide, rise, layout, a shared element) | The foundation duration and curve, or the layout spring | The end geometry this frame |
| Opacity of a scrim, a tooltip, a toast, a menu, a popover, a dialog | The component's own fade, on the same timeline as its move, and never longer than `enter` | The end opacity this frame |
| Scale, rotation, blur, colour | Not used. Stock components do not scale or spin a control, and a colour change is the new colour | The same |
| Spring | Integrated only while motion is full | Not integrated; the target is shown |
| Stagger and sequence delay | The foundation delay, capped | Zero; the children are at the end state together |
| Spinner | One turn per `motion.cycle`, woken when the glyph changes | The glyph stands still and stays visible |
| Determinate progress | `normal` | The value this frame |
| Hover, press, the focus ring's colour, a checkbox mark, a field's edge | Instant, as today | Instant |
| Delays that are not motion: `hover_open`, `hover_close`, the toast lifetime | Unchanged | Unchanged. Waiting is not motion |

Nothing in the stock set is essential motion. The end state carries the information: open or closed, the progress value, a spinner that is still drawn. There is no decoration and no argument that plays motion against the user's request. `NoMotion` can add reduction for one subtree. Nothing cancels it.

## Decorations

ADR 0057 is the owner's model: a component has a sensible default, and a behaviour changes by attaching a typed decoration, the way a class is attached. Decorations of different concerns compose. Two that name the same concern do not: the later one wins. Absent means the default. Truncation and overlay sizing are the first concern (ADR 0058): text flow (`Wrap`, `EndEllipsis`, `MiddleEllipsis`, `Clamp`) and size (`Autosize`, `FixedWidth`), with dialogs fixed by default and menus and toasts `Autosize` by default. The code attaches them with one `decorate` method per component, generic over the trait for the concerns that component accepts.

Motion is a further concern, `MotionDecoration`, attached the same way. This record does not invent a second attachment API. If ADR 0057's accepted shape differs, the motion concern follows that shape. A component that already takes a text or size decoration keeps a single `decorate`. Motion decorations implement the umbrella trait that component accepts. A text or size decoration writes no motion slot, and a motion decoration writes no flow and no size.

Slots compose with each other and with text flow and size, because they name different concerns. Two decorations that set the same slot: the later call wins. The slots are:

| Decoration | Slot | What it changes |
|---|---|---|
| `Transition` | the enter and exit tokens | Names foundation durations. It does not take a `Duration` or a bezier of the caller's own. |
| `Presence` | exit lifecycle | The element stays mounted until the exit has run. Built-in overlays already do this; an application attaches it to opt in. |
| `Layout` | bounds | A change of layout moves from the old bounds to the new. Off unless the component's default or the caller asks. |
| `Shared` | a shared name | Reserved. Two elements with one name would meet. Not in the first implementation. |
| `Stagger` | delay between children | Children start apart by the foundation step, capped. |
| `Sequence` | order against children | `Together` (the default), `BeforeChildren`, or `AfterChildren`. |
| `Spring` | the layout curve | The layout slot uses the foundation spring instead of the duration. |
| `NoMotion` | reduction | This subtree takes the reduced column. It is inherited. A child cannot clear it, and a later decoration of another slot does not clear it. |

`NoMotion` and the user's reduced motion outrank every other slot. That is a precedence exception to "later wins", and it is the same rule as the application's switch and the user's: either one reduces, and neither is turned off by a decoration.

Sensible defaults stay what the components do today, so an application that attaches nothing sees no change. The table in the adoption section is those defaults. `Autosize` (ADR 0058) changes the target size and does not by itself turn `Layout` on. A menu that grows to its longest label measures once. A toast stack that expands is the one stock layout transition, and it is that component's default, not an `Autosize` side effect.

## One source

No new `shell.toml` key, in the user's file or in a theme's. The driver reads `foundation().motion.reduced` and the duration and curve tokens, and nothing else. It does not read Hyprland's per-animation speeds or curves, and oma does not copy them into the sandbox. `animations:enabled` is the one Hyprland fact ODK asks for (ADR 0048). A duration lives in the foundation (ADR 0040) and nowhere beside it.

## GPUI 0.2.2

Pinned by ADR 0029. Read from the crates.io crate `gpui` 0.2.2 (`src/elements/animation.rs`, `src/window.rs`).

What it offers:

- `Animation::new(duration)`, `.repeat()`, `.with_easing(fn(f32) -> f32)`. One-shot by default. `with_animations` runs a list, one after another, and the closure is told the index.
- `AnimationExt::with_animation` wraps an element. On each layout it computes a delta in 0..1, calls the closure, and until the animation is done calls `Window::request_animation_frame`, which notifies the current view on the next frame.
- Easings shipped beside it: `linear`, `quadratic`, `ease_in_out`, `ease_out_quint`, `bounce`, `pulsating_between`. No cubic-bezier type. ODK already evaluates `cubic-bezier(0.22, 1, 0.36, 1)` itself because of that.
- `request_animation_frame` and `on_next_frame`.
- Transforms on an element (`Transformation` scale, translation, rotation) and drag callbacks (`on_drag`, `on_drag_move`).
- After layout, `layout_bounds` for an element that has been laid out.

What it does not offer, and what this record will not pretend it does:

- **Springs.** A spring is a position and a velocity toward a target. `Animation` is a 0..1 delta over a fixed duration.
- **Retargeting.** The start is `Instant::now()` stored once under the element id. Changing the `Animation` value does not restart it, and there is no velocity to continue from.
- **The test clock.** The delta uses `Instant::now()` and `elapsed()`. GPUI's test platform fakes `background_executor().now()` and leaves `Instant::now()` on the wall clock (`docs/checks.md`). Checklist item 31 requires the executor clock so `advance_clock` is the whole of time.
- **Exit.** An element that is not built is not drawn. Nothing keeps it mounted for an exit. `overlay::Presence` is that gap, filled already for overlays.
- **Layout and shared element.** No first/last bounds, no shared layout id. Bounds exist only after layout, for elements that are mounted.
- **A gesture model.** Drag is a callback. There is no hover, press or focus transition: `.hover()` and `.active()` apply on the frame the pointer changes, which is why those colours are instant.
- **An accessibility tree.** Unchanged (ADR 0036).

`with_animation` also asks for a frame on every layout until it finishes, which is the cost `docs/checks.md` ("Frame cost") measured, and `checklist/source.rs` forbids `with_animation`, `AnimationExt` and `request_animation_frame` outside the one wrapper `interaction::request_frame`. The motion driver uses that wrapper, and only while a value is not at its target.

**Where it lives.** The pure part stays in `odk-theme`, which already owns the durations, the curve and `reduced`, and which has no GPUI dependency, so the curve and the spring are tested without a window (the same reason the resolver lives there, ADR 0040). The driver lives in `odk-ui`, beside `Presence`, `tween` and `progress`, which it replaces. Applications reach it through the components and `odk::ui`, the facade of ADR 0028. No new crate. A crate earned its place when the code must not link GPUI (`odk-theme`, `odk-a11y`) or when it must be optional (`odk-db`). The driver cannot avoid GPUI, and the maths is already in the foundation. A plugin that needed to drive motion without the component set would be a later decision; v1 plugins have no UI API (ADR 0021).

## Tokens

ADR 0040's layers are unchanged. Motion is foundation, not source and not semantic.

The source layer gains nothing. `colors.toml` and `shell.toml` are not a curve and not a duration. Of `shell.toml`, ODK reads `[font] base-size` (ADR 0043) and, only in oma's sandbox copy, `[motion] reduced` (ADR 0048). This record adds no key.

The foundation keeps the tokens it has. Text size does not scale them (ADR 0043). The standard curve stays `cubic-bezier(0.22, 1, 0.36, 1)`. Any curve the foundation adds later is monotonic, starts at 0, ends at 1, and does not leave that range: the design language has no overshoot and no bounce, and a curve that left the range could flash.

Omarchy's default Hyprland file (`default/hypr/looknfeel.lua` on `basecamp/omarchy` master, the file ADR 0048 already cites) sets `animations.enabled = true` and then its own curves and leaves. Speeds are in deciseconds (Hyprland's `hl.animation`: 1 is 100 ms). The leaves an ODK window sits beside:

| Leaf | Speed | Curve | In ms |
|---|---:|---|---:|
| `windows` | 3.79 | `easeOutQuint` `(0.23, 1, 0.32, 1)` | 379 |
| `windowsIn` | 4.1 | `easeOutQuint`, style `popin 87%` | 410 |
| `windowsOut` | 1.49 | `linear`, style `popin 87%` | 149 |
| `fadeIn` | 1.73 | `almostLinear` `(0.5, 0.5, 0.75, 1)` | 173 |
| `fadeOut` | 1.46 | `almostLinear` | 146 |
| `layers` | 3.81 | `easeOutQuint` | 381 |
| `layersOut` | 1.5 | `linear` | 150 |

Hyprland's example config also defines a spring (`mass = 1`, `stiffness ≈ 238.12`, `damping ≈ 24.21`). That spring is underdamped (damping ratio about 0.78) and Omarchy's default file does not use it. ODK does not copy it.

The foundation's own numbers stay the ones ADR 0040 fixed: `fast` 140, `normal` 180, `slow` 240, `enter` 240, `exit` 160, the standard curve. They are close to Omarchy's `easeOutQuint` and shorter than a Hyprland window. Matching the compositor's windows would throw away durations the owner already set, and reading them live would be a second source. The sandbox could not see a user's custom curve without a new copied file, which this record refuses.

One spring token is added to the foundation, `settle`, critically damped (damping ratio 1), so it reaches the target and does not pass it. Its mass, stiffness and damping are ODK's, named beside the durations, and provisional in the same way `cycle` is. No stock component uses it until the layout driver does. A spring is a function of the time since it was aimed, from the position and velocity at that instant, so a test jumps the clock and reads the value. It is not a per-frame simulation that drifts with the frame rate.

`stagger` is a foundation delay, provisional, a step of 30 ms, and the whole group waits at most `slow` (240 ms). Children past the cap start with the last one. A long list cannot keep moving.

The component layer does not gain a parallel motion table. A component's default is a reference to these tokens, as the interaction contract's motion table already is. A decoration selects among the same tokens.

`hover_open`, `hover_close` and the toast lifetime stay delays, not transitions, and reduced motion does not shorten them.

## Accessibility

GPUI 0.2.2 has no accessibility tree (ADR 0036). `Describe` is still what a component reports, and a motion decoration does not change a role, a name or a state. An entering or leaving element reports the state it is going to, not "animating". A screen reader still hears nothing. PR #5 records the same limit and does not claim WCAG 2.3.3, which is AAA. This record meets it anyway, and says the level.

**2.3.3 Animation from Interactions (AAA).** Motion animation triggered by interaction can be turned off, unless it is essential. Reduced motion, as the table above, turns off every spatial animation and every fade. Stock motion is not essential. `NoMotion` turns it off for one subtree even when the user has not asked.

**2.2.2 Pause, Stop, Hide (A).** Anything that moves, blinks, scrolls or updates by itself for more than five seconds, beside other content, can be paused, stopped or hidden. The stock set has two cases. A spinner runs for as long as the work runs; the glyph is the busy state, the control stays, and reduced motion holds the glyph still. Stopping the glyph does not stop the work, and the pause belongs to the control that started the work, not to a second control on the spinner. A toast's lifetime is the five-second delay, which is 2.2.1 (PR #5: the clock pauses while the pointer is over the stack or the stack has focus, and Escape dismisses). The enter, the exit and the stack's expand are short and user-triggered. The stagger cap keeps a group inside `slow`. A loop the driver runs for itself, other than the spinner, is not added.

**2.3.1 Three Flashes or Below Threshold (A).** Nothing flashes more than three times in one second, in either mode. No bounce easing, no GPUI `pulsating_between`, no underdamped spring. One opacity step, including a scrim arriving at once under reduced motion, is one change and not a flash.

**2.2.1 Timing Adjustable (A),** already how toasts work on PR #5. The driver does not own the toast clock and does not restart it because a transition started, reversed or was cut short by reduced motion.

**2.5.7 Dragging Movements (AA).** PR #5: no component requires a drag. This record adds no gesture and no drag. A later reorder that needed a drag would need a keyboard path in that record, not in this one.

**Focus.** Focus is moved on the frame the state changes, through `move_focus`, and never onto an element that is exiting. The ring is drawn on that frame. It is not held back until a transition ends.

- On close, focus is restored on the frame the close starts, to the handle remembered at open, or left to GPUI when that handle is gone. The exiting subtree drops its tab stops and its hit testing on that frame. It may still be painted. Tab and the pointer cannot land in it. Restoration does not wait out the exit: waiting would leave the ring on the way out, or nowhere, for the length of the exit.
- An entrance fade may include the ring. Menus, popovers, dialogs and the command palette fade in over at most `enter` (240 ms) and take focus on the open frame; GPUI multiplies a parent's opacity onto the ring, and splitting those trees to exempt it would delete the fade or rebuild every overlay. Reduced motion is full opacity on the first frame, which is the request that asks for the end state immediately. 2.4.7 is met by the ring being present; 2.4.13 (AAA) is the contrast of the indicator's own pixels, and PR #5 does not claim it.
- A tooltip and a hover card never take focus. A toast does not take focus when it appears; its stack is a tab stop (PR #5). If the user moves focus onto a surface that is still fading, that surface's opacity goes to 1 on that frame, so a ring the user just moved is not left mid-fade. An overlay that placed focus itself on open is the entrance case above, and its fade runs.
- A layout move of the focused control keeps the ring on that control. The scroll-into-view that PR #5 added for a dialog, a sheet and a toast is asked for against the end bounds when the move starts, so the destination is on screen. Reduced motion is the same call with no distance to travel.
- A theme, density, corner or reduced-motion change still moves nothing and loses nothing (checklist item 39).

## Testing

Time is `cx.background_executor().now()` (checklist item 31). The driver never reads `Instant::now()`, which is why it does not use GPUI's `Animation`. Tests move time with `advance_clock` and draw the next frame, as `checklist/motion.rs` does now.

For a transition under test: the value at the start is the current value and not the target; a midpoint matches the curve at that fraction; the end value is the target once the duration has passed; a reverse at the midpoint continues from the value at the midpoint (item 22). A spring is asserted the same way from the closed form, including that it does not pass the target.

The reduced path is its own test, not a comment on the full one. `set_reduced_motion(true)`, one frame, the end state, and no further frame requested (items 20 and 39). The same for Hyprland's answer in the theme tests that already flip `animations:enabled`.

Interruption: a new target, reduced motion turned on midway, a theme change, and the element removed. Each one ends on the state that is now true and leaves no frame requested.

Presence: halfway through an exit the element is still painted; after the duration it is not; from the first exiting frame it is not a tab stop and does not hold focus. Layout: the bounds at the end are the laid-out bounds, the midpoint is between the old and the new, and reduced motion is the new bounds on the first frame.

The source scan already forbids `with_animation`, `AnimationExt`, `request_animation_frame` and `Instant::now()` in components (items 31 and 35). The driver's frame requests go through `request_frame` and are counted. A settled window asks for none.

## Adoption

No application API changes in the first slice. The private drivers become one driver and the checklist stays green. Decorations are attached after ADR 0057's `decorate` exists. Until then the defaults below are what the components do, expressed as the tokens they already pass.

| Component | Default, full motion | Reduced | Note |
|---|---|---|---|
| Switch thumb, tab indicator | `normal`, standard curve | End state | Already `progress` |
| Progress bar | `normal` | The value | Already `tween`. The indeterminate spinner is the spinner row |
| Spinner | `cycle`, a timer per glyph | Still, visible | Not a frame loop |
| Button hover, press, focus, checkbox, field edge | Instant | Instant | Unchanged |
| Menu, select list | in `normal`, out `exit` | End state | `menu_list.rs`. Width is measured, not animated, including under `Autosize` |
| Popover, hover card | in `normal`, out `exit` | End state | |
| Tooltip | in `fast`, out `exit`, a fade | End state | Never takes focus |
| Dialog, command palette | fade and a slight rise over `enter`; scrim fades with them; out `exit` | End state, focus restored the same frame | Focus moves on the open frame. On close it leaves before the exit paints |
| Sheet, modal or side panel | slide over `enter` / `exit`; a modal's scrim fades | End state | The panel is not faded today, and is not given a fade |
| Toast card | in `normal`, out `exit`, fade and a short slide | End state | May fade: it does not take focus when it appears |
| Toast stack (ADR 0058) | expand and collapse of the collapsed group over `normal` (`Layout`); the count is the new number immediately | End height this frame | Timers pause while expanded, and while the pointer or focus is on the stack (PR #5). The driver does not run the timer. Scroll when there are too many is the list's scroll, at the end height under reduced motion |
| Scrim | Opacity follows the overlay's presence | End opacity | Not its own timeline. Hit testing stays as ADR 0045 for as long as the overlay is open, and goes when the close starts |

`Shared` is not used by any of these. A tab indicator is a fade between two underlines, not a shared element.

## Decisions

Settled by the owner on 2026-10-27, each as this record recommended.

1. **Is reduced motion instant for a fade as well as for a move?** Decided: yes. One rule, the one `transition()` already implements, and the one Hyprland's switch implements. A crossfade-only mode is a second policy and would keep an exit on screen after the state had changed.

2. **A new `odk-motion` crate?** Decided: no. The solver stays in `odk-theme`, the driver in `odk-ui`.

3. **Follow Hyprland's speeds and curves live, or copy Omarchy's `easeOutQuint` `(0.23, 1, 0.32, 1)` over the foundation's `(0.22, 1, 0.36, 1)`?** Decided: neither. Keep ADR 0040's durations and curve. Do not read `hl.animation`.

4. **Springs at all, given the design language forbids overshoot?** Decided: one critically damped `settle` token, used by layout only, unused by the stock components until then. No underdamped spring, and not Hyprland's example spring.

5. **Shared elements and gestures in the first implementation?** Decided: reserve the `Shared` slot, implement it only after `Layout` is in the checklist, and do not add gestures. Hover and press stay instant. No drag.

6. **Should a surface that holds focus be forced to full opacity, removing the entrance fade from menus, popovers, dialogs and the command palette?** Decided: no. Keep those fades. They are at most `enter` (240 ms), focus and the ring arrive on the open frame, and reduced motion is already full opacity on that frame. Forcing opacity 1 whenever a surface is focused would delete the fades the motion table already specifies. What does change is the close: focus leaves on the first exiting frame, and the exiting surface is not a tab stop, so the ring is never on the way out.

7. **May an application pass its own `Duration` or bezier?** Decided: no. `Transition` names foundation tokens. Item 19 then holds for application code as well as for components.

8. **Does `NoMotion` on a parent bind the children?** Decided: yes, and a child cannot clear it.

9. **What is the stagger step?** Decided: 30 ms, provisional, and the group never waits longer than `slow` (240 ms).

10. **Toast stack motion against ADR 0058.** Decided: the expand and collapse is `Layout` over `normal`; the grouped count changes immediately; timers stay the toast manager's, paused while expanded and while hovered or focused; scroll is the list, not a decoration. If ADR 0058's accepted behaviour differs, the motion follows that behaviour and only supplies the timeline.

## Consequences

- `docs/spec/odk-motion.md` is the spec. Until the driver is built, a component keeps the motion `docs/spec/odk-interaction.md` and `docs/spec/odk-foundation-tokens.md` describe, and its "First implementation" is the order of work.
- With the code, not before it: the interaction contract's sentence "every transition is zero" becomes the category table (the outcome is the same; the close-focus rule is the part that is new); the foundation spec gains `settle` and `stagger`; `docs/accessibility.md` gains 2.3.3, 2.2.2 and 2.3.1 as met by this policy. Entrance fades stay. Until then those documents describe what is built.
- ADR 0028 is not amended: no crate and no facade module. ADR 0035 and ADR 0040 are not amended: no duration moves to Omarchy. ADR 0048 is not amended: this record uses it and does not add a key.
- The first implementation is a replacement of the private drivers, checked by the existing motion tests, then the decorations, then the close-focus rule. `Shared` and gestures are not part of it.
