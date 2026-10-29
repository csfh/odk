//! The Data page: components that lay out and show an application's data. A resizable group of
//! panels, in a row and in a column, whose handles move with the pointer or the arrow keys
//! (ADR 0069), and what it reports when a resize ends; a data table of a few hundred processes
//! that draws only the rows in view (ADR 0070), sorts, selects and has a menu per row; and
//! charts drawn on ODK's canvas in named inks (ADR 0071): series that move between samples, a
//! heat grid, a donut with labels, and a view that pans and zooms; and images drawn from bytes
//! (ADR 0072): an avatar, a preview cropped and shown whole, one still waiting and one broken.

use std::f32::consts::TAU;
use std::rc::Rc;
use std::sync::Arc;

use odk::gpui::{Div, px};
use odk::prelude::*;

use crate::buttons::caption;
use crate::content::Content;
use crate::{Page, Settings};

/// How tall each resizable group is drawn here: a group fills its parent, so the page gives it
/// a height of its own.
const GROUP_HEIGHT: f32 = 220.0;

/// How many processes the table shows: enough that only a few are in view at once.
const PROCESSES: u32 = 200;

/// How tall the table is drawn here.
const TABLE_HEIGHT: f32 = 300.0;

/// A process as the table shows it.
#[derive(Clone)]
pub struct Process {
    pid: u32,
    name: SharedString,
    user: &'static str,
    cpu: f64,
    memory: u64,
    depth: usize,
}

/// The processes the table shows, made up, with the worst case's long names on some.
fn processes(c: &Content) -> Vec<Process> {
    let names = c.processes;
    (0..PROCESSES)
        .map(|i| Process {
            pid: 100 + i * 7,
            name: format!("{}-{i}", names[i as usize % names.len()]).into(),
            user: if i % 5 == 0 { "root" } else { "me" },
            cpu: f64::from((i * 37) % 997) / 10.0,
            memory: u64::from((i * 7919) % 4096) * 1024,
            depth: (i % 3) as usize,
        })
        .collect()
}

/// The Data page: its own state, and the Content switch it follows.
pub struct DataPage {
    worst_case: bool,
    /// What the row's `on_layout` last reported, as whole per cent.
    layout: Option<Vec<u32>>,
    /// What the table last reported: the selected process, the one activated, and an action.
    selected: Option<SharedString>,
    said: Option<String>,
    empty: bool,
    loading: bool,
    /// The charts' sample: its number, and the series it moves from and to.
    sample: u64,
    from: Rc<Vec<[f32; SERIES]>>,
    to: Rc<Vec<[f32; SERIES]>>,
    /// The pannable view: its offset and scale, and the last press in it.
    view: (f32, f32, f32),
    pressed: Option<(f32, f32)>,
    /// The pictures' bytes, kept so each is made into an image once (ADR 0072).
    landscape: Arc<[u8]>,
    portrait: Arc<[u8]>,
}

/// How many series the line chart has, and how many points each.
const SERIES: usize = 3;
const POINTS: usize = 48;
/// How tall each chart is drawn here.
const CHART_HEIGHT: f32 = 160.0;

/// A made-up sample of each series, between 0 and 1, the same for the same number.
fn sample(n: u64) -> Vec<[f32; SERIES]> {
    let mut seed = n
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    let mut next = move || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((seed >> 33) % 1000) as f32 / 1000.0
    };
    (0..POINTS)
        .map(|i| {
            let wave = (i as f32 / 6.0 + n as f32).sin() * 0.25 + 0.5;
            [
                (wave + next() * 0.2 - 0.1).clamp(0.0, 1.0),
                (0.3 + next() * 0.3).clamp(0.0, 1.0),
                (1.0 - wave) * 0.6 + next() * 0.1,
            ]
        })
        .collect()
}

impl DataPage {
    pub fn new(settings: &Entity<Settings>, cx: &mut Context<Self>) -> Self {
        cx.observe(settings, |this, settings, cx| {
            this.worst_case = settings.read(cx).worst_case;
            cx.notify();
        })
        .detach();
        Self {
            worst_case: settings.read(cx).worst_case,
            layout: None,
            selected: None,
            said: None,
            empty: false,
            loading: false,
            sample: 0,
            from: Rc::new(sample(0)),
            to: Rc::new(sample(0)),
            view: (0.0, 0.0, 1.0),
            pressed: None,
            landscape: LANDSCAPE.into(),
            portrait: PORTRAIT.into(),
        }
    }
}

