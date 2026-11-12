//! A canvas an application draws a chart on, in named inks, widths and text, with a transition it
//! is handed and a pointer it hears (ADR 0071), on the Rune host. ODK draws no chart kinds itself.
//!
//! Rune's canvas is a declarative shape and not a paint callback, so the box is the size Rune
//! reported on the last layout and what `paint` draws is recorded and turned into Rune elements
//! (ADR 0093).

use std::cell::RefCell;
use std::collections::HashMap;
use std::f32::consts::{PI, TAU};
use std::rc::Rc;
use std::time::Duration;

use crate::describe::apply;
use crate::host::Hosted;
use crate::ink::ink;
use crate::style::{self, Paint, focus_ring};
use crate::text::plain;
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::{ChartStyle, HEAT_STEPS, SERIES_ROLES};
use odk_theme::foundation::MotionDuration;
use odk_theme::semantic;
use rune::{
    Drag, El, Key, KeyEvent, LineCap, MagnifyEvent, Path, Phase, Shape, Tx, WheelEvent, canvas, el,
};

/// A colour from the theme, as an ink resolves to.
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
    fn resolve(self, s: &ChartStyle) -> ThemeColour {
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
    fn points(self, s: &ChartStyle) -> f32 {
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

    /// The path as Rune draws it; `None` for an empty path. Rune has no arc, so an arc is cubic
    /// pieces of at most a quarter turn (ADR 0093).
    fn to_rune(&self) -> Option<Path> {
        let finite = |v: &[f32]| v.iter().all(|n| n.is_finite());
        let mut path = Path::new();
        let mut started = false;
        for step in &self.steps {
            match *step {
                Step::Move(x, y) if finite(&[x, y]) => {
                    path = path.move_to([x, y]);
                    started = true;
                }
                Step::Line(x, y) if finite(&[x, y]) => {
                    path = if started {
                        path.line_to([x, y])
                    } else {
                        started = true;
                        path.move_to([x, y])
                    };
                }
                Step::Cubic(c1x, c1y, c2x, c2y, x, y)
                    if started && finite(&[c1x, c1y, c2x, c2y, x, y]) =>
                {
                    path = path.cubic_to([c1x, c1y], [c2x, c2y], [x, y]);
                }
                Step::Arc {
                    cx,
                    cy,
                    r,
                    from,
                    to,
                } if finite(&[cx, cy, r, from, to]) && r > 0.0 && from != to => {
                    let start = [cx + r * from.cos(), cy + r * from.sin()];
                    path = if started {
                        path.line_to(start)
                    } else {
                        started = true;
                        path.move_to(start)
                    };
                    for (c1, c2, end) in arc_cubics(cx, cy, r, from, to) {
                        path = path.cubic_to(c1, c2, end);
                    }
                }
                Step::Close if started => path = path.close(),
                _ => {}
            }
        }
        started.then_some(path)
    }
}

/// An arc as cubic Bézier curves, each of at most a quarter turn: the standard approximation,
/// whose control points lie `4/3 * tan(sweep / 4)` of the radius along the tangents.
fn arc_cubics(cx: f32, cy: f32, r: f32, from: f32, to: f32) -> Vec<([f32; 2], [f32; 2], [f32; 2])> {
    let sweep = (to - from).clamp(-TAU, TAU);
    let pieces = ((sweep.abs() / (PI / 2.0)).ceil() as usize).max(1);
    let step = sweep / pieces as f32;
    let k = 4.0 / 3.0 * (step / 4.0).tan();
    (0..pieces)
        .map(|i| {
            let a0 = from + step * i as f32;
            let a1 = a0 + step;
            let p0 = [cx + r * a0.cos(), cy + r * a0.sin()];
            let p1 = [cx + r * a1.cos(), cy + r * a1.sin()];
            let c1 = [p0[0] - k * r * a0.sin(), p0[1] + k * r * a0.cos()];
            let c2 = [p1[0] + k * r * a1.sin(), p1[1] - k * r * a1.cos()];
            (c1, c2, p1)
        })
        .collect()
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
    fn points(self, s: &ChartStyle) -> f32 {
        match self {
            ChartHeight::Compact => s.heights[0],
            ChartHeight::Regular => s.heights[1],
            ChartHeight::Tall => s.heights[2],
        }
    }
}

/// One mark a `paint` drew.
enum Mark {
    Fill(ChartPath, ThemeColour),
    Stroke(ChartPath, f32, ThemeColour),
    Text {
        x: f32,
        y: f32,
        text: String,
        colour: ThemeColour,
        anchor: TextAnchor,
        baseline: TextBaseline,
        halo: bool,
    },
}

/// What a chart's `paint` draws with, in the chart's own coordinates. It records each mark; the
/// chart draws them once `paint` returns (ADR 0093).
pub struct Scene {
    width: f32,
    height: f32,
    style: ChartStyle,
    marks: Vec<Mark>,
}

impl Scene {
    /// The box's width in points, as Rune last laid it out.
    pub fn width(&self) -> f32 {
        self.width
    }

    /// The box's height in points, as Rune last laid it out.
    pub fn height(&self) -> f32 {
        self.height
    }

    pub fn fill(&mut self, path: &ChartPath, ink: Ink) {
        self.marks
            .push(Mark::Fill(path.clone(), ink.resolve(&self.style)));
    }

    pub fn stroke(&mut self, path: &ChartPath, ink: Ink, width: StrokeWidth) {
        self.marks.push(Mark::Stroke(
            path.clone(),
            width.points(&self.style),
            ink.resolve(&self.style),
        ));
    }

    fn rectangle(x: f32, y: f32, w: f32, h: f32) -> Option<ChartPath> {
        [x, y, w, h].iter().all(|v| v.is_finite()).then(|| {
            let (w, h) = (w.max(0.0), h.max(0.0));
            ChartPath::new()
                .move_to(x, y)
                .line_to(x + w, y)
                .line_to(x + w, y + h)
                .line_to(x, y + h)
                .close()
        })
    }

    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, ink: Ink) {
        if let Some(path) = Self::rectangle(x, y, w, h) {
            self.fill(&path, ink);
        }
    }

    pub fn stroke_rect(&mut self, x: f32, y: f32, w: f32, h: f32, ink: Ink, width: StrokeWidth) {
        if let Some(path) = Self::rectangle(x, y, w, h) {
            self.stroke(&path, ink, width);
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
        text: impl Into<String>,
        ink: Ink,
        anchor: TextAnchor,
        baseline: TextBaseline,
        halo: bool,
    ) {
        let text = text.into();
        if text.is_empty() || !x.is_finite() || !y.is_finite() {
            return;
        }
        self.marks.push(Mark::Text {
            x,
            y,
            text,
            colour: ink.resolve(&self.style),
            anchor,
            baseline,
            halo,
        });
    }
}

type PaintFn = Rc<dyn Fn(&mut Scene, f32)>;
type OnPoint<T> = Rc<dyn Fn(&mut T, f32, f32)>;
type OnZoom<T> = Rc<dyn Fn(&mut T, f32, f32, f32)>;

/// A canvas the application draws a chart on (ADR 0071, ADR 0093). It fills its parent, or is as
/// tall as `height` says; `paint` draws in the box's own coordinates with [`Ink`]s,
/// [`StrokeWidth`]s and text in the chart's type role, and is handed how far the latest
/// `transition` has got, from 0 to 1, so the application can draw between its old and new
/// layouts. The box is the size of the chart's last layout, so `paint` is not called before the
/// first one.
///
/// Keyboard: a chart with `on_press`, `on_pan` or `on_zoom` is a tab stop; the arrow keys pan by
/// a twentieth of the box, `+` and `-` zoom about its middle, and Enter or Space presses its
/// middle; Escape during a drag pans back by what it moved. The pointer presses, drags to pan and
/// zooms with the wheel or a pinch (Shift and the wheel pan), all in the box's coordinates. A
/// drag reports each move, since the application redraws its view as it pans (ADR 0071).
pub struct Chart<T> {
    id: String,
    name: String,
    description: Option<String>,
    generation: u64,
    height: Option<ChartHeight>,
    paint: Option<PaintFn>,
    on_press: Option<OnPoint<T>>,
    on_pan: Option<OnPoint<T>>,
    on_zoom: Option<OnZoom<T>>,
}

impl<T: 'static> Chart<T> {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
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

    /// The words that say what the chart shows, for a screen reader.
    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// Asks for a height instead of filling the parent's (ADR 0075).
    pub fn height(mut self, height: ChartHeight) -> Self {
        self.height = Some(height);
        self
    }

    /// Says the data changed: the chart runs from 0 to 1 over the foundation's `normal`.
    pub fn transition(mut self, generation: u64) -> Self {
        self.generation = generation;
        self
    }

    pub fn paint(mut self, f: impl Fn(&mut Scene, f32) + 'static) -> Self {
        self.paint = Some(Rc::new(f));
        self
    }

    /// A press, or Enter or Space, at `(x, y)` in the box's coordinates.
    pub fn on_press(mut self, f: impl Fn(&mut T, f32, f32) + 'static) -> Self {
        self.on_press = Some(Rc::new(f));
        self
    }

    /// A pan by `(dx, dy)` points: a drag's move, an arrow key, or Shift and the wheel.
    pub fn on_pan(mut self, f: impl Fn(&mut T, f32, f32) + 'static) -> Self {
        self.on_pan = Some(Rc::new(f));
        self
    }

    /// A zoom by `factor` about `(x, y)`: the wheel, a pinch, `+` and `-`.
    pub fn on_zoom(mut self, f: impl Fn(&mut T, f32, f32, f32) + 'static) -> Self {
        self.on_zoom = Some(Rc::new(f));
        self
    }

    fn interactive(&self) -> bool {
        self.on_press.is_some() || self.on_pan.is_some() || self.on_zoom.is_some()
    }
}

