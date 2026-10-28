//! The Data page: components that lay out and show an application's data. A resizable group of
//! panels, in a row and in a column, whose handles move with the pointer or the arrow keys
//! (ADR 0069), and what it reports when a resize ends; a data table of a few hundred processes
//! that draws only the rows in view (ADR 0070), sorts, selects and has a menu per row.

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
