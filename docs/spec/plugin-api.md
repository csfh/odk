# Plugin API specification

Normative for what `odk-plugin` builds. It fills in what `docs/spec/plugin-manifest.md` leaves under "Not covered here" and follows ADR 0092; it must not change a decision recorded in an ADR, and where the two disagree the ADR wins. Written on 2026-11-11 with the first implementation, so a function the crate does not have yet is listed as not built, and is not promised.

## The state a Plugin runs in

Each Plugin has a Luau state of its own (ADR 0092, point 1), made by `mlua` with `Lua::sandbox(true)` (ADR 0091). Nothing is shared with another Plugin: not a global, not a function. What a Plugin can reach is what the sandbox leaves, which `docs/checks.md` records for `mlua` 0.12: no `io`, no `package`, `os` reduced to `clock`, `date`, `difftime` and `time`, and `debug` to `info` and `traceback`.

The host sets two limits on every state. Both are named constants of `odk-plugin`, provisional until measurement settles them (ADR 0092, point 6):

| Limit | Constant | Value now |
|---|---|---|
| Memory of the state | `DEFAULT_MEMORY_LIMIT` | 8 MiB. An empty sandboxed state used 436 584 bytes (`mlua` 0.12.2, 2026-11-11). |
| Time of one call into the Plugin, and of loading it | `DEFAULT_CALL_BUDGET` | 100 ms. |
| Bytes in the Plugin's private directory | `DEFAULT_STORAGE_LIMIT` | 1 MiB. |

A Plugin that exhausts its memory or its time is unloaded for the rest of the session and reported. A Plugin that throws, or returns a value that may not cross, is reported and stays loaded.

## Globals a Plugin is given

A state has the global table of each capability granted and no other, so a Plugin can test for what it was given.

### `require(name)`

Always present. It loads `<name>.luau` from inside the Plugin's directory, once for each state, and returns what the file returns (`true` if it returns nothing). `name` is a relative path of plain segments: no `..`, no `.`, no empty segment, no absolute path, no backslash. A file reached through a symlink that leaves the Plugin's directory is refused. A source file may be 1 MiB. A name that starts with `./` or `../` is refused, since it holds a `.` or `..` segment; any other name is a file under the Plugin's directory and nothing else (there is no module search path and no `@` alias).

### `app` (capability `app.state`)

| Function | Returns |
|---|---|
| `app.get(name)` | The value the host exposed with `Host::expose(name, value)`, read when called, or `nil`. |

There is no way to list names or to reach anything that was not exposed. Subscribing to changes is not built.

### `storage` (capability `storage`)

The Plugin's private directory is `<application data directory>/plugin-data/<plugin id>/`. Every path is relative and follows the rule of `require`'s names, with a symlink that leaves the directory refused for reading, writing and removing, a dangling symlink included. A failure raises a Luau error.

| Function | Does |
|---|---|
| `storage.read(path)` | The file's bytes as a string, or `nil` if there is no such file. |
| `storage.write(path, content)` | Writes the file, making directories under the Plugin's directory as needed. Refused when the directory would hold more than the storage limit. |
| `storage.list([path])` | The names in a directory (the Plugin's own when `path` is omitted), sorted, as a list. |
| `storage.remove(path)` | Removes a file, and only a file. |

### `ui` and `network`

Not built. A Plugin granted either has the global table, which is empty, so it can see the grant. `ui` will take declarative descriptions for the extension points an application registers, and `network` will hand a request to a handler the application supplies and answer later (ADR 0092, point 5); neither is specified further until built.

## What crosses

A call into a Plugin and a value it returns are plain data (`odk_plugin::Data`): `nil`, a boolean, a number, a string, a list (keys 1 to n) or a map (string keys), copied. A function, a thread, userdata, a table that mixes string keys with others or has gaps, text that is not UTF-8, a table nested more than 16 deep, and more than 10 000 values are refused, and the call is reported as a `Runtime` failure.

## Errors

Every failure is a `PluginError` the host's callback hears: the Plugin's `id` (its directory's name when `plugin.toml` could not be read), the phase (`Load` or `Call`), the kind (`Manifest`, `Refused`, `Syntax`, `Runtime`, `Memory`, `Timeout`), the message, and for `Refused` the capabilities the host refused. ODK writes no log. A Plugin that is refused, or whose manifest is invalid, is not loaded; the others are.

## Host side

```text
Host::new(data_dir, Grants, Limits, report)   data_dir = ~/.local/share/oma/data/<identifier>
Host::load_dir(plugins_dir)                   at start; plugins_dir() is data_dir/plugins
Host::expose(name, Data)
Host::call(plugin id, function, &[Data]) -> Result<Data, CallError>
Host::loaded() -> ids
```

Loading is at start and unloading at exit; hot reload and enabling or disabling at runtime are out of v1 (`docs/spec/plugin-manifest.md`). Two Plugins with one id do not both load.