impl<T> Describe for Chart<T> {
    fn accessible(&self) -> Accessible {
        let accessible = Accessible::new(Role::Image, self.name.clone());
        match &self.description {
            Some(description) => accessible.description(description.clone()),
            None => accessible,
        }
    }
}

/// What the window keeps for one chart.
#[derive(Default)]
struct Slot {
    /// The box as Rune last laid it out.
    width: f32,
    height: f32,
    generation: u64,
    /// Whether a transition runs, when it began on the host's clock, and how far it has got.
    moving: bool,
    began: Option<f64>,
    progress: f32,
    /// A drag in progress: where the last move left it and how far it has panned in all.
    drag: Option<Drag2>,
}

#[derive(Clone, Copy, Default)]
struct Drag2 {
    last: (f32, f32),
    panned: (f32, f32),
}

/// The charts of a window.
#[derive(Default)]
pub(crate) struct Charts {
    slots: RefCell<HashMap<String, Slot>>,
}

impl Charts {
    /// What a build of the chart `id` sees: its box, and how far its transition has got (1 when
    /// none runs). A new `generation` starts a transition; reduced motion has it done at once.
    fn begin(&self, id: &str, generation: u64, reduced: bool) -> (f32, f32, f32) {
        let mut slots = self.slots.borrow_mut();
        let first = !slots.contains_key(id);
        let slot = slots.entry(id.to_string()).or_default();
        if first {
            slot.generation = generation;
        } else if slot.generation != generation {
            slot.generation = generation;
            slot.moving = !reduced;
            slot.began = None;
            slot.progress = 0.0;
        }
        if reduced {
            slot.moving = false;
        }
        let progress = if slot.moving { slot.progress } else { 1.0 };
        (slot.width, slot.height, progress)
    }

