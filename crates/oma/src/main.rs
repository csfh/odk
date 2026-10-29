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
        /// Path to a Bundle file or directory
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
    /// Sign the odk release `tools/package-release` made in DIR with the release root, whose
    /// 32-byte seed is read on standard input (ADR 0086)
    SignRelease {
        /// The directory holding install.sh and the tarball
        dir: PathBuf,
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
        /// The file holding the release key's 32-byte ed25519 seed, which signs the Bundle
        /// (ADR 0060). It is never put in the Bundle
        #[arg(long, value_name = "SEED")]
        sign: PathBuf,
        /// The key id of the publisher root that authorizes the release key, 16 hex digits
        #[arg(long, value_name = "KEY_ID")]
        root: String,
        /// The monotonic version, which must grow with every release (ADR 0060, section 4)
        #[arg(long, value_name = "N")]
        release: u64,
        /// Days until the signature expires; installs and upgrades refuse it after that
        #[arg(long, value_name = "DAYS", default_value_t = 365)]
        expires_in: u32,
        /// Where updates are published; an https URL on the publisher's host. The default is
        /// https://<publisher>/<Name>.oma
        #[arg(long, value_name = "URL")]
        update_url: Option<String>,
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

    // What the registration path units and the login run call (ADR 0009, ADR 0060 section 7).
    if std::env::args().skip(1).eq(["--register"]) {
        return register();
    }

    if let Some(path) = oma::entry::single_file(std::env::args_os().skip(1), &subcommands()) {
        eprintln!("odk oma: {}", oma::entry::refusal(&path));
        return ExitCode::FAILURE;
    }

    match Cli::parse().command {
        Command::Inspect { bundle_dir } if bundle_dir.is_file() => {
            let layout = oma::layout::Layout::from_env();
            match oma::flow::inspect_file(layout.as_ref(), &bundle_dir) {
                Ok(inspected) => {
                    print!("{}", inspect_file_text(&inspected));
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("odk oma: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Inspect { bundle_dir } => match Bundle::open(&bundle_dir) {
            Ok(bundle) => {
                print!("{}", inspect_text(bundle.manifest(), None));
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
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs() as i64);
            let prepared = match oma::flow::dev(
                &layout,
                &project,
                runtime.as_deref().filter(|p| p.is_absolute()),
                now,
                &oma::consent::ask,
                &oma::daemon::SystemctlStop,
            ) {
                Ok(prepared) => prepared,
                Err(e) => {
                    eprintln!("odk oma: {e}");
                    return ExitCode::FAILURE;
                }
            };
            if let Some(root) = &prepared.made_key {
                eprintln!(
                    "odk oma: made this computer's dev key in {}; dev builds are signed under its root {root}",
                    layout.dev_key_dir().display()
                );
            }
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
        Command::SignRelease { dir } => {
            use std::io::Read;
            let mut seed = Vec::new();
            if let Err(e) = std::io::stdin().take(64).read_to_end(&mut seed) {
                eprintln!("odk oma: cannot read the seed on standard input: {e}");
                return ExitCode::FAILURE;
            }
            let Some(root) = oma::trust::SigningKey::from_seed(&seed) else {
                eprintln!(
                    "odk oma: standard input is {} bytes; the release root is a 32-byte Ed25519 seed",
                    seed.len()
                );
                return ExitCode::FAILURE;
            };
            let running = match std::env::current_exe() {
                Ok(path) => path,
                Err(e) => {
                    eprintln!("odk oma: cannot find this program: {e}");
                    return ExitCode::FAILURE;
                }
            };
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs() as i64);
            let compiled = oma::odk_release::release_root();
            match oma::odk_release::sign(&dir, &root, compiled.as_ref(), &running, now) {
                Ok(release) => {
                    let key = root.public_key();
                    println!(
                        "signed release {} under the release root {} (fingerprint {})",
                        release.version,
                        key.key_id(),
                        key.fingerprint()
                    );
                    print!("{}", oma::odk_release::pem(&key));
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("odk oma: {e}");
                    // What the seed's public key is, all of it public, to compile in as the
                    // release root and to publish.
                    if matches!(
                        e,
                        oma::odk_release::Error::NoReleaseRoot
                            | oma::odk_release::Error::NotThisRoot { .. }
                    ) {
                        let key = root.public_key();
                        eprintln!(
                            "odk oma: the key on standard input is the root {}, fingerprint {}. To compile it in, put this in crates/oma/src/odk_release.rs:\n{}\nIts public key:\n{}",
                            key.key_id(),
                            key.fingerprint(),
                            oma::odk_release::constant_line(&key),
                            oma::odk_release::pem(&key).trim_end()
                        );
                    }
                    ExitCode::FAILURE
                }
            }
        }
        Command::Bundle {
            executable,
            manifest,
            out,
            includes,
            sign,
            root,
            release,
            expires_in,
            update_url,
        } => {
            let signing = match signing(&sign, &root, release, expires_in, update_url) {
                Ok(signing) => signing,
                Err(e) => {
                    eprintln!("odk oma: {e}");
                    return ExitCode::FAILURE;
                }
            };
            match includes
                .iter()
                .map(|text| oma::bundle::Include::parse(text))
                .collect::<Result<Vec<_>, _>>()
                .and_then(|includes| {
                    oma::bundle::write_file(&executable, &manifest, &out, &includes, &signing)
                }) {
                Ok(bundle) => {
                    println!("{}", bundle.display());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("odk oma: {e}");
                    ExitCode::FAILURE
                }
            }
        }
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

/// `signed` is a Bundle file's lines about its signature, which go before its permissions.
fn inspect_text(m: &oma::fmt::Manifest, signed: Option<&str>) -> String {
    let mut out = format!("{} {}\n{}\n", m.name, m.version, m.identifier);
    if let Some(description) = &m.description {
        out.push_str(description);
        out.push('\n');
    }
    if let Some(signed) = signed {
        out.push_str("\nSigned:\n");
        out.push_str(signed);
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

/// A Bundle file: [`inspect_text`] with its monotonic version, its publisher and its root,
/// whose fingerprint is shown when the root is pinned for this app id (ADR 0060).
fn inspect_file_text(inspected: &oma::flow::Inspected) -> String {
    let signed = &inspected.signed;
    let mut lines = format!(
        "  version {}\n  publisher {}\n",
        signed.version,
        signed.publisher.as_str()
    );
    match &inspected.pinned {
        Some(root) => lines.push_str(&format!(
            "  publisher key {}, pinned, fingerprint {}\n",
            signed.root_key_id,
            root.fingerprint()
        )),
        None => lines.push_str(&format!(
            "  publisher key {}, not pinned\n",
            signed.root_key_id
        )),
    }
    for (root, path) in &inspected.held {
        lines.push_str(&format!(
            "  held under publisher key {root}: {}\n",
            path.display()
        ));
    }
    inspect_text(&inspected.manifest, Some(&lines))
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

/// `odk oma --register`: reconciles the Bundle files in `~/Applications`, with no dialog, and
/// says on standard error what it did with any it did not keep.
fn register() -> ExitCode {
    let Some(layout) = oma::layout::Layout::from_env() else {
        eprintln!("odk oma: $HOME is not set");
        return ExitCode::FAILURE;
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let agent = oma::fetch::agent();
    let fetch = |publisher: &oma::trust::Publisher| oma::fetch::oma_keys(&agent, publisher);
    use oma::flow::Registered;
    for done in oma::flow::register(&layout, now, &fetch, &oma::flow::systemctl) {
        match done {
            Registered::Kept(_) => {}
            Registered::Applied(path) => eprintln!("odk oma: installed {}", path.display()),
            Registered::Held { app, .. } => {
                eprintln!("odk oma: {app} is signed by a different publisher and is held")
            }
            Registered::Deleted { path, why } => {
                eprintln!("odk oma: deleted {}: {why}", path.display())
            }
            Registered::Waiting { path, why } => {
                eprintln!("odk oma: left {}: {why}", path.display())
            }
        }
    }
    ExitCode::SUCCESS
}

/// What `bundle` signs with: the release key's seed read from `seed`, now as the issue time, and
/// `expires_in` days after it as the expiry.
fn signing(
    seed: &std::path::Path,
    root: &str,
    release: u64,
    expires_in: u32,
    update_url: Option<String>,
) -> Result<oma::bundle::Signing, String> {
    use oma::trust::{SigningKey, Timestamp};
    let bytes = std::fs::read(seed).map_err(|e| format!("{}: {e}", seed.display()))?;
    let release_key = SigningKey::from_seed(&bytes).ok_or_else(|| {
        format!(
            "{}: a release key is a 32-byte ed25519 seed, and this is {} bytes",
            seed.display(),
            bytes.len()
        )
    })?;
    let root = root
        .parse()
        .map_err(|_| format!("--root {root:?} is not a key id of 16 lowercase hex digits"))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let at = |seconds| Timestamp::from_unix_seconds(seconds).ok_or("the clock is out of range");
    Ok(oma::bundle::Signing {
        release: release_key,
        root,
        version: release,
        issued_at: at(now)?,
        expires_at: at(now + i64::from(expires_in) * 86_400)?,
        update_url,
    })
}
