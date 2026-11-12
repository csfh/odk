//! What a component costs to build and to build again, on Rune's headless runtime (checklist
//! item 36, `docs/roadmap.md` Track A, work package 10). No harness crate: it times with
//! `Instant` and prints a table, so it adds no dependency.
//!
//! `cargo bench -p odk-view` runs every sample of `tests/common`; `cargo bench -p odk-view --
//! Button` runs those whose names contain `Button`. Two figures for each, the least of several
//! runs, in microseconds:
//!
//! - **build**: a window with the sample in it, built and laid out from scratch (the first frame).
//! - **rebuild**: the same window drawn again after something changed, which is what a keystroke
//!   or a hover costs the view. Rune draws no pixels here, so this is the cost of the tree and not
//!   of the GPU; `tools/idle-cpu` measures a running story.
//!
//! The numbers are a floor to compare one change with the last on one machine, not a promise
//! about another. `DRAW_BUDGET_MS` (default 4) marks a row that is over with `!`.

#[path = "../tests/common/mod.rs"]
mod common;

use std::time::{Duration, Instant};

use common::{SAMPLES, first_frame, open};
use odk_theme::ThemeSnapshot;
use odk_view::{Button, Hosted, frame, headless};
use rune::{Cx, El, col};

const SIZE: (f32, f32) = (480.0, 360.0);
const RUNS: usize = 30;

fn budget() -> Duration {
    std::env::var("DRAW_BUDGET_MS")
        .ok()
        .and_then(|ms| ms.parse().ok())
        .map_or(Duration::from_millis(4), Duration::from_millis)
}

/// The least time of `runs` runs of `f`.
fn least(runs: usize, mut f: impl FnMut()) -> Duration {
    (0..runs)
        .map(|_| {
            let started = Instant::now();
            f();
            started.elapsed()
        })
        .min()
        .expect("at least one run")
}

fn micros(d: Duration) -> f64 {
    d.as_secs_f64() * 1e6
}

fn thousand(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    let mut list = col().key("thousand").gap(4.0);
    for n in 0..1000 {
        list = list.child(
            Button::new(format!("b{n}"), format!("Button {n}"), |_: &mut ()| {}).show(hosted),
        );
    }
    frame(hosted, cx, list, [])
}

fn main() {
    // `cargo bench` passes `--bench`; anything else that is not a flag filters by name.
    let filter = std::env::args()
        .skip(1)
        .find(|a| !a.starts_with('-'))
        .unwrap_or_default();
    let limit = budget();
    println!(
        "{:<18} {:>12} {:>12}   (least of {RUNS}, microseconds; ! is over {limit:?})",
        "component", "build", "rebuild"
    );
    let mut over = 0;
    for sample in SAMPLES.iter().filter(|s| s.name.contains(&filter)) {
        let build = first_frame(sample, "Save", SIZE, RUNS);
        let mut runtime = open(sample, "Save", SIZE, None);
        let rebuild = least(RUNS, || {
            runtime.invalidate();
            runtime.frame(1.0 / 60.0);
        });
        let mark = if build > limit || rebuild > limit {
            over += 1;
            "!"
        } else {
            ""
        };
        println!(
            "{:<18} {:>12.0} {:>12.0} {mark}",
            sample.name,
            micros(build),
            micros(rebuild)
        );
    }
    if filter.is_empty() {
        let build = least(5, || {
            drop(headless(
                Hosted::new((), ThemeSnapshot::emergency(), common::face()),
                thousand,
                SIZE,
            ));
        });
        println!("{:<18} {:>12.0}", "1000 buttons", micros(build));
    }
    if over > 0 {
        println!("{over} over the budget");
    }
}