    fn measure(&self, id: &str, width: f32, height: f32) {
        let mut slots = self.slots.borrow_mut();
        let slot = slots.entry(id.to_string()).or_default();
        slot.width = width;
        slot.height = height;
    }

    fn size(&self, id: &str) -> (f32, f32) {
        self.slots
            .borrow()
            .get(id)
            .map_or((0.0, 0.0), |s| (s.width, s.height))
    }

    fn with_drag<R>(&self, id: &str, f: impl FnOnce(&mut Option<Drag2>) -> R) -> R {
        let mut slots = self.slots.borrow_mut();
        f(&mut slots.entry(id.to_string()).or_default().drag)
    }

    pub(crate) fn moving(&self) -> bool {
        self.slots.borrow().values().any(|s| s.moving)
    }

    /// Steps every running transition to `now` over `length`; says whether any progress changed,
    /// and whether any still runs.
    fn step(&self, now: f64, length: f64) -> (bool, bool) {
        let (mut changed, mut running) = (false, false);
        for slot in self.slots.borrow_mut().values_mut().filter(|s| s.moving) {
            let began = *slot.began.get_or_insert(now);
            let progress = if length <= 0.0 {
                1.0
            } else {
                (((now - began) / length) as f32).min(1.0)
            };
            changed |= progress != slot.progress;
            slot.progress = progress;
            if progress >= 1.0 {
                slot.moving = false;
            } else {
                running = true;
            }
        }
        (changed, running)
    }
}

