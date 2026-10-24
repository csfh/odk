//! Spike: does gpui 0.2.2 build here and open a Wayland window? (docs/checks.md, ADR 0029)
//! Prints when the first frame is rendered, then quits after two seconds.

use std::time::Duration;

use gpui::{
    App, Application, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};

struct Spike;

impl Render for Spike {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        println!("rendered a frame");
        div().flex().size_full().justify_center().items_center().bg(rgb(0x1a1b26)).text_color(rgb(0xc0caf5)).child("gpui 0.2.2")
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(300.), px(120.)), cx);
        cx.open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, |_, cx| cx.new(|_| Spike))
            .unwrap();
        cx.activate(true);
        cx.spawn(async move |cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            cx.update(|cx| cx.quit()).ok();
        })
        .detach();
    });
}
