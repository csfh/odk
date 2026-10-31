// snippet start
use odk::prelude::*;

fn status() -> impl IntoElement {
    Stack::new()
        .gap(Space::Md)
        .child(Badge::new("Beta").tone(Tone::Info).marked())
        .child(StatusIndicator::new(Status::Success, "Connected"))
        .child(
            Alert::new("saved", "Changes saved")
                .variant(Tone::Success)
                .description("Your notes are up to date.")
                .on_dismiss(|_, _, _| {}),
        )
        .child(Progress::new("upload", "Uploading").value(35.0).show_value(true))
        .child(Spinner::new("loading").label("Loading…"))
}
// snippet end