impl Render for DataPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = Content::get(self.worst_case);
        crate::page(Page::Data, section(self, &c, cx))
    }
}

fn group(title: &'static str) -> impl IntoElement {
    Heading::new(title).level(3)
}

/// The section.
pub fn section(g: &mut DataPage, c: &Content, cx: &mut Context<DataPage>) -> Div {
    v_flex()
        .flex_shrink_0()
        .gap(Space::Lg)
        .child(group("Resizable"))
        .child(resizable(g, c, cx))
        .child(group("Data table"))
        .child(table(g, c, cx))
        .child(group("Charts"))
        .child(charts(g, cx))
        .child(group("Images"))
        .child(images(g))
        .child(group("Items"))
        .child(items(c))
}

/// A panel's content: a title over a paragraph, which wraps and is cut at the panel's edge.
fn panel_text(title: &'static str, body: &'static str) -> impl IntoElement {
    v_flex()
        .p(Space::Md)
        .gap(Space::Sm)
        .child(Text::new(title).single_line())
        .child(Text::new(body).muted())
}

fn framed(group: impl IntoElement) -> Div {
    v_flex().h(px(GROUP_HEIGHT)).w_full().child(group)
}

fn resizable(g: &mut DataPage, c: &Content, cx: &mut Context<DataPage>) -> Div {
    let [list, detail, log] = c.panels;
    let reported = match &g.layout {
        Some(sizes) => sizes
            .iter()
            .map(|s| format!("{s}%"))
            .collect::<Vec<_>>()
            .join(" · "),
        None => "nothing yet".into(),
    };
    v_flex()
        .gap(Space::Sm)
        .child(caption(
            "A row of three panels. Drag a handle, or Tab to it and use Left, Right, Home and End; the list keeps between 20% and 60%, the detail at least 30%.",
        ))
        .child(framed(
            Resizable::horizontal("data-row", "List, detail and log")
                .panel(
                    ResizablePanel::new()
                        .default_size(0.3)
                        .min_size(0.2)
                        .max_size(0.6)
                        .child(panel_text(list.0, list.1)),
                )
                .panel(
                    ResizablePanel::new()
                        .min_size(0.3)
                        .child(panel_text(detail.0, detail.1)),
                )
                .panel(
                    ResizablePanel::new()
                        .default_size(0.25)
                        .child(panel_text(log.0, log.1)),
                )
                .on_layout(cx.listener(|this, sizes: &[f32], _, cx| {
                    this.layout = Some(sizes.iter().map(|s| (s * 100.0).round() as u32).collect());
                    cx.notify();
                })),
        ))
        .child(caption(format!("Reported when a resize ends: {reported}")))
        .child(caption(
            "A column: Up and Down move the handle. Below it, a disabled group, whose handle neither moves nor takes focus.",
        ))
        .child(framed(
            Resizable::vertical("data-column", "Editor and terminal")
                .panel(
                    ResizablePanel::new()
                        .default_size(0.6)
                        .min_size(0.25)
                        .child(panel_text(detail.0, detail.1)),
                )
                .panel(
                    ResizablePanel::new()
                        .min_size(0.2)
                        .child(panel_text(log.0, log.1)),
                ),
        ))
        .child(framed(
            Resizable::horizontal("data-disabled", "Fixed split")
                .disabled(true)
                .panel(ResizablePanel::new().child(panel_text(list.0, list.1)))
                .panel(ResizablePanel::new().child(panel_text(detail.0, detail.1))),
        ))
}

