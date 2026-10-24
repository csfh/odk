// snippet start
use odk::prelude::*;

struct Account {
    name: SharedString,
}

impl Render for Account {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap(Space::Md)
            .child(
                FormField::new("name-field", "Name")
                    .required(true)
                    .child(
                        Input::new("name", "Name")
                            .value(self.name.clone())
                            .placeholder("Ada Lovelace")
                            .on_change(cx.listener(|this, value: &str, _, cx| {
                                this.name = value.to_owned().into();
                                cx.notify();
                            })),
                    ),
            )
            .child(
                FormField::new("search-field", "Search")
                    .child(SearchInput::new("search", "Search settings")),
            )
            .child(
                FormField::new("password-field", "Password")
                    .child(PasswordInput::new("password", "Password")),
            )
    }
}
// snippet end