/// The host's step of every running transition at `now`: the next frame while one runs, and
/// whether a progress changed, so the view runs again only then (checklist item 35).
pub(crate) fn advance<T>(hosted: &mut Hosted<T>, now: f64, tx: &mut Tx) -> bool {
    if !hosted.ui.charts.moving() {
        return false;
    }
    let paint = style::paint(hosted);
    let length = paint
        .tokens
        .foundation()
        .motion
        .transition(MotionDuration::Normal)
        .as_secs_f64();
    let (changed, running) = hosted.ui.charts.step(now, length);
    if running {
        tx.wake_after(Duration::ZERO);
    }
    changed
}

impl<T: 'static> Chart<T> {
    pub fn show(self, hosted: &Hosted<T>) -> El {
        let paint = style::paint(hosted);
        let style = paint.tokens.chart();
        let foundation = paint.tokens.foundation();
        let accessible = self.accessible();
        // A transition has no length when motion is reduced, and is done at once.
        let reduced = foundation
            .motion
            .transition(MotionDuration::Normal)
            .is_zero();
        let (width, height, raw) = hosted.ui.charts.begin(&self.id, self.generation, reduced);
        let eased = foundation.motion.easing.apply(raw);
        let measure_id = self.id.clone();
        let boxed = apply(el().key(self.id.clone()), &accessible)
            .clip_rounded(0.0)
            .on_layout(move |hosted: &mut Hosted<T>, rect: [f32; 4]| {
                let (w, h) = hosted.ui.charts.size(&measure_id);
                if (w - rect[2]).abs() > 0.5 || (h - rect[3]).abs() > 0.5 {
                    hosted.ui.charts.measure(&measure_id, rect[2], rect[3]);
                }
            });
        let mut chart = match self.height {
            Some(h) => boxed.w_full().h(h.points(&style)),
            None => boxed.w_full().h_full(),
        };
        if width > 0.0
            && height > 0.0
            && let Some(draw) = &self.paint
        {
            let mut scene = Scene {
                width,
                height,
                style,
                marks: Vec::new(),
            };
            draw(&mut scene, eased);
            for layer in layers(&scene.marks, &paint, &style, width, height) {
                chart = chart.child(layer);
            }
        }
        if !self.interactive() {
            return chart;
        }
        self.interact(chart, &style, &paint)
    }
}

