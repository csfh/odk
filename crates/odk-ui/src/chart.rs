//! A canvas an application draws a chart on, in named inks, widths and text, with a transition
//! it is handed and a pointer it hears (ADR 0071). ODK draws no chart kinds itself.

use std::f32::consts::{PI, TAU};
use std::rc::Rc;
use std::time::Instant;

use gpui::{
    App, Bounds, DispatchPhase, ElementId, FocusHandle, Hsla, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement,
    PathBuilder, Pixels, Point, RenderOnce, ScrollWheelEvent, SharedString, Styled, TextRun,
    Window, canvas, div, fill, outline, point, px, size,
};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::{ChartStyle, HEAT_STEPS, SERIES_ROLES};
use odk_theme::foundation::MotionDuration;
use odk_theme::semantic;

use crate::components;
use crate::focus::{child_id, focus_handle, move_focus, part_name};
use crate::interaction::{focus_ring, request_frame};
use crate::theme::{colour, family};

/// A colour from the theme, as an ink resolves to (`odk_theme::Color`).
type ThemeColour = odk_theme::Color;

/// How far an arrow key pans, as a share of the chart's width or height. ODK's own.
const KEY_PAN: f32 = 0.05;
/// How much `+` and `-` zoom by, and one notch of the wheel. ODK's own.
const KEY_ZOOM: f32 = 1.25;
const WHEEL_ZOOM: f32 = 1.12;

/// A colour a chart draws in, by what it means. The theme says what each is (ADR 0071).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ink {
    /// A series' line or mark, by number; a seventh takes the first's colour again.
    Series(usize),
    /// A series' tint, for an area or a bar.
    SeriesFill(usize),
    /// A step of the sequential ramp, 0 (lowest) to 4 (highest); larger is the highest.
    Heat(usize),
    /// A Role's mark, such as `Role::Danger` for what failed.
    Role(semantic::Role),
    /// A Role's tint.
    RoleFill(semantic::Role),
    Text,
    TextMuted,
    Grid,
    Axis,
    /// The surface the chart is on, for a gap between marks.
    Canvas,
}

impl Ink {
    pub(crate) fn resolve(self, s: &ChartStyle) -> ThemeColour {
        let series = |n: usize| n % SERIES_ROLES.len();
        let role = |r: semantic::Role| SERIES_ROLES.iter().position(|x| *x == r).unwrap_or(0);
        match self {
            Ink::Series(n) => s.series[series(n)],
            Ink::SeriesFill(n) => s.series_fill[series(n)],
            Ink::Heat(level) => s.heat[level.min(HEAT_STEPS - 1)],
            Ink::Role(r) => s.series[role(r)],
            Ink::RoleFill(r) => s.series_fill[role(r)],
            Ink::Text => s.text,
            Ink::TextMuted => s.text_muted,
            Ink::Grid => s.grid,
            Ink::Axis => s.axis,
            Ink::Canvas => s.canvas,
        }
    }
}

/// How wide a stroke is: a border, the focus width, or twice that.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StrokeWidth {
    Thin,
    #[default]
    Regular,
    Thick,
}

impl StrokeWidth {
    fn px(self, s: &ChartStyle) -> f32 {
        match self {
            StrokeWidth::Thin => s.thin,
            StrokeWidth::Regular => s.regular,
            StrokeWidth::Thick => s.thick,
        }
    }
}

/// Where a text's point is along it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAnchor {
    #[default]
    Start,
    Middle,
    End,
}

/// Where a text's point is across it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextBaseline {
    Top,
    #[default]
    Middle,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Step {
    Move(f32, f32),
    Line(f32, f32),
    Cubic(f32, f32, f32, f32, f32, f32),
    Arc {
        cx: f32,
        cy: f32,
        r: f32,
        from: f32,
        to: f32,
    },
    Close,
}

/// A path in a chart's coordinates, from its top left corner, y down.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChartPath {
    steps: Vec<Step>,
}

impl ChartPath {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn move_to(mut self, x: f32, y: f32) -> Self {
        self.steps.push(Step::Move(x, y));
        self
    }

    pub fn line_to(mut self, x: f32, y: f32) -> Self {
        self.steps.push(Step::Line(x, y));
        self
    }

