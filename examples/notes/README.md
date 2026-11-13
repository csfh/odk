# notes

A small application that uses ODK's Plugins (ADR 0092, `docs/spec/plugin-api.md`) and its database (`odk::db`, ADR 0049, `docs/spec/odk-db.md`): a note, an Actions menu, and a status line. It is the template for an application that does either; `src/lib.rs` says what each part shows.

- The note is kept in `notes.sqlite` in the data directory: read once before the window, each change sent without waiting. A change that fails shows a notice under the note until the next one works; a database that cannot be opened shows one too, and the note stays in memory.

- `plugins/word-count` has `app.state` and `ui`: it adds "Count words" to the Actions menu and reads the note the application exposed.
- `plugins/quote` has `ui` and `network`: it adds "Fetch a quote", asks the application for a URL, and hears the answer later. The application answers from a fixed table; ODK ships no HTTP client, and a real application makes the request itself, off its window's thread.

`cargo test -p notes` runs it headless, with a Plugin that never ends beside the two and a database that fails. To run the window, `odk oma dev examples/notes` builds it by `oma.toml`, signs `Notes.oma` with this computer's dev key from `manifest.toml` and runs it in its sandbox, or `cargo run -p notes` runs it under a session of your own (both the owner's; agents do not open windows, `CLAUDE.md`). The Plugins load from `plugins/` in the data directory, where the samples are written first when it is empty, and the note is `notes.sqlite` beside them: the data directory is `$HOME` under `odk oma run`, and `~/.local/share/odk-examples/dev.example.notes/` when run by hand.
