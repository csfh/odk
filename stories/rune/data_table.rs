//! The DataTable story on Rune: sortable headers, one selected row, a thousand rows of which only
//! those in view are built, a row menu, and a button per row. Run it with
//! `tools/story rune-data_table`.

use odk_view::{Column, DataTable, Hosted, Layout, MenuItem, SortDirection, Space};
use rune::{Cx, El};

#[path = "support.rs"]
mod support;
use support::{caption, page, section};

struct Process {
    pid: u32,
    name: String,
    cpu: f64,
}

struct Story {
    rows: Vec<Process>,
    last: String,
}

impl Default for Story {
    fn default() -> Self {
        let names = [
            "init", "odkd", "hyprland", "waybar", "firefox", "kitty", "nvim", "cargo",
        ];
        let rows = (0..1000)
            .map(|i| Process {
                pid: 100 + i,
                name: format!("{}-{i:03}", names[(i as usize) % names.len()]),
                cpu: f64::from((i * 37) % 1000) / 10.0,
            })
            .collect();
        Self {
            rows,
            last: "Click a row, press Enter on it, or open its menu with a right click.".into(),
        }
    }
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let mut layers = Vec::new();
    let table = DataTable::<Story, Process>::new(
        "processes",
        "Processes",
        |s: &Story| s.rows.as_slice(),
        |p: &Process| p.pid.to_string(),
    )
    .column(
        Column::new("name", "Name", |p: &Process| p.name.clone())
            .grow(2.0)
            .sort_by(|p: &Process| p.name.clone()),
    )
    .column(
        Column::new("cpu", "CPU", |p: &Process| format!("{:.1}%", p.cpu))
            .chars(8)
            .align_end()
            .sort_by(|p: &Process| p.cpu),
    )
    .column(
        Column::new("pid", "PID", |p: &Process| p.pid.to_string())
            .chars(6)
            .align_end(),
    )
    .column(Column::action(
        "end",
        "",
        |_: &Process| Some("End".to_string()),
        |s: &mut Story, key: &str| s.last = format!("Pressed End on {key}."),
    ))
    .default_sort("cpu", SortDirection::Descending)
    .on_select(|s: &mut Story, key: &str| s.last = format!("Selected {key}."))
    .on_activate(|s: &mut Story, key: &str| s.last = format!("Activated {key}."))
    .row_menu(|p: &Process, menu| {
        let name = p.name.clone();
        menu.item(
            MenuItem::new("End process")
                .danger()
                .on_select(move |s: &mut Story| s.last = format!("Ended {name}.")),
        )
    })
    .height(420.0)
    .show(hosted, &mut layers);
    page(
        hosted,
        cx,
        [section(
            hosted,
            "Processes",
            "A thousand rows; Tab reaches the headers and then the rows. Up, Down, Home, End and Page keys move.",
            layout
                .stack(Space::Sm)
                .child(table)
                .child(caption(hosted, &hosted.app.last)),
        )],
        layers,
    )
}

fn main() {
    support::run(Story::default(), view);
}