    /// A cubic Bézier curve to `(x, y)` through the control points `(c1x, c1y)` and
    /// `(c2x, c2y)`, as Canvas 2D's `bezierCurveTo`.
    pub fn curve_to(mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) -> Self {
        self.steps.push(Step::Cubic(c1x, c1y, c2x, c2y, x, y));
        self
    }

    /// An arc of the circle at `(cx, cy)` of radius `r` from angle `from` to `to`, in radians,
    /// clockwise on screen when `to` is larger, as Canvas 2D's `arc`. It joins the path with a
    /// line from where it was; a whole turn or more is a circle.
    pub fn arc(mut self, cx: f32, cy: f32, r: f32, from: f32, to: f32) -> Self {
        self.steps.push(Step::Arc {
            cx,
            cy,
            r,
            from,
            to,
        });
        self
    }

    pub fn close(mut self) -> Self {
        self.steps.push(Step::Close);
        self
    }

    /// The path as GPUI builds it, at `origin`; `None` for an empty path or one GPUI refuses.
    fn build(&self, mut b: PathBuilder, origin: Point<Pixels>) -> Option<gpui::Path<Pixels>> {
        let at = |x: f32, y: f32| point(origin.x + px(x), origin.y + px(y));
        let finite = |v: &[f32]| v.iter().all(|n| n.is_finite());
        let mut started = false;
        for step in &self.steps {
            match *step {
                Step::Move(x, y) if finite(&[x, y]) => {
                    b.move_to(at(x, y));
                    started = true;
                }
                Step::Line(x, y) if finite(&[x, y]) => {
                    if started {
                        b.line_to(at(x, y));
                    } else {
                        b.move_to(at(x, y));
                        started = true;
                    }
                }
                Step::Cubic(c1x, c1y, c2x, c2y, x, y)
                    if started && finite(&[c1x, c1y, c2x, c2y, x, y]) =>
                {
                    b.cubic_bezier_to(at(x, y), at(c1x, c1y), at(c2x, c2y));
                }
                Step::Arc {
                    cx,
                    cy,
                    r,
                    from,
                    to,
                } if finite(&[cx, cy, r, from, to]) && r > 0.0 => {
                    let start = (cx + r * from.cos(), cy + r * from.sin());
                    if started {
                        b.line_to(at(start.0, start.1));
                    } else {
                        b.move_to(at(start.0, start.1));
                        started = true;
                    }
                    for (sweep_to, large) in arc_pieces(from, to) {
                        let end = at(cx + r * sweep_to.cos(), cy + r * sweep_to.sin());
                        b.arc_to(point(px(r), px(r)), px(0.0), large, to > from, end);
                    }
                }
                Step::Close if started => b.close(),
                _ => {}
            }
        }
        started.then(|| b.build().ok()).flatten()
    }
}

/// An arc from `from` to `to` as SVG arcs: a turn or more is two halves of a circle, and
/// anything less is one arc, large when it is more than half a turn.
fn arc_pieces(from: f32, to: f32) -> Vec<(f32, bool)> {
    let sweep = to - from;
    if sweep.abs() >= TAU {
        let half = from + sweep.signum() * PI;
        vec![(half, false), (from + sweep.signum() * TAU, false)]
    } else {
        vec![(to, sweep.abs() > PI)]
    }
}

/// How tall a chart is when it asks rather than filling its parent (ADR 0075): `Compact` for a
/// sparkline, `Regular` for a chart, `Tall` for one that needs room, such as a code to scan.
/// Each is the foundation's spacing, so it grows with the text size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartHeight {
    Compact,
    Regular,
    Tall,
}

impl ChartHeight {
    fn px(self, s: &ChartStyle) -> f32 {
        match self {
            ChartHeight::Compact => s.heights[0],
            ChartHeight::Regular => s.heights[1],
            ChartHeight::Tall => s.heights[2],
        }
    }
}

/// What a chart's `paint` draws with, in the chart's own coordinates.
pub struct Scene<'a> {
    bounds: Bounds<Pixels>,
    style: ChartStyle,
    window: &'a mut Window,
    cx: &'a mut App,
}

