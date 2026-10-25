//! `oma`: runs, installs and sandboxes `.oma` Bundles (`docs/spec/oma-cli.md`).

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use oma::fmt::Bundle;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print a Bundle's name, Identifier, version and the permissions it requests
    Inspect {
        /// Path to a Bundle directory
        bundle_dir: PathBuf,
    },
    /// Run an application in its sandbox
    Run {
        /// An installed application's Identifier, or a path to a Bundle directory
        target: String,
        /// Stay, relay terminal signals and exit with the application's status
        #[arg(long)]
        wait: bool,
    },
    /// Ask for the permissions a Bundle requests and record the answer
    Consent {
        /// An installed application's Identifier, or a path to a Bundle directory
        target: String,
    },
    /// Copy a Bundle into ~/Applications
    Install {
        /// Path to a Bundle directory
        bundle_dir: PathBuf,
        /// Record consent for exactly the permissions the Bundle asks for now, without a dialog
        #[arg(long)]
        accept_permissions: bool,
    },
    /// Build a project, install its Bundle over the installed copy and run it (needs a dev.toml)
    Dev {
        /// The project's directory, which holds manifest.toml and dev.toml
        #[arg(default_value = ".")]
        project: PathBuf,
        /// Stay, relay terminal signals and exit with the application's status
        #[arg(long)]
        wait: bool,
    },
    /// Omarchy's text size and font, as sandboxed applications are shown them
    Omarchy {
        #[command(subcommand)]
        command: OmarchyCommand,
    },
    /// Assemble `<Name>.oma/` from a built executable and a manifest.toml (compiles nothing)
    Bundle {
        /// The built x86_64 ELF executable
        executable: PathBuf,
        /// The Bundle's manifest.toml; an icon it names is read from beside it
        #[arg(long)]
        manifest: PathBuf,
        /// Directory to create the Bundle in
        #[arg(short, long, default_value = ".")]
        out: PathBuf,
        /// A regular file to put in the Bundle too: `FILE` or `FILE=PATH/IN/BUNDLE`. Repeat it.
        /// An executable bit is kept; a symlink is refused (ADR 0050)
        #[arg(long = "include", value_name = "FILE[=PATH]")]
        includes: Vec<String>,
    },
}

#[derive(Subcommand)]
enum OmarchyCommand {
    /// Copy the user's text size and font for sandboxes now (`oma run` does it before every launch)
    Sync,
}

fn main() -> ExitCode {
    // Internal: `oma` re-executed as the init of an application's PID namespace (ADR 0024).
    let mut args = std::env::args();
    if args.nth(1).as_deref() == Some("--init") {
        let config = std::env::args().nth(2).unwrap_or_default();
        oma::sandbox::init::run(&config);
    }

    match Cli::parse().command {
        Command::Inspect { bundle_dir } => match Bundle::open(&bundle_dir) {
            Ok(bundle) => {
                print!("{}", inspect_text(&bundle));
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("oma: {e}");
                ExitCode::FAILURE
            }
        },
        Command::Run { target, wait } => {
            let Some(layout) = oma::layout::Layout::from_env() else {
                eprintln!("oma: $HOME is not set");
                return ExitCode::FAILURE;
            };
            let host = oma::sandbox::launch::HostEnv::from_process();
            match oma::flow::run(&layout, &host, &target, wait, &oma::consent::ask) {
                Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
                Err(e) => {
                    eprintln!("oma: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Consent { target } => {
            let Some(layout) = oma::layout::Layout::from_env() else {
                eprintln!("oma: $HOME is not set");
                return ExitCode::FAILURE;
            };
            match oma::flow::consent(&layout, &target, &oma::consent::ask) {
                Ok(true) => ExitCode::SUCCESS,
                Ok(false) => {
                    println!("nothing to ask: everything it requests is already allowed");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("oma: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Install {
            bundle_dir,
            accept_permissions,
        } => {
            let Some(layout) = oma::layout::Layout::from_env() else {
                eprintln!("oma: $HOME is not set");
                return ExitCode::FAILURE;
            };
            match oma::flow::install(&layout, &bundle_dir, accept_permissions, &oma::consent::ask) {
                Ok(path) => {
                    println!("{}", path.display());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("oma: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Dev { project, wait } => {
            let Some(layout) = oma::layout::Layout::from_env() else {
                eprintln!("oma: $HOME is not set");
                return ExitCode::FAILURE;
            };
            let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
            let prepared = match oma::flow::dev(
                &layout,
                &project,
                runtime.as_deref().filter(|p| p.is_absolute()),
                &oma::consent::ask,
                &oma::daemon::SystemctlStop,
            ) {
                Ok(prepared) => prepared,
                Err(e) => {
                    eprintln!("oma: {e}");
                    return ExitCode::FAILURE;
                }
            };
            let what = if prepared.replaced {
                "replaced the installed copy at"
            } else {
                "installed at"
            };
            eprintln!(
                "oma: {} {what} {}",
                prepared.identifier,
                prepared.installed.display()
            );
            if let Some(socket) = &prepared.foreign_daemon {
                eprintln!(
                    "oma: something else is serving {}, so the application will use that daemon, not the one just built",
                    socket.display()
                );
            }
            let host = oma::sandbox::launch::HostEnv::from_process();
            match oma::flow::run(
                &layout,
                &host,
                prepared.identifier.as_str(),
                wait,
                &oma::consent::ask,
            ) {
                Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
                Err(e) => {
                    eprintln!("oma: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Omarchy { command } => {
            let Some(layout) = oma::layout::Layout::from_env() else {
                eprintln!("oma: $HOME is not set");
                return ExitCode::FAILURE;
            };
            match command {
                OmarchyCommand::Sync => match oma::omarchy::refresh(&layout) {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(e) => {
                        eprintln!("oma: cannot copy: {e}");
                        ExitCode::FAILURE
                    }
                },
            }
        }
        Command::Bundle {
            executable,
            manifest,
            out,
            includes,
        } => match includes
            .iter()
            .map(|text| oma::bundle::Include::parse(text))
            .collect::<Result<Vec<_>, _>>()
            .and_then(|includes| {
                oma::bundle::assemble_with(&executable, &manifest, &out, &includes)
            }) {
            Ok(bundle) => {
                println!("{}", bundle.display());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("oma: {e}");
                ExitCode::FAILURE
            }
        },
    }
}

fn inspect_text(bundle: &Bundle) -> String {
    let m = bundle.manifest();
    let mut out = format!("{} {}\n{}\n", m.name, m.version, m.identifier);
    if let Some(description) = &m.description {
        out.push_str(description);
        out.push('\n');
    }
    out.push_str("\nPermissions:\n");
    let lines = oma::fmt::permission_lines(&m.permissions);
    if lines.is_empty() {
        out.push_str("  none beyond the baseline\n");
    }
    for line in lines {
        out.push_str(&format!("  {line}\n"));
    }
    out
}
