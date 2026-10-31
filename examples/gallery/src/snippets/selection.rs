// snippet start
use odk::prelude::*;

fn choices() -> impl IntoElement {
    Stack::new()
        .gap(Space::Md)
        .child(
            FormField::new("country-field", "Country").child(
                Select::new("country", "Country")
                    .placeholder("Choose a country")
                    .item("dk", "Denmark")
                    .item("se", "Sweden")
                    .item("no", "Norway")
                    .disable("no")
                    .default_value("dk")
                    .on_change(|value, _, _| println!("chose {value}")),
            ),
        )
        .child(Checkbox::new("hidden", "Show hidden files").default_checked(true))
        .child(
            RadioGroup::new("size", "Size")
                .item("small", "Small")
                .item("large", "Large")
                .default_value("small"),
        )
        .child(Switch::new("wifi", "Wi-Fi").default_checked(true))
}
// snippet end