impl Scene<'_> {
    pub fn width(&self) -> f32 {
        f32::from(self.bounds.size.width)
    }

    pub fn height(&self) -> f32 {
        f32::from(self.bounds.size.height)
    }

    /// The colour `ink` is in the current theme, for a mark the scene cannot draw.
    fn ink(&self, ink: Ink) -> ThemeColour {
        ink.resolve(&self.style)
    }

    pub fn fill(&mut self, path: &ChartPath, ink: Ink) {
        let colour = Hsla::from(colour(self.ink(ink)));
        if let Some(p) = path.build(PathBuilder::fill(), self.bounds.origin) {
            self.window.paint_path(p, colour);
        }
    }

    pub fn stroke(&mut self, path: &ChartPath, ink: Ink, width: StrokeWidth) {
        let colour = Hsla::from(colour(self.ink(ink)));
        let w = width.px(&self.style);
        if let Some(p) = path.build(PathBuilder::stroke(px(w)), self.bounds.origin) {
            self.window.paint_path(p, colour);
        }
    }

    fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> Option<Bounds<Pixels>> {
        [x, y, w, h].iter().all(|v| v.is_finite()).then(|| {
            Bounds::new(
                point(self.bounds.origin.x + px(x), self.bounds.origin.y + px(y)),
                size(px(w.max(0.0)), px(h.max(0.0))),
            )
        })
    }

    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, ink: Ink) {
        if let Some(b) = self.rect(x, y, w, h) {
            let colour = Hsla::from(colour(self.ink(ink)));
            self.window.paint_quad(fill(b, colour));
        }
    }

    pub fn stroke_rect(&mut self, x: f32, y: f32, w: f32, h: f32, ink: Ink, width: StrokeWidth) {
        if let Some(b) = self.rect(x, y, w, h) {
            let (colour, w) = (Hsla::from(colour(self.ink(ink))), width.px(&self.style));
            self.window
                .paint_quad(outline(b, colour, gpui::BorderStyle::Solid).border_widths(px(w)));
        }
    }

    /// `text` at `(x, y)`, the point placed by `anchor` along it and `baseline` across it, in
    /// the chart's type role. With `halo` it is drawn over a rim of the canvas's colour so it
    /// reads over a mark.
    #[allow(clippy::too_many_arguments)]
    pub fn text(
        &mut self,
        x: f32,
        y: f32,
        text: impl Into<SharedString>,
        ink: Ink,
        anchor: TextAnchor,
        baseline: TextBaseline,
        halo: bool,
    ) {
        let text = text.into();
        if text.is_empty() || !x.is_finite() || !y.is_finite() {
            return;
        }
        let f = components().foundation().clone();
        let t = f.typography.style(self.style.type_role);
        let font_size = px(t.size);
        let line_height = px(t.line_height);
        let mut font = self.window.text_style().font();
        font.family = family(&f, odk_theme::foundation::FontFamily::Ui);
        let shape = |colour: Hsla, window: &mut Window| {
            let run = TextRun {
                len: text.len(),
                font: font.clone(),
                color: colour,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            window
                .text_system()
                .shape_line(text.clone(), font_size, &[run], None)
        };
        let line = shape(Hsla::from(colour(self.ink(ink))), self.window);
        let width = f32::from(line.width);
        let dx = match anchor {
            TextAnchor::Start => 0.0,
            TextAnchor::Middle => width / 2.0,
            TextAnchor::End => width,
        };
        let lh = f32::from(line_height);
        let dy = match baseline {
            TextBaseline::Top => 0.0,
            TextBaseline::Middle => lh / 2.0,
            TextBaseline::Bottom => lh,
        };
        let origin = point(
            self.bounds.origin.x + px(x - dx),
            self.bounds.origin.y + px(y - dy),
        );
        if halo {
            let rim = shape(Hsla::from(colour(self.ink(Ink::Canvas))), self.window);
            let r = px(self.style.thin);
            for (ox, oy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
                let at = point(origin.x + r * ox, origin.y + r * oy);
                let _ = rim.paint(at, line_height, self.window, self.cx);
            }
        }
        let _ = line.paint(origin, line_height, self.window, self.cx);
    }
}

type Paint = Rc<dyn Fn(&mut Scene, f32)>;
type OnPoint = Rc<dyn Fn(f32, f32, &mut Window, &mut App)>;
type OnPan = Rc<dyn Fn(f32, f32, &mut Window, &mut App)>;
type OnZoom = Rc<dyn Fn(f32, f32, f32, &mut Window, &mut App)>;

