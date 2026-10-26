# Examples

Small applications that show what building on ODK looks like. They are the target the ODK API is designed against: when a component or the `run` entry point changes, the examples change with it.

Each example is a member of the workspace, so `cargo build --workspace` compiles it and CI would catch an API change that breaks it. Run one with `cargo run -p hello`.

Each example holds an application crate and the `manifest.toml` of its Bundle. To make a Bundle, build the crate in release mode and assemble it: `cargo build --release -p hello`, then `oma bundle target/release/hello --manifest examples/hello/manifest.toml`, which creates `Hello.oma/` (ADR 0032). `oma inspect Hello.oma` checks it. ADR 0052 accepts a single `.oma` file instead of that directory; the command still writes the directory until that ADR is implemented.

| Example | Shows |
|---|---|
| `hello` | A window with a heading, a counter and a button; the only permission is `gpu`. |
| `todo` | A to-do list kept in a SQLite database (`odk::db`, the `db` feature): a text field, check boxes, a segmented filter, a notice when the list cannot be kept. The only permission is `gpu`; the database is in the application's own data directory. `TODO_FILE` names another file, for a test or a tool. |
| `gallery` | Every built ODK component in each of its states, for looking at and for trying with the keyboard. Two switches at the top turn reduced motion on and swap the demo content for the worst realistic case. |
