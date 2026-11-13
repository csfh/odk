# notes

A small application that uses ODK's Plugins (ADR 0092, `docs/spec/plugin-api.md`): a note, an Actions menu, and a status line. It is the template for an application that does; `src/lib.rs` says what each part shows.

- `plugins/word-count` has `app.state` and `ui`: it adds "Count words" to the Actions menu and reads the note the application exposed.
- `plugins/quote` has `ui` and `network`: it adds "Fetch a quote", asks the application for a URL, and hears the answer later. The application answers from a fixed table; ODK ships no HTTP client, and a real application makes the request itself, off its window's thread.

`cargo test -p notes` runs it headless, with a Plugin that never ends beside the two. To run the window, `odk oma dev examples/notes` needs an `oma.toml` (not written: the example is for reading and for the tests), or `cargo run -p notes` under a session of your own (the owner's tool; agents do not open windows, `CLAUDE.md`). The Plugins load from `plugins/` in the data directory, where the samples are written first when it is empty: `$HOME/plugins/` under `odk oma run`, and `~/.local/share/odk-examples/dev.example.notes/plugins/` when run by hand.