/// The marks as Rune elements, in the order they were drawn: a run of fills and strokes is one
/// canvas, and a text is an element of its own, so each lies over what was drawn before it.
fn layers(marks: &[Mark], paint: &Paint, style: &ChartStyle, width: f32, height: f32) -> Vec<El> {
    let mut out = Vec::new();
    let mut shape = Shape::new();
    let mut open = false;
    let flush = |shape: &mut Shape, open: &mut bool, out: &mut Vec<El>| {
        if *open {
            let done = std::mem::replace(shape, Shape::new());
            out.push(
                canvas(done)
                    .absolute(0.0, 0.0)
                    .size(width, height)
                    .a11y_hidden(),
            );
            *open = false;
        }
    };
    for mark in marks {
        match mark {
            Mark::Fill(path, colour) => {
                if let Some(path) = path.to_rune() {
                    shape = shape.fill_path(&path, ink(*colour));
                    open = true;
                }
            }
            Mark::Stroke(path, w, colour) => {
                if let Some(path) = path.to_rune() {
                    shape = shape.stroke_path(&path, *w, LineCap::Round, ink(*colour));
                    open = true;
                }
            }
            Mark::Text {
                x,
                y,
                text,
                colour,
                anchor,
                baseline,
                halo,
            } => {
                flush(&mut shape, &mut open, &mut out);
                if *halo {
                    let rim = style.thin;
                    for (ox, oy) in [(-rim, 0.0), (rim, 0.0), (0.0, -rim), (0.0, rim)] {
                        out.push(text_at(
                            paint,
                            style,
                            text,
                            (*x + ox, *y + oy),
                            style.canvas,
                            *anchor,
                            *baseline,
                            width,
                        ));
                    }
                }
                out.push(text_at(
                    paint,
                    style,
                    text,
                    (*x, *y),
                    *colour,
                    *anchor,
                    *baseline,
                    width,
                ));
            }
        }
    }
    flush(&mut shape, &mut open, &mut out);
    out
}

/// `text` placed by `anchor` and `baseline` at `at`. The text's width is not known when the view
/// is built, so it sits in a row that spans the box and is pushed to its start, middle or end.
#[allow(clippy::too_many_arguments)]
fn text_at(
    paint: &Paint,
    style: &ChartStyle,
    text: &str,
    at: (f32, f32),
    colour: ThemeColour,
    anchor: TextAnchor,
    baseline: TextBaseline,
    span: f32,
) -> El {
    let line = paint.role(style.type_role).line_height;
    let top = at.1
        - match baseline {
            TextBaseline::Top => 0.0,
            TextBaseline::Middle => line / 2.0,
            TextBaseline::Bottom => line,
        };
    let words = plain(paint, text.to_owned(), style.type_role, colour);
    let row = rune::row().a11y_hidden();
    match anchor {
        TextAnchor::Start => row.justify_start().w(span).h(line).absolute(at.0, top),
        TextAnchor::Middle => row
            .justify_center()
            .w(2.0 * span)
            .h(line)
            .absolute(at.0 - span, top),
        TextAnchor::End => row.justify_end().w(span).h(line).absolute(at.0 - span, top),
    }
    .child(words)
}

