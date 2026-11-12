//! Finding an installed family's files, for a window that may only load a font from bytes
//! (`face.rs`, ADR 0095).
//!
//! `odkd` names the family the user set (ADR 0062). A sandboxed window cannot ask fontconfig, but
//! the sandbox shows the system's fonts under `/usr` and the user's own at `~/.local/share/fonts`
//! (ADR 0095), so the files are looked for there: those whose name is near the family's, checked
//! against the family their own name table gives.

use std::fs;
use std::path::{Path, PathBuf};

use read_fonts::types::NameId;
use read_fonts::{FileRef, FontRef, TableProvider};

use crate::face::Face;

/// How deep a font directory is searched.
const DEPTH: usize = 8;

/// The weight of a regular face and of a bold one, in the font's own scale.
const REGULAR: i32 = 400;
const BOLD: i32 = 700;

/// The weight from which a face counts as bold.
const BOLD_FROM: i32 = 600;

impl Face {
    /// The family `family` from the fonts installed where the sandbox shows them: `/usr/share/fonts`,
    /// `/usr/local/share/fonts`, `~/.local/share/fonts` and `~/.fonts`. `None` when there is no
    /// such family, and the window keeps the bundled one.
    pub fn installed(family: &str) -> Option<Face> {
        Self::installed_in(family, &default_roots())
    }

    /// [`Face::installed`] looking in `roots` only.
    pub fn installed_in(family: &str, roots: &[PathBuf]) -> Option<Face> {
        let wanted = flat(family);
        if wanted.is_empty() {
            return None;
        }
        let mut files = Vec::new();
        for root in roots {
            collect(root, &wanted, 0, &mut files);
        }
        let mut found: Vec<(i32, Vec<u8>)> = Vec::new();
        let mut name = None;
        for path in files {
            let Ok(bytes) = fs::read(&path) else {
                continue;
            };
            if let Some((actual, weight)) = identify(&bytes, family) {
                // One family only: the first name that matched decides which.
                if name.get_or_insert_with(|| actual.clone()) == &actual {
                    found.push((weight, bytes));
                }
            }
        }
        let name = name?;
        let pick = |target: i32, floor: i32, found: &mut Vec<(i32, Vec<u8>)>| {
            let at = found
                .iter()
                .enumerate()
                .filter(|(_, (w, _))| *w >= floor)
                .min_by_key(|(_, (w, _))| (w - target).abs())
                .map(|(i, _)| i)?;
            Some(found.remove(at).1)
        };
        let regular = pick(REGULAR, 0, &mut found)?;
        let mut files = vec![regular];
        if let Some(bold) = pick(BOLD, BOLD_FROM, &mut found) {
            files.push(bold);
        }
        Some(Face::leak_files(files, name))
    }
}

fn default_roots() -> Vec<PathBuf> {
    let mut roots = vec![
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
    ];
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        roots.push(home.join(".local/share/fonts"));
        roots.push(home.join(".fonts"));
    }
    roots
}

/// Lowercase letters and digits only, so `JetBrains Mono` and `JetBrainsMono-Regular` meet.
fn flat(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// The font files under `dir` whose name holds `wanted`.
fn collect(dir: &Path, wanted: &str, depth: usize, into: &mut Vec<PathBuf>) {
    if depth > DEPTH {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        // A directory that is a link is not followed, so a loop cannot trap the search; a file
        // that is a link is, since fontconfig's own links to fonts are common.
        if kind.is_dir() {
            collect(&path, wanted, depth + 1, into);
        } else if is_font(&path)
            && fs::metadata(&path).is_ok_and(|m| m.is_file())
            && path
                .file_stem()
                .is_some_and(|stem| flat(&stem.to_string_lossy()).contains(wanted))
        {
            into.push(path);
        }
    }
}

fn is_font(path: &Path) -> bool {
    path.extension().is_some_and(|ext| {
        ["ttf", "otf", "ttc", "otc"]
            .iter()
            .any(|known| ext.eq_ignore_ascii_case(known))
    })
}

/// The family name a file gives and its weight, when it holds an upright face of `family`.
fn identify(bytes: &[u8], family: &str) -> Option<(String, i32)> {
    let file = FileRef::new(bytes).ok()?;
    for font in file.fonts().flatten() {
        let Some(actual) = family_of(&font) else {
            continue;
        };
        if flat(&actual) != flat(family) || slanted(&font) {
            continue;
        }
        let weight = font
            .os2()
            .map_or(REGULAR, |os2| i32::from(os2.us_weight_class()));
        return Some((actual, weight));
    }
    None
}

/// The name a face gives its family: the typographic one when it has it, as the family of a
/// whole range of weights, and the plain one otherwise.
fn family_of(font: &FontRef) -> Option<String> {
    name_of(font, NameId::TYPOGRAPHIC_FAMILY_NAME).or_else(|| name_of(font, NameId::FAMILY_NAME))
}

fn slanted(font: &FontRef) -> bool {
    let style = name_of(font, NameId::TYPOGRAPHIC_SUBFAMILY_NAME)
        .or_else(|| name_of(font, NameId::SUBFAMILY_NAME))
        .unwrap_or_default()
        .to_lowercase();
    style.contains("italic") || style.contains("oblique")
}

fn name_of(font: &FontRef, id: NameId) -> Option<String> {
    let names = font.name().ok()?;
    names
        .name_record()
        .iter()
        .filter(|record| record.name_id() == id)
        .find_map(|record| {
            let text: String = record.string(names.string_data()).ok()?.chars().collect();
            (!text.is_empty()).then_some(text)
        })
}
