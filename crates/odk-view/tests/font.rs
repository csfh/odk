//! Finding an installed family. Nothing here opens a window.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use odk_theme::ThemeSnapshot;
use odk_view::{Face, Hosted, frame, headless};
use rune::{Cx, El, col};

/// What fontconfig gives for `pattern`: the file and the first family name.
fn fc(pattern: &str) -> (PathBuf, String) {
    let ask = |format: &str| {
        let output = Command::new("fc-match")
            .args(["-f", format, pattern])
            .output()
            .expect("fc-match");
        assert!(output.status.success(), "fc-match {pattern}");
        String::from_utf8(output.stdout).expect("utf-8")
    };
    (
        PathBuf::from(ask("%{file}").trim()),
        ask("%{family[0]}").trim().to_string(),
    )
}

fn flat(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// A directory of its own, removed when the test ends.
struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("odk-view-font-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("a directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn put(&self, relative: &str, bytes: &[u8]) {
        let target = self.0.join(relative);
        fs::create_dir_all(target.parent().expect("a parent")).expect("a directory");
        fs::write(target, bytes).expect("a file");
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn sans() -> (Vec<u8>, String) {
    let (file, family) = fc("sans");
    (fs::read(file).expect("read the sans font"), family)
}

#[test]
fn an_installed_family_is_found_by_its_own_name_wherever_in_a_font_directory_its_file_is() {
    let (bytes, family) = sans();
    let dir = Dir::new("found");
    dir.put(&format!("truetype/deep/{}.ttf", flat(&family)), &bytes);
    let face = Face::installed_in(&family, &[dir.path().to_path_buf()]).expect("the family");
    assert_eq!(flat(face.family()), flat(&family));
    assert!(face.faces() >= 1);
    // However it is written.
    let shouted = family.to_uppercase();
    assert!(Face::installed_in(&shouted, &[dir.path().to_path_buf()]).is_some());
}

#[test]
fn a_name_that_matches_a_file_but_not_its_family_finds_nothing() {
    let (bytes, _) = sans();
    let dir = Dir::new("ghost");
    dir.put("ghostfamily.ttf", &bytes);
    assert!(Face::installed_in("Ghost Family", &[dir.path().to_path_buf()]).is_none());
}

#[test]
fn a_file_that_is_not_a_font_a_missing_directory_and_no_family_find_nothing() {
    let (bytes, family) = sans();
    let dir = Dir::new("notfont");
    dir.put(&format!("{}.txt", flat(&family)), &bytes);
    assert!(Face::installed_in(&family, &[dir.path().to_path_buf()]).is_none());
    assert!(Face::installed_in(&family, &[dir.path().join("missing")]).is_none());
    assert!(Face::installed_in("", &[dir.path().to_path_buf()]).is_none());
}

#[test]
fn the_bold_face_comes_with_the_regular_one() {
    let (regular, family) = sans();
    let (bold_file, bold_family) = fc("sans:bold");
    if flat(&bold_family) != flat(&family) || bold_file == fc("sans").0 {
        eprintln!("this machine's sans has no bold file of its own; nothing to check");
        return;
    }
    let dir = Dir::new("bold");
    dir.put(&format!("{}-regular.ttf", flat(&family)), &regular);
    dir.put(
        &format!("{}-bold.ttf", flat(&family)),
        &fs::read(bold_file).expect("the bold font"),
    );
    let face = Face::installed_in(&family, &[dir.path().to_path_buf()]).expect("the family");
    assert_eq!(face.faces(), 2, "regular and bold");
}

#[test]
fn the_first_root_that_has_the_family_is_not_the_only_one_searched() {
    let (bytes, family) = sans();
    let (empty, full) = (Dir::new("empty"), Dir::new("full"));
    full.put(&format!("{}.ttf", flat(&family)), &bytes);
    let roots = [empty.path().to_path_buf(), full.path().to_path_buf()];
    assert!(Face::installed_in(&family, &roots).is_some());
}

struct App;

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    frame(hosted, cx, col().key("page"), Vec::new())
}

#[test]
fn a_window_draws_with_the_family_it_found() {
    let (bytes, family) = sans();
    let dir = Dir::new("window");
    dir.put(&format!("{}.ttf", flat(&family)), &bytes);
    let face = Face::installed_in(&family, &[dir.path().to_path_buf()]).expect("the family");
    let mut runtime = headless(
        Hosted::new(App, ThemeSnapshot::emergency(), face),
        page,
        (200.0, 100.0),
    );
    runtime.frame(1.0 / 60.0);
    assert!(
        runtime
            .elements()
            .iter()
            .any(|e| e.key.as_deref() == Some("page"))
    );
}
