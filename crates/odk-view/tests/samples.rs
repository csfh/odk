//! The sample registry in `common/mod.rs` covers every public component, and every sample builds
//! at every worst-case string. Nothing here opens a window.

mod common;

use std::collections::BTreeSet;

use common::{CONTENT, SAMPLES, open};

/// Exports from `lib.rs` that are not components, with the reason each is not one.
const IGNORED: &[(&str, &str)] = &[
    (
        "Accessible",
        "an accessibility value a component reports, not a component",
    ),
    ("Align", "a placement value a popup takes"),
    ("Placement", "a placement value a popup takes"),
    ("Cell", "a layout builder"),
    (
        "Column",
        "a DataTable column, built inside the DataTable sample",
    ),
    ("ColourOptionsError", "an error from the colour options"),
    (
        "CommandItem",
        "a Command item, built inside the Command sample",
    ),
    ("ChartHeight", "a named chart height, passed to a Chart"),
    (
        "ChartPath",
        "a path the application draws on a Chart's canvas",
    ),
    ("ContentColumn", "a layout builder"),
    (
        "Describe",
        "the trait that reports a component's accessible name",
    ),
    ("DropdownMenu", "the Menu sample builds it"),
    ("Face", "a font handle the host takes"),
    (
        "FieldControl",
        "a control a FormField takes, not a component of its own",
    ),
    ("Fielded", "the trait a FormField's child implements"),
    ("FileChooser", "the desktop's file chooser, not a component"),
    ("FileChooserError", "an error from the file chooser"),
    ("Flow", "how a Text or Heading wraps, passed to it"),
    ("Follower", "the theme follower, not a component"),
    ("Glyph", "an icon or a character a component takes"),
    ("Hosted", "the host a view runs in"),
    ("ImageFit", "how an Image fits its box, passed to it"),
    ("Ink", "a named colour a Chart paints with"),
    ("Layout", "a layout builder, not a component"),
    ("MAX_VISIBLE", "the toast limit, a constant"),
    ("MenuItem", "a menu entry, built inside the Menu sample"),
    ("NoWindow", "the error for a call made with no window open"),
    ("Role", "an accessibility role, not a component"),
    (
        "ResizablePanel",
        "a panel of a Resizable, built inside the Resizable sample",
    ),
    ("Scene", "the canvas a Chart paints on"),
    ("Side", "which edge a Sheet opens from, passed to it"),
    ("SortDirection", "a DataTable sort direction, passed to it"),
    ("SortKey", "a DataTable sort key"),
    ("Space", "a spacing scale, not a component"),
    ("Status", "the status a StatusIndicator takes"),
    ("StartFolder", "where the file chooser starts"),
    ("Store", "the state a component keeps, not a component"),
    ("StrokeWidth", "a stroke width a Chart paints with"),
    ("TextAnchor", "where a Chart's text sits"),
    ("TextBaseline", "where a Chart's text sits vertically"),
    (
        "Toaster",
        "the toast manager, built inside the Toast sample",
    ),
    ("ToastId", "the id of a toast"),
    ("Tone", "a colour tone a component takes"),
    ("Update", "a theme update, not a component"),
    ("Window", "a handle to a window `Windows` made"),
    ("WindowConfig", "the window's configuration"),
    (
        "Windows",
        "the builder of an application of more than one window",
    ),
];

/// The names `lib.rs` exports: every capitalised identifier in its `pub use` statements.
fn exported() -> BTreeSet<String> {
    let lib = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"))
        .expect("read lib.rs");
    let mut names = BTreeSet::new();
    let mut statement: Option<String> = None;
    for line in lib.lines() {
        // Only top-level exports: `pub mod prelude` re-exports the rune types inside it.
        if line.starts_with("pub use ") {
            statement = Some(String::new());
        }
        if let Some(text) = statement.as_mut() {
            text.push_str(line.trim());
            text.push(' ');
            if line.trim_end().ends_with(';') {
                for word in text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
                    if word.starts_with(|c: char| c.is_ascii_uppercase()) {
                        names.insert(word.to_string());
                    }
                }
                statement = None;
            }
        }
    }
    names
}

fn components() -> BTreeSet<String> {
    let ignored: BTreeSet<&str> = IGNORED.iter().map(|(name, _)| *name).collect();
    exported()
        .into_iter()
        .filter(|name| !ignored.contains(name.as_str()))
        .collect()
}

#[test]
fn every_public_component_has_a_sample() {
    let samples: BTreeSet<String> = SAMPLES.iter().map(|s| s.name.to_string()).collect();
    let components = components();
    let missing: Vec<_> = components.difference(&samples).collect();
    assert!(missing.is_empty(), "no sample for {missing:?}");
    let unlisted: Vec<_> = samples.difference(&components).collect();
    assert!(
        unlisted.is_empty(),
        "samples that are not public components: {unlisted:?}"
    );
}

#[test]
fn every_ignored_name_is_really_exported() {
    let exported = exported();
    for (name, reason) in IGNORED {
        assert!(
            exported.contains(*name),
            "{name} is ignored ({reason}) but lib.rs does not export it"
        );
    }
}

#[test]
fn sample_names_are_unique() {
    let mut seen = BTreeSet::new();
    for sample in SAMPLES {
        assert!(
            seen.insert(sample.name),
            "two samples named {}",
            sample.name
        );
    }
}

#[test]
fn every_sample_builds_at_every_worst_case_string() {
    for sample in SAMPLES {
        for content in CONTENT {
            let runtime = open(sample, content.text, (480.0, 360.0), None);
            let root = runtime
                .elements()
                .into_iter()
                .any(|e| e.key.as_deref() == Some("odk-root"));
            assert!(root, "{} with {} draws no page", sample.name, content.name);
        }
    }
}

fn has_text(runtime: &rune::Runtime<odk_view::Hosted<()>>, text: &str) -> bool {
    runtime
        .elements()
        .iter()
        .any(|e| !e.exiting && e.text.as_deref() == Some(text))
}

fn sample(name: &str) -> &'static common::Sample {
    SAMPLES
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("no {name} sample"))
}

#[test]
fn a_dialog_sample_draws_its_title_open() {
    let runtime = open(sample("Dialog"), "Save your changes?", (480.0, 360.0), None);
    assert!(has_text(&runtime, "Save your changes?"));
}

#[test]
fn a_select_sample_is_revealed_with_its_list_open() {
    let runtime = open(sample("Select"), "Fruit", (480.0, 360.0), None);
    assert!(has_text(&runtime, "Apple"), "the list did not open");
}

#[test]
fn a_tooltip_sample_is_revealed_by_a_rest_of_the_pointer() {
    let runtime = open(sample("Tooltip"), "A tip", (480.0, 360.0), None);
    assert!(has_text(&runtime, "A tip"), "the tooltip did not open");
}
