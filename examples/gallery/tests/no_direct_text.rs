//! The gallery draws text through ODK components and decorations. A direct GPUI text call
//! (`text_role`, `text_ellipsis`, a font or a line height) would bypass the truncation rules
//! of ADR 0058.

use std::fs;
use std::path::Path;

/// Methods the gallery must not call. Field access such as `s.line_height` is not one of them.
const FORBIDDEN: &[&str] = &[
    ".text_ellipsis(",
    ".line_clamp(",
    ".whitespace_nowrap(",
    ".text_size(",
    ".text_role(",
    ".font_family(",
    ".font_weight(",
    ".font(",
    ".line_height(",
];

#[test]
fn the_gallery_draws_text_through_odk() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut hits = Vec::new();
    walk(root, &mut hits);
    assert!(
        hits.is_empty(),
        "direct GPUI text calls:\n{}",
        hits.join("\n")
    );
}

fn walk(dir: &Path, hits: &mut Vec<String>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
    for entry in entries {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            walk(&path, hits);
            continue;
        }
        if path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        // This file lists the calls it forbids.
        if path.file_name().is_some_and(|n| n == "no_direct_text.rs") {
            continue;
        }
        let source =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        for (n, line) in source.lines().enumerate() {
            for needle in FORBIDDEN {
                if line.contains(needle) {
                    hits.push(format!("{}:{}: {line}", path.display(), n + 1));
                }
            }
        }
    }
}
