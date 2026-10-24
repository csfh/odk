// snippet start
use odk::prelude::*;

struct Settings {
    page: SharedString,
}

impl Render for Settings {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .gap(Space::Lg)
            .child(
                NavList::new("pages", "Settings")
                    .group("Workspace")
                    .item("general", "General")
                    .item("keys", "Keyboard")
                    .disable("keys")
                    .value(self.page.clone())
                    .on_change(cx.listener(|this, page: &SharedString, _, cx| {
                        this.page = page.clone();
                        cx.notify();
                    })),
            )
            .child(
                Tabs::new("view", "View")
                    .tab("list", "List", |_, _| div().child("The list"))
                    .tab("grid", "Grid", |_, _| div().child("The grid")),
            )
    }
}
// snippet end
