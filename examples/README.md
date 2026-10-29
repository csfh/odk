# Examples

Small applications that show what building on ODK looks like. They are the target the ODK API is designed against: when a component or the `run` entry point changes, the examples change with it.

Each example is a member of the workspace, so `cargo build --workspace` compiles it and CI would catch an API change that breaks it. Run one with `cargo run -p hello`.

Each example holds an application crate and the `manifest.toml` of its Bundle. To make a Bundle, build the crate in release mode and sign it into one file: `cargo build --release -p hello`, then `odk oma bundle target/release/hello --manifest examples/hello/manifest.toml --sign <release seed> --root <root key id> --release <N>`. That writes `Hello.oma`, signed for the publisher the manifest names (ADR 0032, ADR 0052, ADR 0060). `odk oma install Hello.oma` installs it, after fetching that publisher's key set.

To build, install and run one in a single step, `odk oma dev examples/hello` does what its `oma.toml` says: it builds the crate and the licences the Bundle carries (ADR 0081), signs the Bundle with a dev key `odk oma dev` makes for this computer, installs it over the installed copy and runs it (ADR 0051, ADR 0085). No publisher's key set is fetched for it.

| Example | Shows |
|---|---|
| `hello` | A window with a heading, a counter and a button; the only permission is `gpu`. |
| `todo` | A to-do list kept in a SQLite database (`odk::db`, the `db` feature): a text field, check boxes, a segmented filter, a notice when the list cannot be kept. The only permission is `gpu`; the database is in the application's own data directory. `TODO_FILE` names another file, for a test or a tool. |
| `gallery` | Every built ODK component in each of its states, for looking at and for trying with the keyboard. Two switches at the top turn reduced motion on and swap the demo content for the worst realistic case. |
