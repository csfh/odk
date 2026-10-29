---
status: amended by ADR 0075
---

# A chart is a canvas the application draws on, in named inks

Atmos draws nineteen kinds of chart (a heatmap, a ridgeline, a treemap, a sankey, a sunburst, a radar, sparklines, bars) on a Canvas, with layouts of its own (`services/Charts.js`), and its plan is to port those layouts to Rust and keep them in Atmos (`docs/odk-primitives.md`, section B). What it draws with is small: paths of lines, Bézier curves and arcs, filled and stroked; filled and stroked rectangles; text at a point, with a halo so it reads over a fill. No gradient, dash, clip or transform. An application built from the prelude alone cannot use GPUI's `canvas`, a colour or a pixel number, and `docs/spec/odk-ui-components.md` left charts out of v1.

## Decision

**ODK draws no chart kinds; it gives a canvas.** `Chart::new(id, name)` is a box that fills its parent, and `paint(|scene| …)` draws into it, each frame it is drawn, in the box's own coordinates (`scene.width()`, `scene.height()`), with ODK's own small types: a `ChartPath` of `move_to`, `line_to`, `curve_to` (cubic), `arc` (a circle's arc by centre, radius and angles) and `close`; `fill`, `stroke`, `fill_rect`, `stroke_rect` and `text`. Layouts stay in the application.

**Colours are named, never given.** Every mark takes an `Ink`: a series by number (`Series(n)`, its line, and `SeriesFill(n)`, its tint), a step of a sequential ramp (`Heat(level)`, 0 to 4), a Role's mark and tint (`Role(role)`, `RoleFill(role)`), text (`Text`, `TextMuted`), the grid and the axis, and the surface itself (`Canvas`, for a gap or a halo). The component layer resolves them from the semantic layer: the series are the six Roles' focus colours in a fixed order, which reach `ui_contrast` over every surface, and their tints are the Roles' muted fills; the ramp runs from the neutral muted fill through the accent's tints to its focus colour. In a monochrome theme the series are greys, so a chart must also say what is what in words, a mark or a position (checklist item 18): the application labels its series.

**Widths and text are named too.** A stroke is `Thin` (a border), `Regular` (the focus width) or `Thick` (twice that); text is the `small` type role, anchored at its start, middle or end and its top, middle or baseline, with an optional halo in the canvas colour.

**Motion is a progress the chart hands the application.** `transition(generation)` says the data changed: the chart runs from 0 to 1 over the foundation's `normal` on its curve and passes that progress to `paint`, and the application draws the scene between the old and new layouts at it. Reduced motion gives 1 at once. A transition asks for frames only while it moves and while the chart is in view; a chart at rest or out of view asks for none (checklist item 35). Atmos tweens over its two-second sample instead, which would keep every visible chart drawing all the time; a chart now settles in `normal` after each sample and stands still until the next.

**The pointer, and its keys.** `on_press(|point| …)`, `on_pan(|dx, dy| …)` (a drag, ADR 0069, or the wheel with Shift) and `on_zoom(|factor, point| …)` (the wheel) report in the box's coordinates. A chart with any of them is a tab stop: the arrow keys pan by a twentieth of the box, `+` and `-` zoom about the middle, and Enter or Space presses the middle. Atmos builds its treemap's focus, pan and zoom on these. A drag holds the window until the button is released, as ADR 0069 has it, but a chart's pan reports at every move rather than once at the end: the application redraws its view as it pans, and keeps nothing per move. Escape during a drag pans back by what the drag moved, and ends it, so it still puts back what was there.

**Accessibility.** A chart reports `Image` with its name and a `description` the application gives, the words that say what the chart shows.

## Considered Options

- **Chart kinds in ODK.** Nineteen layouts, each a design of its own, for one application; the layouts are Atmos's and change with it.
- **GPUI's `canvas` through the escape hatch.** Atmos forbids it to itself so that nothing in it names a colour, a size or a GPUI type, and a theme change would not reach a colour the application computed.
- **The application's own colours with an alpha.** Atmos derives its fills as a colour at an alpha; that is colour maths outside the theme layers (checklist item 40). The tints and the ramp are resolved once per theme instead.
- **A tween over the sample's interval.** Smooth, but it is a redraw at every display frame for as long as a chart is in view.
