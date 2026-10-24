// snippet start
use odk::prelude::*;

fn overlays() -> impl IntoElement {
    h_flex()
        .gap(Space::Md)
        .child(DropdownMenu::new("file", "File").menu(|menu| {
            menu.item(MenuItem::new("Open").hint("Ctrl+O").on_select(|_, _| {}))
                .separator()
                .item(MenuItem::new("Quit").on_select(|_, _| {}))
        }))
        .child(Tooltip::new(
            "save-tip",
            "Save the file",
            Button::new("save").label("Save"),
        ))
        .child(Button::new("notify").label("Notify").on_click(|_, window, cx| {
            toast(window, cx, Toast::new("Saved").variant(Tone::Success));
        }))
}
// snippet end