fn table(g: &mut DataPage, c: &Content, cx: &mut Context<DataPage>) -> Div {
    let rows = if g.empty { Vec::new() } else { processes(c) };
    let selected = g
        .selected
        .as_ref()
        .map_or("none".into(), |s| format!("pid {s}"));
    let said = g.said.clone().unwrap_or_else(|| "nothing yet".into());
    let entity = cx.entity().downgrade();
    let say = move |what: String, cx: &mut odk::gpui::App| {
        let _ = entity.update(cx, |this, cx| {
            this.said = Some(what);
            cx.notify();
        });
    };
    let (end, menu_say, activate) = (say.clone(), say.clone(), say);
    v_flex()
        .gap(Space::Sm)
        .child(caption(
            "Two hundred processes, of which only those in view are drawn. Tab to the headers and press Enter to sort, then Tab to the rows: Up, Down, Home, End, Page Up and Page Down select, Enter activates, the Menu key or Shift+F10 opens the row's menu. A right-click selects a row and opens its menu; End takes the pointer only, and is in the menu too.",
        ))
        .child(
            h_flex()
                .gap(Space::Md)
                .child(
                    Switch::new("data-empty", "No rows")
                        .checked(g.empty)
                        .on_change({
                            let me = cx.entity().downgrade();
                            move |on, _, cx| {
                                let _ = me.update(cx, |this, cx| {
                                    this.empty = on;
                                    cx.notify();
                                });
                            }
                        }),
                )
                .child(
                    Switch::new("data-loading", "Loading")
                        .checked(g.loading)
                        .on_change({
                            let me = cx.entity().downgrade();
                            move |on, _, cx| {
                                let _ = me.update(cx, |this, cx| {
                                    this.loading = on;
                                    cx.notify();
                                });
                            }
                        }),
                ),
        )
        .child(
            v_flex().h(px(TABLE_HEIGHT)).w_full().child(
                DataTable::new("data-table", "Processes", rows)
                    .row_key(|p: &Process| p.pid.to_string().into())
                    .indent(|p: &Process| p.depth)
                    .column(
                        Column::new("name", "Name", |p: &Process| p.name.clone())
                            .grow(3.0)
                            .sort_by(|p: &Process| p.name.clone()),
                    )
                    .column(
                        Column::new("pid", "PID", |p: &Process| p.pid.to_string().into())
                            .chars(6)
                            .align_end()
                            .sort_by(|p: &Process| p.pid),
                    )
                    .column(
                        Column::cell("user", "User", |p: &Process, _, _| {
                            Badge::new(p.user).into_any_element()
                        })
                        .chars(6),
                    )
                    .column(
                        Column::new("cpu", "CPU", |p: &Process| format!("{:.1}%", p.cpu).into())
                            .chars(7)
                            .align_end()
                            .sort_by(|p: &Process| p.cpu),
                    )
                    .column(
                        Column::new("memory", "Memory", |p: &Process| {
                            format!("{} MiB", p.memory / 1024).into()
                        })
                        .chars(9)
                        .align_end()
                        .sort_by(|p: &Process| p.memory),
                    )
                    .column(Column::action(
                        "end",
                        "",
                        |p: &Process| (p.user != "root").then(|| "End".into()),
                        move |p: &Process, _, cx| end(format!("End pressed on pid {}", p.pid), cx),
                    ))
                    .default_sort("cpu", SortDirection::Descending)
                    .loading(g.loading)
                    .empty("No process matches")
                    .on_select(cx.listener(|this, key: &SharedString, _, cx| {
                        this.selected = Some(key.clone());
                        cx.notify();
                    }))
                    .on_activate(move |key, _, cx| activate(format!("Activated pid {key}"), cx))
                    .row_menu(move |p: &Process, menu| {
                        let (pid, say) = (p.pid, menu_say.clone());
                        menu.item(
                            MenuItem::new("End")
                                .danger()
                                .disabled(p.user == "root")
                                .on_select(move |_, cx| say(format!("End chosen for pid {pid}"), cx)),
                        )
                        .item(MenuItem::new("Copy pid").hint("Ctrl+C"))
                    }),
            ),
        )
        .child(caption(format!("Selected: {selected}. Last: {said}.")))
}

fn chart_box(chart: Chart) -> Div {
    v_flex().h(px(CHART_HEIGHT)).w_full().child(chart)
}