impl<T: 'static> Chart<T> {
    /// The chart as a tab stop that hears the pointer and the keys.
    fn interact(self, chart: El, style: &ChartStyle, paint: &Paint) -> El {
        let Chart {
            id,
            on_press,
            on_pan,
            on_zoom,
            ..
        } = self;
        let line = paint
            .role(odk_theme::foundation::TypeRole::Body)
            .line_height;
        let (drag_id, wheel_id, magnify_id, key_id) =
            (id.clone(), id.clone(), id.clone(), id.clone());
        let (drag_press, drag_pan) = (on_press.clone(), on_pan.clone());
        let (wheel_pan, wheel_zoom) = (on_pan.clone(), on_zoom.clone());
        let magnify_zoom = on_zoom.clone();
        let (key_press, key_pan, key_zoom) = (on_press, on_pan, on_zoom);
        focus_ring(chart, &style.focus)
            .focusable(true)
            .on_drag(move |hosted: &mut Hosted<T>, drag: &mut Drag, _: &mut Tx| {
                match drag.phase {
                    Phase::Down => {
                        if drag_pan.is_some() {
                            hosted
                                .ui
                                .charts
                                .with_drag(&drag_id, |d| *d = Some(Drag2::default()));
                        }
                        if let Some(press) = &drag_press {
                            press(&mut hosted.app, drag.x, drag.y);
                        }
                    }
                    Phase::Move => {
                        let Some(pan) = &drag_pan else { return };
                        if drag.cancelled() {
                            return;
                        }
                        let moved = hosted.ui.charts.with_drag(&drag_id, |d| {
                            let state = d.as_mut()?;
                            let delta = (drag.dx - state.last.0, drag.dy - state.last.1);
                            state.last = (drag.dx, drag.dy);
                            state.panned = (state.panned.0 + delta.0, state.panned.1 + delta.1);
                            Some(delta)
                        });
                        if let Some((dx, dy)) = moved {
                            pan(&mut hosted.app, dx, dy);
                        }
                    }
                    Phase::Up => {
                        let ended = hosted.ui.charts.with_drag(&drag_id, Option::take);
                        // Escape pans back by what the drag moved, and ends it.
                        if let (Some(state), Some(pan), true) = (ended, &drag_pan, drag.cancelled())
                        {
                            pan(&mut hosted.app, -state.panned.0, -state.panned.1);
                        }
                    }
                }
            })
            .on_wheel(move |hosted: &mut Hosted<T>, ev: WheelEvent, _: &mut Tx| {
                if ev.dy == 0.0 {
                    return false;
                }
                if ev.mods.shift {
                    let Some(pan) = &wheel_pan else { return false };
                    pan(&mut hosted.app, 0.0, ev.dy);
                    return true;
                }
                let Some(zoom) = &wheel_zoom else {
                    return false;
                };
                // A notch is a line of body text: the wheel up, content moving down, zooms in.
                let _ = &wheel_id;
                zoom(&mut hosted.app, WHEEL_ZOOM.powf(ev.dy / line), ev.x, ev.y);
                true
            })
            .on_magnify(
                move |hosted: &mut Hosted<T>, ev: MagnifyEvent, _: &mut Tx| {
                    let _ = &magnify_id;
                    let Some(zoom) = &magnify_zoom else {
                        return false;
                    };
                    zoom(&mut hosted.app, 1.0 + ev.delta, ev.x, ev.y);
                    true
                },
            )
            .on_key(move |hosted: &mut Hosted<T>, ev: &KeyEvent, _: &mut Tx| {
                if ev.mods.ctrl || ev.mods.alt || ev.mods.cmd || ev.mods.logo {
                    return false;
                }
                let (w, h) = hosted.ui.charts.size(&key_id);
                let (mx, my) = (w / 2.0, h / 2.0);
                let pan = match &ev.key {
                    Key::Left => Some((-w * KEY_PAN, 0.0)),
                    Key::Right => Some((w * KEY_PAN, 0.0)),
                    Key::Up => Some((0.0, -h * KEY_PAN)),
                    Key::Down => Some((0.0, h * KEY_PAN)),
                    _ => None,
                };
                if let Some((dx, dy)) = pan {
                    return key_pan.as_ref().is_some_and(|f| {
                        f(&mut hosted.app, dx, dy);
                        true
                    });
                }
                match &ev.key {
                    Key::Text(t) if t == "+" || t == "=" => key_zoom.as_ref().is_some_and(|f| {
                        f(&mut hosted.app, KEY_ZOOM, mx, my);
                        true
                    }),
                    Key::Text(t) if t == "-" => key_zoom.as_ref().is_some_and(|f| {
                        f(&mut hosted.app, 1.0 / KEY_ZOOM, mx, my);
                        true
                    }),
                    Key::Enter => key_press.as_ref().is_some_and(|f| {
                        f(&mut hosted.app, mx, my);
                        true
                    }),
                    Key::Text(t) if t == " " => key_press.as_ref().is_some_and(|f| {
                        f(&mut hosted.app, mx, my);
                        true
                    }),
                    _ => false,
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f32, b: f32, tolerance: f32) -> bool {
        (a - b).abs() <= tolerance
    }

    /// The point of the cubic `p0, c1, c2, p1` at `t`.
    fn at(p0: [f32; 2], c1: [f32; 2], c2: [f32; 2], p1: [f32; 2], t: f32) -> [f32; 2] {
        let u = 1.0 - t;
        let f = |i: usize| {
            u * u * u * p0[i]
                + 3.0 * u * u * t * c1[i]
                + 3.0 * u * t * t * c2[i]
                + t * t * t * p1[i]
        };
        [f(0), f(1)]
    }

    #[test]
    fn an_arc_is_cubics_of_at_most_a_quarter_turn_that_stay_on_the_circle() {
        let (cx, cy, r) = (30.0, 40.0, 25.0);
        for (from, to, pieces) in [
            (0.0, 1.0, 1),
            (0.0, PI / 2.0, 1),
            (0.0, PI, 2),
            (0.5, 0.5 + 3.0, 2),
            (0.0, TAU, 4),
            (0.0, 3.0 * TAU, 4),
            (1.0, -1.0, 2),
        ] {
            let cubics = arc_cubics(cx, cy, r, from, to);
            assert_eq!(cubics.len(), pieces, "{from} to {to}");
            let mut start = [cx + r * from.cos(), cy + r * from.sin()];
            for (c1, c2, end) in &cubics {
                for step in 1..8 {
                    let p = at(start, *c1, *c2, *end, step as f32 / 8.0);
                    let distance = ((p[0] - cx).powi(2) + (p[1] - cy).powi(2)).sqrt();
                    assert!(near(distance, r, r * 0.001), "{from} to {to}: {distance}");
                }
                start = *end;
            }
            let last = (to - from).clamp(-TAU, TAU) + from;
            assert!(near(start[0], cx + r * last.cos(), 1e-3));
            assert!(near(start[1], cy + r * last.sin(), 1e-3));
        }
    }

    #[test]
    fn an_empty_or_unusable_path_draws_nothing() {
        assert!(ChartPath::new().to_rune().is_none());
        assert!(ChartPath::new().close().to_rune().is_none());
        assert!(
            ChartPath::new()
                .curve_to(1.0, 1.0, 2.0, 2.0, 3.0, 3.0)
                .to_rune()
                .is_none()
        );
        assert!(ChartPath::new().line_to(f32::NAN, 1.0).to_rune().is_none());
        assert!(
            ChartPath::new()
                .arc(0.0, 0.0, 0.0, 0.0, 1.0)
                .to_rune()
                .is_none()
        );
        assert!(ChartPath::new().move_to(1.0, 2.0).to_rune().is_some());
    }

    #[test]
    fn a_ink_beyond_the_ramp_or_the_series_wraps_or_clamps() {
        let style = ThemeSnapshot_style();
        assert_eq!(
            Ink::Series(0).resolve(&style),
            Ink::Series(SERIES_ROLES.len()).resolve(&style)
        );
        assert_eq!(
            Ink::Heat(HEAT_STEPS - 1).resolve(&style),
            Ink::Heat(99).resolve(&style)
        );
    }

    #[allow(non_snake_case)]
    fn ThemeSnapshot_style() -> ChartStyle {
        let snapshot = odk_theme::ThemeSnapshot::emergency();
        crate::style::paint_for(
            &snapshot,
            &odk_theme::foundation::FoundationConfig::default(),
        )
        .tokens
        .chart()
    }
}
