//! The files that register `.oma` with the system (ADR 0053, ADR 0056, ADR 0060 on pull request
//! #9; issue #11). They are text in `packaging/`; nothing here registers anything on the
//! machine running the tests, writes `/proc/sys/fs/binfmt_misc`, or calls `systemd-binfmt`.

use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

use odk_cli::Layout;

fn packaging(file: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../packaging")
        .join(file);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn the_binfmt_line_is_exactly_the_one_the_adr_gives_and_names_the_private_program() {
    let conf = packaging("binfmt.d/odk.conf");
    assert_eq!(conf, ":odk-oma:M:0:OMA1::/usr/lib/odk/odk-oma:\n");

    // Its fields: no mask, and no flags, so no `C` and no credentials from the file.
    let fields: Vec<&str> = conf.trim_end().split(':').collect();
    assert_eq!(
        fields,
        [
            "",
            "odk-oma",
            "M",
            "0",
            "OMA1",
            "",
            "/usr/lib/odk/odk-oma",
            ""
        ]
    );
    assert!(fields[7].is_empty(), "the flags field is empty");

    // It is the file the dispatcher runs for `odk oma` from `/usr/bin/odk`, and it is not a
    // shell wrapper.
    let system = Layout {
        exe_dir: Some("/usr/bin".into()),
        path: Some(OsString::new()),
    };
    assert_eq!(system.oma(), Some(fields[6].into()));
    assert!(!conf.contains("binfmt\n") && !fields[6].ends_with("/binfmt"));
}

#[test]
fn the_desktop_handler_runs_odk_oma_run_by_absolute_path_and_through_no_shell() {
    let desktop = packaging("applications/oma.desktop");
    let exec: Vec<&str> = desktop
        .lines()
        .filter_map(|l| l.strip_prefix("Exec="))
        .collect();
    assert_eq!(exec, ["/usr/bin/odk oma run %f"], "one Exec, the facade");
    for shell in ["sh -c", "bash", "/bin/sh", "env "] {
        assert!(!desktop.contains(shell), "{shell}");
    }
    assert!(desktop.contains("MimeType=application/x-oma;"));
    assert!(desktop.contains("NoDisplay=true"), "not in the launcher");
    assert!(
        !desktop.contains("Actions="),
        "there is no mount to show contents with yet"
    );
    // The command is a path the dispatcher resolves, then `oma`, never the file.
    let words: Vec<&str> = exec[0].split(' ').collect();
    assert_eq!(&words[1..], ["oma", "run", "%f"]);
}

#[test]
fn the_mime_type_matches_by_name_and_by_the_four_bytes_at_the_start() {
    let xml = packaging("mime/oma.xml");
    assert!(xml.contains(r#"<mime-type type="application/x-oma">"#));
    assert!(xml.contains(r#"<glob pattern="*.oma"/>"#));
    assert!(xml.contains(r#"<magic priority="80">"#));
    assert!(xml.contains(r#"<match type="string" value="OMA1" offset="0"/>"#));
}
