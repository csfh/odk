//! The checklist's completeness rules for the components on the Rune host
//! (`docs/spec/component-checklist.md`, items 25, 26 and 28): every public component says what it
//! is and how it is used, is exported from the crate's root, and has a story of its own.

use std::fs;
use std::path::{Path, PathBuf};

/// Files that are the host or plumbing, not a component with a public type of its own.
const NOT_COMPONENTS: &[&str] = &[
    "describe.rs",
    "face.rs",
    "files.rs",
    "follow.rs",
    "font.rs",
    "host.rs",
    "ink.rs",
    "lib.rs",
    "number.rs",
    "post.rs",
    "store.rs",
    "style.rs",
];

/// Component files whose story has another name, or none: a helper the other components share
/// has no story of its own (item 28).
const STORY_NAMES: &[(&str, &str)] = &[
    ("radio.rs", "radio_group"),
    ("status.rs", "status_indicator"),
];
const NO_STORY: &[&str] = &[
    "hover.rs",
    "images.rs",
    "placement.rs",
    "toggle.rs",
    "tone.rs",
];

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn sources() -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = fs::read_dir(crate_dir().join("src"))
        .expect("odk-view/src")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .map(|path| {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            (name, fs::read_to_string(&path).expect("a source file"))
        })
        .collect();
    found.sort();
    found
}

fn components() -> Vec<(String, String)> {
    sources()
        .into_iter()
        .filter(|(name, _)| !NOT_COMPONENTS.contains(&name.as_str()))
        .collect()
}

/// The public types a file declares, with the line above each that is not an attribute.
fn public_types(text: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut found = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("#[cfg(test)]") {
            break;
        }
        let Some(rest) = line
            .strip_prefix("pub struct ")
            .or_else(|| line.strip_prefix("pub enum "))
        else {
            continue;
        };
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        let above = lines[..i]
            .iter()
            .rev()
            .find(|l| !l.trim_start().starts_with("#["))
            .copied()
            .unwrap_or("");
        found.push((name, above.trim().to_owned()));
    }
    found
}

/// Item 25: a public component has a doc comment that says what it is and how it is used.
#[test]
fn every_public_component_has_a_doc_comment() {
    let mut missing = Vec::new();
    for (file, text) in components() {
        for (name, above) in public_types(&text) {
            if !above.starts_with("///") {
                missing.push(format!("{file}: `{name}`"));
            }
        }
    }
    assert!(missing.is_empty(), "no doc comment: {}", missing.join(", "));
}

/// Item 26: every public type of a component is exported from the crate's root.
#[test]
fn every_public_component_is_exported_from_the_root() {
    let lib = fs::read_to_string(crate_dir().join("src/lib.rs")).expect("lib.rs");
    let exports: String = lib
        .lines()
        .filter(|l| l.trim_start().starts_with("pub use") || l.starts_with("    "))
        .collect::<Vec<_>>()
        .join(" ");
    for (file, text) in components() {
        for (name, _) in public_types(&text) {
            let word = |s: &str| {
                s.split(|c: char| !c.is_alphanumeric() && c != '_')
                    .any(|w| w == name)
            };
            assert!(
                word(&exports),
                "{file}: `{name}` is public and not exported from lib.rs"
            );
        }
    }
}

/// Item 28: a component has a story of its own, `stories/rune/<name>.rs`.
#[test]
fn every_component_has_a_story() {
    let stories = crate_dir().join("../../stories/rune");
    for (file, text) in components() {
        if NO_STORY.contains(&file.as_str()) || public_types(&text).is_empty() {
            continue;
        }
        let stem = file.trim_end_matches(".rs");
        let story = STORY_NAMES
            .iter()
            .find(|(f, _)| *f == file)
            .map_or(stem, |(_, s)| *s);
        let path = stories.join(format!("{story}.rs"));
        assert!(Path::new(&path).exists(), "{file}: no story at {path:?}");
    }
    // And a story that names no component is a stale one.
    let stems: Vec<String> = components()
        .into_iter()
        .map(|(f, _)| f.trim_end_matches(".rs").to_owned())
        .chain(STORY_NAMES.iter().map(|(_, s)| (*s).to_owned()))
        .collect();
    for entry in fs::read_dir(&stories).expect("stories/rune").flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(story) = name.strip_suffix(".rs") else {
            continue;
        };
        // The shared support file and the stories of components that live in a shared file.
        let shared = ["support", "context_menu", "heading"];
        assert!(
            stems.iter().any(|s| s == story) || shared.contains(&story),
            "stories/rune/{name} has no component"
        );
    }
}