/// A canvas the application draws a chart on (ADR 0071). It fills its parent; `paint` draws in
/// the box's own coordinates with [`Ink`]s, [`StrokeWidth`]s and text in the chart's type role,
/// and is handed how far the latest `transition` has got, from 0 to 1, so the application can
/// draw between its old and new layouts. The transition runs over the foundation's `normal`,
/// at once with reduced motion, and asks for frames only while it moves and the chart is in
/// view.
///
/// Keyboard: a chart with `on_press`, `on_pan` or `on_zoom` is a tab stop; the arrow keys pan
/// by a twentieth of the box, `+` and `-` zoom about its middle, and Enter or Space presses its
/// middle; Escape during a drag pans back by what it moved. The pointer presses, drags to pan
/// and zooms with the wheel (Shift and the wheel pan), all in the box's coordinates. A drag
/// reports each move, since the application redraws its view as it pans (ADR 0071).
///
/// ```ignore
/// Chart::new("cpu", "CPU over the last minute")
///     .description("Between 4 and 37%, now 12%")
///     .transition(sample)
///     .paint(move |scene, t| {
///         let path = layout.between(&old, t).path(scene.width(), scene.height());
///         scene.stroke(&path, Ink::Series(0), StrokeWidth::Regular);
///     })
/// ```
#[derive(IntoElement)]
pub struct Chart {
    id: ElementId,
    name: SharedString,
    description: Option<SharedString>,
    generation: u64,
    height: Option<ChartHeight>,
    paint: Option<Paint>,
    on_press: Option<OnPoint>,
    on_pan: Option<OnPan>,
    on_zoom: Option<OnZoom>,
}

