//! The smallest ODK window application: a heading, a counter and a button.

use odk::prelude::*;

struct Hello {
    clicks: usize,
}

impl Render for Hello {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        root()
            .gap(Space::Md)
            .p(Space::Lg)
            .child(Heading::new("Hello, Omarchy").level(1))
            .child(Text::new(format!("Clicked {} times", self.clicks)).muted())
            .child(
                Button::new("click")
                    .label("Click me")
                    .variant(ButtonVariant::Primary)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.clicks += 1;
                        cx.notify();
                    })),
            )
    }
}

fn main() {
    odk::ui::run(|cx| {
        let window = WindowConfig::new("com.example.hello")
            .title("Hello")
            .size(360.0, 240.0);
        odk::ui::open_window(cx, window, |_, cx| cx.new(|_| Hello { clicks: 0 })).unwrap();
    });
}
