# ODK motion

The spec for ADR 0059 (accepted on 2026-10-27). Not built: until the driver lands, a component keeps the motion it has today, which `odk-interaction.md` and `odk-foundation-tokens.md` describe; "First implementation" below is the order of work. An ADR wins over a spec in any case.

The words are the glossary's. **Reduced motion** is the user's or the application's request that every transition show its end state at once. **Motion** is how a control gets from one state it already has to the next.

## What a caller writes

Attachment is ADR 0057's `decorate`, one method on the component. A motion decoration is a value of one of the types below. It implements the trait the component's `decorate` already accepts, and it writes one slot. A text or size decoration writes none of these slots. Two slots compose. The same slot: the later call wins. `NoMotion` is not cleared by a later call.

```rust
dialog.decorate(NoMotion)
dialog.decorate(Transition::new(MotionDuration::Fast, MotionDuration::Exit))
list.decorate(Stagger::default())
list.decorate(Sequence::BeforeChildren)
panel.decorate(Spring::Settle)   // layout slot only
panel.decorate(Layout)
// Shared is reserved and is not constructed by the first implementation.
```

`Transition` takes two `MotionDuration`s, enter and exit. It does not take `std::time::Duration`, a millisecond count, or control points. `Spring::Settle` selects the foundation spring for the layout slot. `Stagger::default` is the foundation step. `Layout` with no spring uses the component's duration and the standard curve.

Built-in overlays already have presence. An application element that should play an exit opts in:

```rust
row.decorate(Presence)
row.decorate(Transition::new(MotionDuration::Normal, MotionDuration::Exit))
```

A component that accepts no motion decoration does not grow a `decorate` for it. Hover, press and focus colour are not a slot.

## Defaults

An application that attaches nothing gets the component's default. The defaults are the adoption table in ADR 0059. In short: the switch, the tab indicator and the progress value use `normal`; menus, popovers and hover cards enter on `normal` and leave on `exit`; a tooltip enters on `fast`; a dialog, a sheet and the command palette enter on `enter` and leave on `exit`; a toast card enters on `normal` and leaves on `exit`; the toast stack's expand and collapse is `Layout` over `normal` once ADR 0058's collapsed stack exists. A scrim's opacity is the presence amount. A menu's width is measured once. Entrance fades stay, including on a menu, a popover, a dialog and the command palette.

`Autosize` (ADR 0058) sets the target size. It does not turn `Layout` on.

## Reduced motion

On when the application asks (`odk::ui::set_reduced_motion`) or the user does. The user asks when Hyprland's `animations:enabled` is false (ADR 0048), which `odkd` tells every application, a sandboxed one too (ADR 0062). No answer is not a request. Either request is enough. `NoMotion` is a third way to reduce one subtree, and it is inherited.

When it is on, the driver reports the target on that frame: geometry, opacity, a spring's target, a stagger's children. The spinner's glyph does not advance. A progress bar shows its value. `hover_open`, `hover_close` and the toast lifetime are not shortened.

When it is off, the categories in ADR 0059 apply. An overlay that placed focus itself on open may fade in over at most `enter`, ring included. If the user later moves focus onto a surface that is still fading, that surface's opacity goes to 1 on that frame and a move that was in progress continues from where it was.

## Driver

One driver in `odk-ui` replaces `overlay::Presence`, `interaction::tween` and `interaction::progress`. The checklist's observable rules stay: item 19 (foundation tokens only), item 20 (reduced), item 21 (an exit is kept and drawn), item 22 (a reverse continues from the current value), item 31 (the executor clock), item 35 (no frame while settled), item 39 (an interruption ends on the true state and asks for nothing more).

State per running transition: the value at the last aim, the target, the velocity for a spring, and the executor instant it was aimed. A new target stores the current value and velocity and aims again. A zero duration, or reduced motion, stores the target and no instant.

Time is `cx.background_executor().now()`. The driver does not call GPUI's `with_animation` and does not read `Instant::now()`. Frames are requested through `interaction::request_frame`, and only while the value is not the target. A spinner keeps its own timer, as it does today.

Opacity, translation and layout bounds are the channels. Scale, rotation, blur and colour are not channels. A colour is the colour of the new state.

A spring is evaluated from the closed form of a critically damped oscillator at the elapsed time, from the position and velocity stored at the aim. The same elapsed time is the same value on every call. It does not step with the frame, so a test that jumps the clock does not depend on how many frames it drew. The foundation's `settle` token holds the mass, stiffness and damping; the damping ratio is 1. The value does not pass the target.

