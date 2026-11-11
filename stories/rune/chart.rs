//! The Chart story on Rune: a line, an area, bars and a donut drawn on the canvas in named inks,
//! text with anchors and a halo, the three named heights, a transition between two datasets, and a
//! chart that pans and zooms. Run it with `tools/story rune-chart`.

use std::f32::consts::TAU;

use odk_view::{
    Button, Chart, ChartHeight, ChartPath, Hosted, Ink, Layout, Scene, Space, StrokeWidth,
    TextAnchor, TextBaseline,
};
use rune::{Cx, El, col};

#[path = "support.rs"]
mod support;
use support::{caption, page, section};

/// The width of each column of the heights row, in points.
const HEIGHT_COLUMN: f32 = 180.0;

const SAMPLES: [[f32; 8]; 2] = [
    [0.2, 0.5, 0.35, 0.7, 0.55, 0.9, 0.6, 0.8],
    [0.8, 0.4, 0.6, 0.3, 0.45, 0.2, 0.5, 0.25],
];

struct Story {
    generation: u64,
    /// The view's offset and zoom, which a pan and a zoom change.
    pan: (f32, f32),
    zoom: f32,
    last: String,
}

impl Default for Story {
    fn default() -> Self {
        Self {
            generation: 0,
            pan: (0.0, 0.0),
            zoom: 1.0,
            last: "Drag, wheel, +, - and the arrow keys.".into(),
        }
    }
}

/// The value of sample `i` between the two datasets at `progress`.
fn value(from: usize, i: usize, progress: f32) -> f32 {
    let (a, b) = (SAMPLES[from][i], SAMPLES[1 - from][i]);
    a + (b - a) * progress
}

fn line_chart(scene: &mut Scene, from: usize, progress: f32) {
    let (w, h) = (scene.width(), scene.height());
    let (left, right, top, bottom) = (36.0, w - 12.0, 12.0, h - 22.0);
    for k in 0..=4 {
        let y = bottom - (bottom - top) * k as f32 / 4.0;
        scene.stroke(
            &ChartPath::new().move_to(left, y).line_to(right, y),
            Ink::Grid,
            StrokeWidth::Thin,
        );
        scene.text(
            left - 6.0,
            y,
            format!("{}", k * 25),
            Ink::TextMuted,
            TextAnchor::End,
            TextBaseline::Middle,
            false,
        );
    }
    let x = |i: usize| left + (right - left) * i as f32 / 7.0;
    let y = |v: f32| bottom - (bottom - top) * v;
    let mut area = ChartPath::new().move_to(x(0), bottom);
    let mut line = ChartPath::new();
    for i in 0..8 {
        let v = value(from, i, progress);
        area = area.line_to(x(i), y(v));
        line = if i == 0 {
            line.move_to(x(i), y(v))
        } else {
            let prev = value(from, i - 1, progress);
            let mid = (x(i - 1) + x(i)) / 2.0;
            line.curve_to(mid, y(prev), mid, y(v), x(i), y(v))
        };
    }
    scene.fill(&area.line_to(x(7), bottom).close(), Ink::SeriesFill(0));
    scene.stroke(&line, Ink::Series(0), StrokeWidth::Thick);
    let peak = (0..8)
        .max_by(|a, b| value(from, *a, progress).total_cmp(&value(from, *b, progress)))
        .unwrap_or(0);
    scene.text(
        x(peak),
        y(value(from, peak, progress)) - 6.0,
        "peak",
        Ink::Text,
        TextAnchor::Middle,
        TextBaseline::Bottom,
        true,
    );
}

fn bars(scene: &mut Scene) {
    let (w, h) = (scene.width(), scene.height());
    let gap = 8.0;
    let n = 6;
    let bar = (w - gap * (n as f32 + 1.0)) / n as f32;
    for i in 0..n {
        let v = 0.25 + 0.12 * ((i * 5) % 6) as f32;
        let x = gap + (bar + gap) * i as f32;
        let top = (h - 18.0) * (1.0 - v);
        scene.fill_rect(x, top, bar, h - 18.0 - top, Ink::Series(i));
        scene.stroke_rect(x, top, bar, h - 18.0 - top, Ink::Canvas, StrokeWidth::Thin);
        scene.text(
            x + bar / 2.0,
            h - 9.0,
            format!("{}", i + 1),
            Ink::TextMuted,
            TextAnchor::Middle,
            TextBaseline::Middle,
            false,
        );
    }
}

