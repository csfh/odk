// snippet start
use odk::prelude::*;

struct Notes {
    confirming: bool,
}

impl Render for Notes {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        div()
            .child(
                Button::new("discard")
                    .label("Discard…")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.confirming = true;
                        cx.notify();
                    })),
            )
            .child(
                Dialog::new("confirm", "Discard changes?")
                    .open(self.confirming)
                    .description("Your edits will be lost.")
                    .on_close(move |_, cx| {
                        entity.update(cx, |this, cx| {
                            this.confirming = false;
                            cx.notify();
                        })
                    }),
            )
    }
}
// snippet end
