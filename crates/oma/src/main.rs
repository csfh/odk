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
        /// A Bundle file, or a Bundle directory
        bundle: PathBuf,
        /// Record consent for exactly the permissions the Bundle asks for now, without a dialog
        #[arg(long)]
        accept_permissions: bool,
    },
    /// Put an installed Bundle file's previous version back, and the current one aside
    Rollback {
        /// The application's identifier
        identifier: String,
    },
    /// Delete an application's data, previous version, inbox, consent and pin, once it is no
    /// longer in ~/Applications
    Purge {
        /// The application's identifier
        identifier: String,
    },
    /// Build a project, install its Bundle over the installed copy and run it (needs an oma.toml)
    Dev {
        /// The project's directory, which holds manifest.toml and oma.toml
        #[arg(default_value = ".")]
        project: PathBuf,
        /// Stay, relay terminal signals and exit with the application's status
        #[arg(long)]
        wait: bool,
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
    /// Mount a Bundle file read-only, as a disk image, to look inside it, and print the directory
    Mount {
        /// Path to a Bundle file
        file: PathBuf,
        /// Then open the directory in the file manager
        #[arg(long, conflicts_with = "unmount")]
        open: bool,
        /// Unmount it instead, wherever it was mounted from
        #[arg(long)]
        unmount: bool,
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
            let agent = oma::fetch::agent();
            let fetch = |publisher: &oma::trust::Publisher| oma::fetch::oma_keys(&agent, publisher);
            match oma::flow::run(&layout, &host, &target, wait, &oma::consent::ask, &fetch) {
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
            bundle,
            accept_permissions,
        } => {
            let Some(layout) = oma::layout::Layout::from_env() else {
                eprintln!("odk oma: $HOME is not set");
                return ExitCode::FAILURE;
            };
            let installed = if bundle.is_file() {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs() as i64);
                let agent = oma::fetch::agent();
                oma::flow::install_file(
                    &layout,
                    &bundle,
                    accept_permissions,
                    now,
                    &oma::consent::ask,
                    &|publisher| oma::fetch::oma_keys(&agent, publisher),
                )
            } else {
                oma::flow::install(&layout, &bundle, accept_permissions, &oma::consent::ask)
            };
            match installed {
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
        Command::Rollback { identifier } => by_identifier(&identifier, |layout, app| {
            oma::flow::rollback(layout, app).map(|path| println!("{}", path.display()))
        }),
        Command::Purge { identifier } => by_identifier(&identifier, |layout, app| {
            oma::flow::purge(layout, app).map(|roots| {
                for root in roots {
                    println!("purged {app} under publisher key {root}");
                }
            })
        }),
        Command::Dev { project, wait } => {
            let Some(layout) = oma::layout::Layout::from_env() else {
                eprintln!("odk oma: $HOME is not set");
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
                    eprintln!("odk oma: {e}");
                    return ExitCode::FAILURE;
                }
            };
            let what = if prepared.replaced {
                "replaced the installed copy at"
            } else {
                "installed at"
            };
            eprintln!(
                "odk oma: {} {what} {}",
                prepared.identifier,
                prepared.installed.display()
            );
            if let Some(socket) = &prepared.foreign_daemon {
                eprintln!(
                    "odk oma: something else is serving {}, so the application will use that daemon, not the one just built",
                    socket.display()
                );
            }
            let host = oma::sandbox::launch::HostEnv::from_process();
            let agent = oma::fetch::agent();
            let fetch = |publisher: &oma::trust::Publisher| oma::fetch::oma_keys(&agent, publisher);
            match oma::flow::run(
                &layout,
                &host,
                prepared.identifier.as_str(),
                wait,
                &oma::consent::ask,
                &fetch,
            ) {
                Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
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
                eprintln!("odk oma: {e}");
                ExitCode::FAILURE
            }
        },
        Command::Mount {
            file,
            open,
            unmount,
        } => {
            use oma::view::{self, Kernel, Udisksctl};
            let done = if unmount {
                view::unmount(&file, &Kernel::host(), &Udisksctl)
            } else {
                view::mount(&file, &Kernel::host(), &Udisksctl).and_then(|dir| {
                    println!("{}", dir.display());
                    if open { view::open(&dir) } else { Ok(()) }
                })
            };
            match done {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("odk oma: {e}");
                    ExitCode::FAILURE
                }
            }
        }
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

/// Runs `act` on the application `identifier` names, and says why not on standard error.
fn by_identifier(
    identifier: &str,
    act: impl FnOnce(&oma::layout::Layout, &oma::fmt::Identifier) -> Result<(), oma::flow::Error>,
) -> ExitCode {
    let Some(layout) = oma::layout::Layout::from_env() else {
        eprintln!("odk oma: $HOME is not set");
        return ExitCode::FAILURE;
    };
    let app: oma::fmt::Identifier = match identifier.parse() {
        Ok(app) => app,
        Err(e) => {
            eprintln!("odk oma: {e}");
            return ExitCode::FAILURE;
        }
    };
    match act(&layout, &app) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("odk oma: {e}");
            ExitCode::FAILURE
        }
    }
}
