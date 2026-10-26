//! `odk-oma`, the command a person types as `odk oma`: runs, installs and sandboxes `.oma`
//! Bundles (ADR 0056). It is private and not on `PATH`; the `odk` command finds it.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use oma::fmt::Bundle;

#[derive(Parser)]
#[command(name = "odk oma", bin_name = "odk oma", version, about)]
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
    },
}

/// The words that are subcommands, which are never a path to a Bundle.
fn subcommands() -> Vec<String> {
    use clap::CommandFactory;
    let cli = Cli::command();
    let mut names: Vec<String> = cli
        .get_subcommands()
        .map(|c| c.get_name().to_owned())
        .collect();
    names.push("help".to_owned());
    names
}

fn main() -> ExitCode {
    // Internal: `oma` re-executed as the init of an application's PID namespace (ADR 0024).
    let mut args = std::env::args();
    if args.nth(1).as_deref() == Some("--init") {
        let config = std::env::args().nth(2).unwrap_or_default();
        oma::sandbox::init::run(&config);
    }

    if let Some(path) = oma::entry::single_file(std::env::args_os().skip(1), &subcommands()) {
        eprintln!("odk oma: {}", oma::entry::refusal(&path));
        return ExitCode::FAILURE;
    }

    match Cli::parse().command {
        Command::Inspect { bundle_dir } => match Bundle::open(&bundle_dir) {
            Ok(bundle) => {
                print!("{}", inspect_text(&bundle));
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("odk oma: {e}");
                ExitCode::FAILURE
            }
        },
        Command::Run { target, wait } => {
            let Some(layout) = oma::layout::Layout::from_env() else {
                eprintln!("odk oma: $HOME is not set");
                return ExitCode::FAILURE;
            };
            let host = oma::sandbox::launch::HostEnv::from_process();
            match oma::flow::run(&layout, &host, &target, wait, &oma::consent::ask) {
                Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
                Err(e) => {
                    eprintln!("odk oma: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Consent { target } => {
            let Some(layout) = oma::layout::Layout::from_env() else {
                eprintln!("odk oma: $HOME is not set");
                return ExitCode::FAILURE;
            };
            match oma::flow::consent(&layout, &target, &oma::consent::ask) {
                Ok(true) => ExitCode::SUCCESS,
                Ok(false) => {
                    println!("nothing to ask: everything it requests is already allowed");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("odk oma: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Install {
            bundle_dir,
            accept_permissions,
        } => {
            let Some(layout) = oma::layout::Layout::from_env() else {
                eprintln!("odk oma: $HOME is not set");
                return ExitCode::FAILURE;
            };
            match oma::flow::install(&layout, &bundle_dir, accept_permissions, &oma::consent::ask) {
                Ok(path) => {
                    println!("{}", path.display());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("odk oma: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Bundle {
            executable,
            manifest,
            out,
        } => match oma::bundle::assemble(&executable, &manifest, &out) {
            Ok(bundle) => {
                println!("{}", bundle.display());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("odk oma: {e}");
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
