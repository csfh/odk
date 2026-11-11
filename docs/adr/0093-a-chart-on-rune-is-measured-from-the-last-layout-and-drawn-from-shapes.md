---
status: accepted
---

# A chart on Rune is measured from the last layout and drawn from shapes

ADR 0071 made a chart a canvas whose `paint` closure draws in the box's own coordinates, with the box's width and height known, every frame it is drawn, and with text and arcs. GPUI let it: `canvas` gives a paint callback with the laid-out bounds. Rune's `canvas` does not. It takes a declarative `Shape` (polylines, fills, and `Path`s of lines and cubic curves) built in the view, before layout, and has no text and no arc (checked on 2026-11-11 in the pinned revision, `docs/roadmap.md`). ADR 0089 moves `odk-ui` to Rune, so the contract needs an answer. The owner chose, on 2026-11-11, to measure the box from the last frame and keep the API.

## Decision

**`Chart` keeps ADR 0071's and ADR 0075's API** on Rune: `paint(|scene, progress|)`, the `Ink`s, `StrokeWidth`s, `TextAnchor` and `TextBaseline`, `ChartPath`, `height(ChartHeight)`, `transition(generation)`, `on_press`, `on_pan` and `on_zoom`, and the keys.

**The box is the size Rune reported on its last layout.** `Scene::width` and `Scene::height` are what `El::on_layout` last said the chart's element measured. Until the first report they are zero and `paint` is not called, so no layout is ever made from a zero box. The view runs again when the size changes (Rune's own rule for `on_layout`), so a chart appears one frame after it is first laid out, and follows a resize of its window one frame late. ADR 0071 said `paint` gets the box's coordinates each frame it is drawn; on Rune it gets the box of the frame before.

**What `paint` draws is recorded, then turned into Rune elements.** A `Scene` collects each mark in the box's coordinates; the chart then builds one Rune `Shape` for the fills and strokes and positions the text beside it:

- A `ChartPath` becomes a Rune `Path`: `move_to`, `line_to`, `curve_to` and `close` map to the same calls, and `arc` becomes cubic Bézier pieces of at most a quarter turn, which Rune has no arc for. A whole turn or more is a circle, as ADR 0071 has it.
- `fill_rect` and `stroke_rect` are four-point polygons.
- `text` is a Rune text element in the chart's type role, positioned by `TextAnchor` and `TextBaseline` from a container that spans the box, since the text's width is not known when the view is built. With `halo`, it is drawn first in four copies one `thin` stroke away in the canvas colour, then in its own.
- The chart's box clips what is drawn, as the GPUI one did.

**A transition is the host's.** `transition(generation)` starts a run of the foundation's `normal` on its easing, at once with reduced motion; the host's poll advances it at each frame while it runs and the view runs again with the eased progress, as it does for a spinner. A transition cannot know whether the chart is in view (Rune gives a view no such fact), so it runs to its end, at most `normal`, wherever the chart is; a chart at rest asks for nothing (checklist item 35).

**The pointer.** A press and a drag come from `El::on_drag` (ADR 0069: the drag lasts until the button is released; `Drag::cancelled` is Escape, which pans back by what the drag moved), the wheel from `El::on_wheel` (a plain wheel zooms about the pointer, Shift and the wheel pan, and a pinch zooms through `El::on_magnify`), and the keys from `El::on_key`, all as ADR 0071 has them, in the box's coordinates.

## Considered Options

- **The application states the size** (`Chart::size`). Exact and needs no measuring, but a chart would no longer fill its parent as ADR 0071 and ADR 0075 have it, and every application would learn its charts' widths.
- **Wait for Rune** to grow a paint-time callback. Nothing to amend, but GPUI cannot leave the workspace, which the v1 criteria require, until it does, or `Chart` is not in v1.

## Consequences

- ADR 0071 is amended by this record: its contract holds except that `paint` sees the last layout's box. ADR 0075 stands.
- A chart's first frame is empty and a resize lags by a frame. A sparkline that sizes itself from its first paint would see zero, so the application lays out from `scene.width()` and draws nothing while it is zero.
- `docs/checks.md` records the Rune facts the port met.
