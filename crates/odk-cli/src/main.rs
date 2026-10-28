//! `odk`: finds `odk-<name>` and replaces itself with it (ADR 0056). No spawn and no wait: the
//! exit status, the signals, the descriptors and the process are the program's.

use std::ffi::OsString;
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::process::{Command, ExitCode};

use odk_cli::{Action, Layout};

fn main() -> ExitCode {
    // Not a setuid program: it would be running a program found by name as another user.
    if ids_differ() {
        eprintln!("odk: the real and effective user ids differ; odk is not a setuid program");
        return ExitCode::from(126);
    }
    let exe = std::env::current_exe().ok();
    let layout = Layout {
        exe_dir: exe.as_deref().and_then(|e| e.parent()).map(Into::into),
        path: std::env::var_os("PATH"),
    };
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    match odk_cli::parse(&args) {
        Action::Help => print(&odk_cli::help(&layout)),
        Action::Version => print(&format!("odk {}\n", env!("CARGO_PKG_VERSION"))),
        Action::List => print(&odk_cli::list_lines(&layout)),
        Action::Usage(line) => {
            eprintln!("odk: {line}");
            ExitCode::from(2)
        }
        Action::Run { name, args } => {
            let Some(program) = layout.find(&name) else {
                eprint!("{}", odk_cli::not_found(&layout, &name));
                return ExitCode::from(127);
            };
            let mut command = Command::new(&program);
            // The program's argv[0] is the file selected, as an absolute path.
            command.arg0(&program).args(args);
            if let Some(exe) = exe {
                command.env("ODK", exe);
            }
            let error = command.exec();
            eprintln!("odk: cannot run {}: {error}", program.display());
            ExitCode::from(if error.kind() == std::io::ErrorKind::NotFound {
                127
            } else {
                126
            })
        }
    }
}

fn print(text: &str) -> ExitCode {
    match std::io::stdout().write_all(text.as_bytes()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

/// Whether the real and the effective user id differ, from `/proc/self/status`.
fn ids_differ() -> bool {
    let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
        return false;
    };
    status
        .lines()
        .find_map(|l| l.strip_prefix("Uid:"))
        .map(|ids| {
            let mut ids = ids.split_whitespace();
            ids.next() != ids.next()
        })
        .unwrap_or(false)
}