Layout stores the bounds of an element id after each frame. When the laid-out bounds differ, the driver paints a transform from the old bounds toward the new and removes it at the end, so the element ends where layout put it. Under reduced motion it paints no transform. `Shared` would store those bounds under a caller's name so a mounting element could start from an unmounting one's last bounds. It is not implemented in the first slice; the slot is reserved so a decoration of that name is not given a second meaning.

Stagger is a start delay of `min(index * stagger, slow)` on each child. `Sequence::BeforeChildren` starts the parent, then the children; `AfterChildren` the other way; `Together` is no extra wait. The delays are zero when motion is reduced.

Presence keeps an element that has asked to leave mounted until its exit amount is 0, then drops it. On the frame it starts leaving it is removed from the tab order and from hit testing. Focus is moved out on that frame by the overlay code that already remembers the previous handle (`layer_over`). Focus is never moved to an element whose presence is closing.

## Tokens

Foundation, beside the durations that exist today (`odk-foundation-tokens.md`). Text size does not scale them. Reduced motion does not change the stored numbers; `transition()` is what returns zero.

| Token | Value | Status |
|---|---|---|
| `fast`, `normal`, `slow`, `enter`, `exit` | 140, 180, 240, 240, 160 ms | Existing (ADR 0040) |
| `easing` standard | `cubic-bezier(0.22, 1, 0.36, 1)` | Existing. Monotonic, within 0 and 1 |
| `settle` | critically damped spring, damping ratio 1 | Provisional, for layout |
| `stagger` | 30 ms, and a group waits at most `slow` | Provisional |

`cycle`, `hover_open`, `hover_close` and the toast lifetime are unchanged. Hyprland's leaf speeds and beziers are not tokens and are not read.

## Focus and the criteria

- Focus moves on the frame the overlay opens, and back on the frame it starts to close. The exit duration is not a delay on the move. An exiting element is not a tab stop and does not hit-test.
- The ring is drawn on the open frame. An entrance fade may include it. Reduced motion draws the surface at full opacity on that frame.
- PR #5's scroll-into-view runs against the end bounds when a layout transition of the focused control starts.
- 2.3.3 (AAA): reduced motion disables the motion. 2.2.2 (A): nothing of the driver's own moves for more than five seconds; the stagger cap is inside `slow`; the spinner is the busy glyph. 2.3.1 (A): no flashing above three a second, no bounce, no underdamped spring. 2.2.1 (A): the toast clock is the manager's, paused as PR #5 says, including while ADR 0058's stack is expanded.
- `Describe` is unchanged. GPUI 0.2.2 still has no tree (ADR 0036).

## Tests

Added to `checklist/motion.rs` and the theme tests, on the test platform, with `advance_clock`.

- A tween and a presence: start value, eased midpoint, end value, reverse from the midpoint, zero frames once settled.
- Reduced: one frame to the end state, no frame afterwards, for a move, a fade, a spring, a stagger and a spinner.
- Interruptions: new target, reduced motion turned on midway, theme change, element removed. No frame left running.
- Presence: painted at mid-exit, gone after, not a tab stop and not focused on the first exiting frame, focus on the restored handle.
- Layout: midpoint between the old and the new bounds, end bounds equal to layout, reduced motion on the new bounds immediately.
- Spring, in `odk-theme` with no window: the closed form at a given elapsed time, no overshoot, the same answer on a second call.
- Source scan: components still do not name `with_animation`, `AnimationExt`, `request_animation_frame` or `Instant::now()` (items 31 and 35; both scans exist today).

Break each new assertion once and see it fail before trusting it, as the checklist requires.

## First implementation

1. One driver behind the existing private calls. No public decoration yet. Existing motion tests stay green, including the entrance fades.
2. `settle` and `stagger` on the foundation, with their unit tests, used by nothing stock.
3. Close: focus leaves on the first exiting frame, the exiting surface drops its tab stops and its hit testing, and the exit may still paint. Checklist updated for that.
4. After ADR 0057: `MotionDecoration` and the types above, on the components that animate. `NoMotion` covered by the reduced tests.
5. After ADR 0058: the toast stack's expand and collapse on `Layout`, the count immediate, the timers untouched.
6. `Layout` and `Stagger` available to an application. `Shared` and gestures not started.

## Out of scope

GPUI's `Animation` and its easings `bounce` and `pulsating_between`. A raw duration or a caller-supplied bezier. A `shell.toml` motion key. Reading Hyprland's `hl.animation` or `hl.curve`. An underdamped spring. Scale and rotation of a control. Animating a colour. Hover and press transitions. Drag and `whileInView`. Named variants. A new crate. Plugins driving motion.
