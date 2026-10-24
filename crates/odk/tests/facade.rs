//! Applications reach the theme through the facade, not through `odk-theme` directly.

use odk::theme::{Color, ThemeSnapshot};

#[test]
fn the_facade_exposes_the_theme() {
    let colors = "mode = \"dark\"\naccent = \"#ffffff\"\nmuted = \"#888888\"\nbackground = \"#000000\"\ndark_background = \"#000000\"\nlighter_background = \"#111111\"\nforeground = \"#ffffff\"\nred = \"#ff0000\"\nyellow = \"#ffff00\"\ngreen = \"#00ff00\"\nblue = \"#0000ff\"\n";
    let snapshot = ThemeSnapshot::from_toml(colors, None).unwrap();
    assert_eq!(snapshot.palette.accent, Color::from_hex("#ffffff").unwrap());
    assert_eq!(
        snapshot.semantic.text.primary,
        Color::from_hex("#ffffff").unwrap()
    );
}

/// Applications reach the database through the facade too (ADR 0049), and its raw binding.
#[test]
fn the_facade_exposes_the_database() {
    use odk::db::{Database, Migrations, values};

    const SCHEMA: Migrations = Migrations::new(&["CREATE TABLE t (n INTEGER)"]);
    let db = Database::memory(&SCHEMA).unwrap();
    let n = db
        .call_blocking(|c| {
            c.execute("INSERT INTO t VALUES (?1)", odk::db::params![7])?;
            Ok(c.query_row("SELECT n FROM t", [], |r| r.get::<_, i64>(0))?)
        })
        .unwrap();
    assert_eq!(n, 7);
    assert_eq!(values![1, "a"].len(), 2);
    let _: Option<odk::db::rusqlite::Error> = None;
}