fn charts(g: &mut DataPage, cx: &mut Context<DataPage>) -> Div {
    let (from, to) = (g.from.clone(), g.to.clone());
    let lines = Chart::new("data-lines", "Three series over the last samples")
        .description("CPU, memory and network, between 0 and 100%")
        .transition(g.sample)
        .paint(move |scene, t| {
            let (w, h) = (scene.width(), scene.height());
            for y in [0.25, 0.5, 0.75] {
                let rule = ChartPath::new().move_to(0.0, h * y).line_to(w, h * y);
                scene.stroke(&rule, Ink::Grid, StrokeWidth::Thin);
            }
            for s in 0..SERIES {
                let mut path = ChartPath::new();
                for i in 0..POINTS {
                    let v = from[i][s] + (to[i][s] - from[i][s]) * t;
                    let (x, y) = (w * i as f32 / (POINTS - 1) as f32, h * (1.0 - v));
                    path = if i == 0 {
                        path.move_to(x, y)
                    } else {
                        path.line_to(x, y)
                    };
                }
                if s == 0 {
                    let area = path.clone().line_to(w, h).line_to(0.0, h).close();
                    scene.fill(&area, Ink::SeriesFill(0));
                }
                scene.stroke(&path, Ink::Series(s), StrokeWidth::Regular);
            }
            for (s, label) in ["CPU", "Memory", "Network"].iter().enumerate() {
                let y = 4.0 + 16.0 * s as f32;
                scene.fill_rect(4.0, y + 4.0, 8.0, 8.0, Ink::Series(s));
                scene.text(
                    16.0,
                    y,
                    *label,
                    Ink::Text,
                    TextAnchor::Start,
                    TextBaseline::Top,
                    true,
                );
            }
            scene.stroke_rect(0.0, 0.0, w, h, Ink::Axis, StrokeWidth::Thin);
        });

    let heat_from = g.to.clone();
    let heat = Chart::new("data-heat", "Load per core")
        .description("Sixteen cores over twelve samples, darker is busier")
        .paint(move |scene, _| {
            let (cols, rows) = (12usize, 8usize);
            let (w, h) = (scene.width() / cols as f32, scene.height() / rows as f32);
            for r in 0..rows {
                for c in 0..cols {
                    let v = heat_from[(r * 5 + c) % POINTS][r % SERIES];
                    let level = (v * 5.0).floor() as usize;
                    scene.fill_rect(
                        c as f32 * w,
                        r as f32 * h,
                        w - 1.0,
                        h - 1.0,
                        Ink::Heat(level),
                    );
                }
            }
        });

    let donut_to = g.to.clone();
    let donut = Chart::new("data-donut", "Memory by kind")
        .description("Applications, cache and free memory")
        .paint(move |scene, _| {
            let (cx, cy) = (scene.width() / 2.0, scene.height() / 2.0);
            let r = cx.min(cy) - 8.0;
            let parts = [donut_to[0][0] + 0.2, donut_to[0][1] + 0.2, 0.4];
            let total: f32 = parts.iter().sum();
            let mut at = -TAU / 4.0;
            for (n, part) in parts.iter().enumerate() {
                let sweep = TAU * part / total;
                let slice = ChartPath::new()
                    .arc(cx, cy, r, at, at + sweep)
                    .arc(cx, cy, r * 0.6, at + sweep, at)
                    .close();
                scene.fill(&slice, Ink::Series(n));
                let mid = at + sweep / 2.0;
                let (lx, ly) = (cx + r * 0.8 * mid.cos(), cy + r * 0.8 * mid.sin());
                let label = format!("{:.0}%", 100.0 * part / total);
                scene.text(
                    lx,
                    ly,
                    label,
                    Ink::Text,
                    TextAnchor::Middle,
                    TextBaseline::Middle,
                    true,
                );
                at += sweep;
            }
            let ring = ChartPath::new().arc(cx, cy, r, 0.0, TAU);
            scene.stroke(&ring, Ink::Canvas, StrokeWidth::Thin);
        });

    let (ox, oy, zoom) = g.view;
    let me = cx.entity().downgrade();
    let (pan_me, zoom_me) = (me.clone(), me.clone());
    let view = Chart::new("data-view", "A view that pans and zooms")
        .description("A grid of boxes; arrows pan, plus and minus zoom")
        .paint(move |scene, _| {
            let size = 24.0 * zoom;
            for r in 0..12 {
                for c in 0..24 {
                    let (x, y) = (ox + c as f32 * size, oy + r as f32 * size);
                    let ink = Ink::Heat((r + c) % 5);
                    scene.fill_rect(x, y, size - 1.0, size - 1.0, ink);
                }
            }
        })
        .on_press(move |x, y, _, cx| {
            let _ = me.update(cx, |this, cx| {
                this.pressed = Some((x, y));
                cx.notify();
            });
        })
        .on_pan(move |dx, dy, _, cx| {
            let _ = pan_me.update(cx, |this, cx| {
                this.view.0 += dx;
                this.view.1 += dy;
                cx.notify();
            });
        })
        .on_zoom(move |factor, x, y, _, cx| {
            let _ = zoom_me.update(cx, |this, cx| {
                let (ox, oy, z) = this.view;
                let next = (z * factor).clamp(0.25, 8.0);
                let k = next / z;
                this.view = (x - (x - ox) * k, y - (y - oy) * k, next);
                cx.notify();
            });
        });
    let pressed = g
        .pressed
        .map_or("nowhere yet".into(), |(x, y)| format!("at {x:.0}, {y:.0}"));
    v_flex()
        .gap(Space::Sm)
        .child(caption(
            "ODK draws no chart kinds: each is drawn on a canvas in named inks (series, tints, a heat ramp, text, grid, axis), so every colour follows the theme. Next sample moves the series over the foundation's normal.",
        ))
        .child(
            Button::new("data-next")
                .label("Next sample")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.sample += 1;
                    this.from = this.to.clone();
                    this.to = Rc::new(sample(this.sample));
                    cx.notify();
                })),
        )
        .child(chart_box(lines))
        .child(
            h_flex()
                .gap(Space::Md)
                .child(v_flex().flex_1().child(chart_box(heat)))
                .child(v_flex().flex_1().child(chart_box(donut))),
        )
        .child(caption(format!(
            "Drag, Shift and the wheel, or the arrow keys pan; the wheel or + and - zoom; Enter presses the middle. Pressed {pressed}."
        )))
        .child(chart_box(view))
}

