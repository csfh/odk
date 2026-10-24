# Plugin manifest specification, version 1

Normative. This spec defines the manifest of an ODK Plugin (ADR 0003, ADR 0020). It is an ODK concern, not an OMA one: oma never reads it and never sees Plugins (ADR 0010). It must not change a decision recorded in an ADR; if it needs to, write a new ADR first. Where the two disagree, the ADR wins.

## Layout and discovery

A Plugin is a directory holding a `plugin.toml` at its root and its Luau source files. Plugins are installed by the host application itself, not by oma. By default the host application loads Plugins from the subdirectories of `~/.local/share/oma/data/<identifier>/plugins/`, its own data directory (ADR 0012). The host application can configure the plugin directories through ODK, but only to places its sandbox already exposes: its data directory, its own Bundle contents, and named user directories it holds a Filesystem permission for.

Plugins are loaded when the host application starts and unloaded when it exits. Hot-reload and enabling or disabling Plugins at runtime are out of v1; changing the Plugins in use needs an application restart.

## `plugin.toml`

UTF-8 TOML. Any key not listed here is an error, and an invalid manifest means the Plugin is not loaded.

| Key | Type | Required | Rule |
|---|---|---|---|
| `plugin_manifest_version` | integer | yes | Must be `1`. |
| `id` | string | yes | Same syntax as an Identifier (`manifest.md`). Permanent. |
| `name` | string | yes | Display name. Non-empty UTF-8 with no control characters. |
| `version` | string | yes | Semver 2.0.0. |
| `entry` | string | yes | Path to the Luau file to run, relative to the Plugin directory, with no `..`, no absolute path and no symlink leaving the directory. |
| `capabilities` | array of strings | yes | The Plugin API capabilities the Plugin needs. May be empty. |
| `description` | string | no | One-line summary. |

## Capabilities

A capability is a part of the ODK Plugin API. v1 defines exactly four, and anything else is rejected until a new ADR adds it:

| Capability | Meaning |
|---|---|
| `app.state` | Read the state the host application chooses to expose. |
| `ui` | Add to the host application's interface through the ODK UI layer. |
| `network` | Make outbound requests through the host application. |
| `storage` | Read and write the Plugin's own private directory, and nothing else. |

The host application decides which capabilities to grant each Plugin. If the host refuses any capability a Plugin lists, the Plugin is not loaded and the host is told which were refused. Granting never exceeds what the host itself holds: a Plugin granted `network` still cannot connect if the host's sandbox has no network (ADR 0020).

## Example

```toml
plugin_manifest_version = 1
id = "com.example.notes.word-count"
name = "Word count"
version = "0.2.0"
description = "Shows a live word count"
entry = "main.luau"
capabilities = ["app.state", "ui"]
```

## Not covered here

The functions the Plugin API exposes for each capability, whether Plugins are isolated from each other in separate Luau states, resource limits (memory and time) per Plugin, and error reporting are specified separately. Memory and time limits work through `mlua` on Luau, with the caveat that the time limit cannot interrupt a long call into C code. The host must replace `require`. A per-Plugin environment is not a boundary between Plugins, because `getfenv` can reach it; isolating Plugins needs separate Luau states or removing `getfenv` and `setfenv` (`docs/checks.md`).