fn donut(scene: &mut Scene) {
    let (cx, cy) = (scene.width() / 2.0, scene.height() / 2.0);
    let (outer, inner) = (cy - 8.0, cy - 30.0);
    let shares = [0.4_f32, 0.25, 0.2, 0.1, 0.05];
    let mut at = -TAU / 4.0;
    for (i, share) in shares.iter().enumerate() {
        let to = at + TAU * share - 0.03;
        let sector = ChartPath::new()
            .arc(cx, cy, outer, at, to)
            .arc(cx, cy, inner, to, at)
            .close();
        scene.fill(&sector, Ink::Series(i));
        at += TAU * share;
    }
    scene.text(
        cx,
        cy,
        "100%",
        Ink::Text,
        TextAnchor::Middle,
        TextBaseline::Middle,
        false,
    );
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let story = &hosted.app;
    let generation = story.generation;
    let from = (generation % 2) as usize;
    // The transition runs from the dataset the last generation showed to the one it shows now.
    let transition = layout
        .stack(Space::Sm)
        .child(
            Chart::<Story>::new("line", "Requests over eight samples")
                .description("A line and an area over eight samples, between two datasets")
                .height(ChartHeight::Tall)
                .transition(generation)
                .paint(move |scene, progress| line_chart(scene, 1 - from, progress))
                .show(hosted),
        )
        .child(
            Button::new("swap", "Change the data", |story: &mut Story| {
                story.generation += 1
            })
            .show(hosted),
        );
    let marks = layout
        .row(Space::Md)
        .child(
            col().grow(2.0).min_w(0.0).child(
                Chart::<Story>::new("bars", "Six bars")
                    .description("Six bars, each its own series")
                    .height(ChartHeight::Regular)
                    .paint(|scene, _| bars(scene))
                    .show(hosted),
            ),
        )
        .child(
            col().grow(1.0).min_w(0.0).child(
                Chart::<Story>::new("donut", "A donut of five shares")
                    .description("Five shares of a whole, the largest 40%")
                    .height(ChartHeight::Regular)
                    .paint(|scene, _| donut(scene))
                    .show(hosted),
            ),
        );
    let heights = [
        ("Compact", ChartHeight::Compact),
        ("Regular", ChartHeight::Regular),
        ("Tall", ChartHeight::Tall),
    ]
    .into_iter()
    .fold(layout.row(Space::Md), |row, (name, height)| {
        row.child(
            layout
                .stack(Space::Xs)
                .w(HEIGHT_COLUMN)
                .child(
                    Chart::<Story>::new(format!("h-{name}"), name)
                        .height(height)
                        .paint(|scene, _| {
                            scene.stroke_rect(
                                0.0,
                                0.0,
                                scene.width(),
                                scene.height(),
                                Ink::Axis,
                                StrokeWidth::Thin,
                            );
                            scene.text(
                                scene.width() / 2.0,
                                scene.height() / 2.0,
                                format!("{:.0} x {:.0}", scene.width(), scene.height()),
                                Ink::Text,
                                TextAnchor::Middle,
                                TextBaseline::Middle,
                                false,
                            );
                        })
                        .show(hosted),
                )
                .child(caption(hosted, name)),
        )
    });
    let (pan, zoom) = (story.pan, story.zoom);
    let interactive = layout
        .stack(Space::Xs)
        .child(
            Chart::<Story>::new("pan", "A grid that pans and zooms")
                .description("A grid of squares that moves with a drag and the arrow keys")
                .height(ChartHeight::Regular)
                .paint(move |scene, _| {
                    let cell = 40.0 * zoom;
                    let (w, h) = (scene.width(), scene.height());
                    let (ox, oy) = (pan.0.rem_euclid(cell), pan.1.rem_euclid(cell));
                    let mut x = ox - cell;
                    while x < w {
                        scene.stroke(
                            &ChartPath::new().move_to(x, 0.0).line_to(x, h),
                            Ink::Grid,
                            StrokeWidth::Thin,
                        );
                        x += cell;
                    }
                    let mut y = oy - cell;
                    while y < h {
                        scene.stroke(
                            &ChartPath::new().move_to(0.0, y).line_to(w, y),
                            Ink::Grid,
                            StrokeWidth::Thin,
                        );
                        y += cell;
                    }
                    scene.text(
                        8.0,
                        8.0,
                        format!("zoom {zoom:.2}"),
                        Ink::Text,
                        TextAnchor::Start,
                        TextBaseline::Top,
                        true,
                    );
                })
                .on_press(|story: &mut Story, x, y| {
                    story.last = format!("Pressed at {x:.0}, {y:.0}.")
                })
                .on_pan(|story: &mut Story, dx, dy| {
                    story.pan = (story.pan.0 + dx, story.pan.1 + dy);
                    story.last = format!("Panned by {dx:.0}, {dy:.0}.");
                })
                .on_zoom(|story: &mut Story, factor, _, _| {
                    story.zoom = (story.zoom * factor).clamp(0.4, 4.0);
                    story.last = format!("Zoomed by {factor:.2}.");
                })
                .show(hosted),
        )
        .child(caption(hosted, &story.last));

    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "A line and an area",
                "Between two datasets: the chart hands paint a progress from 0 to 1 over `normal`.",
                transition,
            ),
            section(
                hosted,
                "Bars and a donut",
                "Rects, and arcs which Rune draws as cubic curves.",
                marks,
            ),
            section(
                hosted,
                "Heights",
                "Compact, Regular and Tall, as wide as the parent.",
                heights,
            ),
            section(
                hosted,
                "Pointer and keys",
                "A tab stop with a handler: drag to pan, wheel to zoom, Shift and the wheel to pan.",
                interactive,
            ),
        ],
        [],
    )
}

fn main() {
    support::run(Story::default(), view);
}