/// How wide the column of items is here, as a settings page's column might be.
const ITEMS_WIDTH: f32 = 640.0;

/// Items as a settings page draws its rows: each with what it controls at its end, or across it
/// when the control needs the room, divided by separators.
fn items(c: &Content) -> Div {
    let [scroll, accel, speed] = c.items;
    let line = |text: &'static str| (!text.is_empty()).then(|| Text::new(text).mono().muted());
    v_flex()
        .gap(Space::Sm)
        .child(caption(
            "A title, a description and lines under them, actions at the end and a footer across. Padded above and below by the density and not at the sides, so its text lines up with the heading over it. Not a tab stop: Tab reaches its actions.",
        ))
        .child(
            v_flex()
                .w_full()
                .max_w(px(ITEMS_WIDTH))
                .child(
                    Item::new(scroll.0)
                        .description(scroll.1)
                        .children(line(scroll.2))
                        .action(Switch::new("data-item-switch", scroll.0).default_checked(true)),
                )
                .child(Separator::horizontal())
                .child(
                    Item::new(accel.0)
                        .description(accel.1)
                        .children(line(accel.2))
                        .action(
                            Select::new("data-item-select", accel.0)
                                .item("default", "Default")
                                .item("flat", "Flat")
                                .item("adaptive", "Adaptive")
                                .default_value("default"),
                        ),
                )
                .child(Separator::horizontal())
                .child(
                    Item::new(speed.0)
                        .description(speed.1)
                        .children(line(speed.2))
                        .footer(
                            NumberInput::new("data-item-number", speed.0)
                                .min(-1.0)
                                .max(1.0)
                                .step(0.1)
                                .default_value(-0.6),
                        ),
                )
                .child(Separator::horizontal())
                .child(
                    Item::new("Work")
                        .description("A profile with two actions at its end.")
                        .action(Button::new("data-item-apply").label("Apply"))
                        .action(
                            Button::icon("data-item-star", Icon::Star, "Add to favorites")
                                .variant(ButtonVariant::Ghost),
                        ),
                )
                .child(Separator::horizontal())
                .child(
                    Item::new("Keyboard layout")
                        .action(Badge::new("Unknown").tone(Tone::Warning).marked()),
                ),
        )
}

/// Two pictures made for the gallery: a landscape and a portrait.
const LANDSCAPE: &[u8] = include_bytes!("../assets/landscape.png");
const PORTRAIT: &[u8] = include_bytes!("../assets/portrait.png");

/// How large each image's box is here.
const PREVIEW: (f32, f32) = (240.0, 135.0);

fn images(g: &DataPage) -> Div {
    let preview = |name: &'static str, image: Image| {
        v_flex()
            .gap(Space::Xs)
            .child(v_flex().w(px(PREVIEW.0)).h(px(PREVIEW.1)).child(image))
            .child(caption(name))
    };
    v_flex()
        .gap(Space::Sm)
        .child(caption(
            "Drawn from bytes the application holds, never from a path, so a sandboxed window shows what its daemon read. An avatar is a circle the foundation's s12 across.",
        ))
        .child(
            h_flex()
                .gap(Space::Lg)
                .items_start()
                .child(Image::avatar("data-avatar", "Your avatar").source(g.portrait.clone()))
                .child(preview(
                    "Cropped to fill",
                    Image::new("data-cover", "Current background").source(g.landscape.clone()),
                ))
                .child(preview(
                    "Shown whole",
                    Image::new("data-contain", "Current background")
                        .fit(ImageFit::Contain)
                        .source(g.portrait.clone()),
                ))
                .child(preview(
                    "Waiting for its bytes",
                    Image::new("data-waiting", "Current background"),
                ))
                .child(preview(
                    "Bytes that are not an image",
                    Image::new("data-broken", "Current background").source(b"not an image".to_vec()),
                )),
        )
}