impl Chart {
    /// A chart named `name`, such as "CPU over the last minute", for assistive technology.
    pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            generation: 0,
            height: None,
            paint: None,
            on_press: None,
            on_pan: None,
            on_zoom: None,
        }
    }

    /// What the chart shows, in words: what assistive technology reads in its place.
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// Its height, as wide as its parent: a chart that asks for one does not fill its parent's
    /// height, which is how a chart stands in a column of rows.
    pub fn height(mut self, height: ChartHeight) -> Self {
        self.height = Some(height);
        self
    }

    /// The data's generation, such as a sample's number: a new one starts a transition.
    pub fn transition(mut self, generation: u64) -> Self {
        self.generation = generation;
        self
    }

    /// Draws the chart: the scene and how far the transition has got.
    pub fn paint(mut self, f: impl Fn(&mut Scene, f32) + 'static) -> Self {
        self.paint = Some(Rc::new(f));
        self
    }

    /// Hears a press at a point of the box, or Enter or Space (its middle).
    pub fn on_press(mut self, f: impl Fn(f32, f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_press = Some(Rc::new(f));
        self
    }

    /// Hears a pan by `(dx, dy)`: a drag, Shift and the wheel, or an arrow key.
    pub fn on_pan(mut self, f: impl Fn(f32, f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_pan = Some(Rc::new(f));
        self
    }

    /// Hears a zoom by `factor` (above 1 is in) about a point: the wheel, or `+` and `-`.
    pub fn on_zoom(mut self, f: impl Fn(f32, f32, f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_zoom = Some(Rc::new(f));
        self
    }

    fn interactive(&self) -> bool {
        self.on_press.is_some() || self.on_pan.is_some() || self.on_zoom.is_some()
    }
}

impl Describe for Chart {
    fn accessible(&self) -> Accessible {
        let a = Accessible::new(Role::Image, self.name.to_string());
        match &self.description {
            Some(d) => a.description(d.to_string()),
            None => a,
        }
    }
}

/// Where a chart's transition is.
struct Motion {
    generation: u64,
    since: Option<Instant>,
}

/// Where a chart is, where a drag on it was last, and how far that drag has panned.
#[derive(Default)]
struct Pointer {
    bounds: Option<Bounds<Pixels>>,
    drag: Option<Point<Pixels>>,
    panned: (f32, f32),
}

/// How far a transition begun at `since` has got at `now`, eased, over `length`.
fn progress_at(since: Option<Instant>, now: Instant, length: std::time::Duration) -> f32 {
    match since {
        None => 1.0,
        Some(_) if length.is_zero() => 1.0,
        Some(t) => (now.saturating_duration_since(t).as_secs_f32() / length.as_secs_f32()).min(1.0),
    }
}

impl RenderOnce for Chart {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = components();
        let style = c.chart();
        let f = c.foundation();
        let length = f.motion.transition(MotionDuration::Normal);
        let now = cx.background_executor().now();
        let generation = self.generation;
        let motion = window.use_keyed_state(child_id(&self.id, "motion"), cx, |_, _| Motion {
            generation,
            since: None,
        });
        let t = motion.update(cx, |m, _| {
            if m.generation != generation {
                m.generation = generation;
                m.since = Some(now);
            }
            let raw = progress_at(m.since, now, length);
            if raw >= 1.0 {
                m.since = None;
            }
            raw
        });
        let eased = f.motion.easing.apply(t);
        #[cfg(test)]
        crate::drawn::record(&crate::focus::id_path(&self.id), eased);
        let moving = t < 1.0;

        let pointer =
            window.use_keyed_state(child_id(&self.id, "pointer"), cx, |_, _| Pointer::default());
        let paint = self.paint.clone();
        let probe = pointer.clone();
        let surface = canvas(
            move |bounds, window, cx| {
                probe.update(cx, |p, _| p.bounds = Some(bounds));
                // A chart nobody can see asks for nothing; scrolling it in draws it again.
                if moving && window.content_mask().bounds.intersects(&bounds) {
                    request_frame(window);
                }
            },
            move |bounds, _, window, cx| {
                if let Some(paint) = &paint {
                    window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
                        let mut scene = Scene {
                            bounds,
                            style,
                            window,
                            cx,
                        };
                        paint(&mut scene, eased);
                    });
                }
            },
        )
        .size_full();

        let chart = div()
            .id(self.id.clone())
            .debug_selector(|| part_name(&self.id, "chart"))
            .relative();
        let chart = match self.height {
            Some(h) => chart.w_full().h(px(h.px(&style))).flex_none(),
            None => chart.size_full(),
        };
        let mut chart = chart.min_w_0().min_h_0().child(surface);
        if !self.interactive() {
            return chart;
        }

        let focus: FocusHandle = focus_handle(child_id(&self.id, "focus"), true, window, cx);
        let focused = focus.is_focused(window);
        let (on_press, on_pan, on_zoom) = (self.on_press, self.on_pan, self.on_zoom);
        let local = |p: &Pointer, at: Point<Pixels>| -> (f32, f32) {
            p.bounds.map_or((0.0, 0.0), |b| {
                (f32::from(at.x - b.origin.x), f32::from(at.y - b.origin.y))
            })
        };
        {
            let (state, on_press, focus) = (pointer.clone(), on_press.clone(), focus.clone());
            let pans = on_pan.is_some();
            chart =
                chart.on_mouse_down(MouseButton::Left, move |e: &MouseDownEvent, window, cx| {
                    move_focus(&focus, window);
                    let (x, y) = local(state.read(cx), e.position);
                    if pans {
                        state.update(cx, |p, cx| {
                            p.drag = Some(e.position);
                            p.panned = (0.0, 0.0);
                            cx.notify();
                        });
                    }
                    if let Some(f) = &on_press {
                        f(x, y, window, cx);
                    }
                });
        }
        {
            let (state, on_pan, on_zoom) = (pointer.clone(), on_pan.clone(), on_zoom.clone());
            chart = chart.on_scroll_wheel(move |e: &ScrollWheelEvent, window, cx| {
                let lines = e.delta.pixel_delta(px(f32::from(window.line_height()))).y;
                let notches = f32::from(lines) / f32::from(window.line_height());
                if notches == 0.0 {
                    return;
                }
                let (x, y) = local(state.read(cx), e.position);
                if e.modifiers.shift {
                    if let Some(f) = &on_pan {
                        f(0.0, f32::from(lines), window, cx);
                    }
                } else if let Some(f) = &on_zoom {
                    f(WHEEL_ZOOM.powf(notches), x, y, window, cx);
                }
                cx.stop_propagation();
            });
        }
        {
            let state = pointer.clone();
            let (on_press, on_pan, on_zoom) = (on_press.clone(), on_pan.clone(), on_zoom.clone());
            chart = chart
                .track_focus(&focus)
                .on_key_down(move |e: &KeyDownEvent, window, cx| {
                    if e.keystroke.modifiers.control || e.keystroke.modifiers.alt {
                        return;
                    }
                    let (w, h) = state.read(cx).bounds.map_or((0.0, 0.0), |b| {
                        (f32::from(b.size.width), f32::from(b.size.height))
                    });
                    let (mx, my) = (w / 2.0, h / 2.0);
                    let key = e.keystroke.key.as_str();
                    // Escape during a drag pans back by what the drag moved, and ends it.
                    if key == "escape" {
                        let back = state.update(cx, |p, cx| {
                            p.drag.take()?;
                            cx.notify();
                            Some(std::mem::take(&mut p.panned))
                        });
                        if let (Some((dx, dy)), Some(f)) = (back, &on_pan) {
                            f(-dx, -dy, window, cx);
                            cx.stop_propagation();
                        }
                        return;
                    }
                    let pan = match key {
                        "left" => Some((-w * KEY_PAN, 0.0)),
                        "right" => Some((w * KEY_PAN, 0.0)),
                        "up" => Some((0.0, -h * KEY_PAN)),
                        "down" => Some((0.0, h * KEY_PAN)),
                        _ => None,
                    };
                    let handled = if let Some((dx, dy)) = pan {
                        on_pan.as_ref().map(|f| f(dx, dy, window, cx)).is_some()
                    } else {
                        match key {
                            "+" | "=" => on_zoom
                                .as_ref()
                                .map(|f| f(KEY_ZOOM, mx, my, window, cx))
                                .is_some(),
                            "-" => on_zoom
                                .as_ref()
                                .map(|f| f(1.0 / KEY_ZOOM, mx, my, window, cx))
                                .is_some(),
                            "enter" | "space" => {
                                on_press.as_ref().map(|f| f(mx, my, window, cx)).is_some()
                            }
                            _ => false,
                        }
                    };
                    if handled {
                        cx.stop_propagation();
                    }
                });
        }
        if focused {
            chart = chart.child(focus_ring(&style.focus, f.geometry.radius.default, 0.0));
        }

        // While a drag lasts, the whole window's moves and the release are the chart's
        // (ADR 0069).
        if pointer.read(cx).drag.is_some()
            && let Some(on_pan) = on_pan
        {
            let (moves, ups) = (pointer.clone(), pointer.clone());
            chart = chart.child(
                canvas(
                    |_, _, _| {},
                    move |_, _, window, _| {
                        window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                            if phase != DispatchPhase::Bubble {
                                return;
                            }
                            let moved = moves.update(cx, |p, _| {
                                let last = p.drag?;
                                p.drag = Some(e.position);
                                let (dx, dy) = (
                                    f32::from(e.position.x - last.x),
                                    f32::from(e.position.y - last.y),
                                );
                                p.panned = (p.panned.0 + dx, p.panned.1 + dy);
                                Some((dx, dy))
                            });
                            if let Some((dx, dy)) = moved {
                                on_pan(dx, dy, window, cx);
                            }
                        });
                        window.on_mouse_event(move |e: &MouseUpEvent, phase, _, cx| {
                            if phase == DispatchPhase::Bubble && e.button == MouseButton::Left {
                                ups.update(cx, |p, cx| {
                                    p.drag = None;
                                    cx.notify();
                                });
                            }
                        });
                    },
                )
                .absolute()
                .size_0(),
            );
        }
        chart
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turn_or_more_is_two_halves_and_less_is_one_arc() {
        assert_eq!(arc_pieces(0.0, TAU), vec![(PI, false), (TAU, false)]);
        assert_eq!(arc_pieces(0.0, PI / 2.0), vec![(PI / 2.0, false)]);
        assert_eq!(arc_pieces(0.0, 1.5 * PI), vec![(1.5 * PI, true)]);
        let back = arc_pieces(PI, 0.0);
        assert_eq!(back, vec![(0.0, false)], "anticlockwise");
    }

    #[test]
    fn a_transition_runs_over_its_length_and_ends() {
        let start = Instant::now();
        let length = std::time::Duration::from_millis(200);
        let at = |ms| start + std::time::Duration::from_millis(ms);
        assert_eq!(progress_at(None, at(0), length), 1.0, "at rest");
        assert_eq!(progress_at(Some(start), at(100), length), 0.5);
        assert_eq!(progress_at(Some(start), at(400), length), 1.0);
        assert_eq!(
            progress_at(Some(start), at(0), std::time::Duration::ZERO),
            1.0,
            "reduced motion"
        );
    }

    #[test]
    fn reports_an_image_with_its_description() {
        let a = Chart::new("c", "CPU").description("Now 12%").accessible();
        assert_eq!((a.role, a.name.as_str()), (Role::Image, "CPU"));
        assert_eq!(a.description.as_deref(), Some("Now 12%"));
    }
}
