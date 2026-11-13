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

| `app.subscribe(name, function_name)` | Names one of the Plugin's own global functions to call when the application publishes `name` (`Host::publish`): it is called with `name` and the value. Returns nothing. A Plugin may subscribe to 32 names (`MAX_SUBSCRIPTIONS`); a second subscription to a name replaces the first. |

There is no way to list names or to reach anything that was not exposed or published.

### `storage` (capability `storage`)

The Plugin's private directory is `<application data directory>/plugin-data/<plugin id>/`. Every path is relative and follows the rule of `require`'s names, with a symlink that leaves the directory refused for reading, writing and removing, a dangling symlink included. A failure raises a Luau error.

| Function | Does |
|---|---|
| `storage.read(path)` | The file's bytes as a string, or `nil` if there is no such file. |
| `storage.write(path, content)` | Writes the file, making directories under the Plugin's directory as needed. Refused when the directory would hold more than the storage limit. |
| `storage.list([path])` | The names in a directory (the Plugin's own when `path` is omitted), sorted, as a list. |
| `storage.remove(path)` | Removes a file, and only a file. |

### `ui` (capability `ui`)

A Plugin adds a declarative description to an extension point that the application registered (`Host::register_point(name)`), and the application draws what it finds (`Host::contributions(point)`) with its own components. A Plugin is never handed a toolkit element, so a screen reader meets what a component says. A Plugin cannot make a point: a name the application did not register is an error.

| Function | Does |
|---|---|
| `ui.add(point, description)` | Adds `description`, a table with string keys of plain data, to `point` and returns its identifier (a number). If `description.action` is text, it names a global function of the Plugin that `Host::activate(plugin, id)` calls with the identifier when the user uses the item. |
| `ui.remove(id)` | Removes one of the Plugin's own descriptions. Returns whether there was one. |

A description may hold 16 KiB of text, keys included (`MAX_DESCRIPTION_BYTES`), and a Plugin may have 64 added at once (`MAX_CONTRIBUTIONS`); both are provisional. What a description may say (`label`, `icon`, `items`, ...) is the application's to define for each point, so the host checks the shape of the plain data and not its meaning. A Plugin that is unloaded loses its descriptions.

### `network` (capability `network`)

A Plugin hands a request to the application and hears the answer later; it never blocks and never touches a socket. ODK ships no HTTP client for applications (ADR 0092): the application's handler (`Host::on_network`) decides about each request, makes it however it likes, and answers with `Host::answer(plugin, id, Ok(data) | Err(reason))`, now or later. With no handler set, `network.request` is an error.

| Function | Does |
|---|---|
| `network.request(request)` | Checks `request` and gives it to the handler, and returns its identifier (a number). `request` is a table with `url` (text, `https://` or `http://`, at most 2048 bytes), `method` (`GET`, `HEAD`, `POST`, `PUT`, `PATCH` or `DELETE`; `GET` if omitted), `headers` (a table of text, at most 32, names without `:` or space, values with no line break) and `body` (text, at most 64 KiB). Any other field is an error. A Plugin may have 8 requests waiting (`MAX_PENDING_REQUESTS`); the limits are provisional. |
| `network.cancel(id)` | Forgets one of its waiting requests, so an answer to it is refused. Returns whether it was waiting. |

The answer calls the Plugin's global `on_network(id, ok, payload)`: `ok` is true and `payload` the response (plain data) or `ok` is false and `payload` the reason (text). A request is answered once: answering one that was answered, cancelled or whose Plugin was unloaded is `CallError::NoSuchRequest`. The handler must not block and must not call into the host from inside the handler. What the handler allows, and the application's own sandbox network permission, bound what a Plugin can reach (ADR 0020).

## What crosses

A call into a Plugin and a value it returns are plain data (`odk_plugin::Data`): `nil`, a boolean, a number, a string, a list (keys 1 to n) or a map (string keys), copied. A function, a thread, userdata, a table that mixes string keys with others or has gaps, text that is not UTF-8, a table nested more than 16 deep, and more than 10 000 values are refused, and the call is reported as a `Runtime` failure.

## Errors

Every failure is a `PluginError` the host's callback hears: the Plugin's `id` (its directory's name when `plugin.toml` could not be read), the phase (`Load` or `Call`), the kind (`Manifest`, `Refused`, `Syntax`, `Runtime`, `Memory`, `Timeout`), the message, and for `Refused` the capabilities the host refused. ODK writes no log. A Plugin that is refused, or whose manifest is invalid, is not loaded; the others are.

## Host side

```text
Host::new(data_dir, Grants, Limits, report)   data_dir = $HOME in the sandbox (the pair's data directory, ADR 0099)
Host::load_dir(plugins_dir)                   at start; plugins_dir() is data_dir/plugins
Host::expose(name, Data)                      a value Plugins can read with app.get
Host::publish(name, Data) -> usize            expose it and call each subscriber; how many were called
Host::register_point(name)                    an extension point a Plugin with `ui` may add to
Host::contributions(point) -> Vec<Contribution>
Host::activate(plugin, id) -> Result<Data, CallError>   the user used a description
Host::on_network(handler: Fn(NetworkRequest))
Host::answer(plugin, id, Result<Data, String>) -> Result<Data, CallError>
Host::call(plugin id, function, &[Data]) -> Result<Data, CallError>
Host::loaded() -> ids
```

Loading is at start and unloading at exit; hot reload and enabling or disabling at runtime are out of v1 (`docs/spec/plugin-manifest.md`). Two Plugins with one id do not both load.
