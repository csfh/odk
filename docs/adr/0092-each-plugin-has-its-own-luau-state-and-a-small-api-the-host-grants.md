---
status: accepted
---

# Each Plugin has its own Luau state and a small API the host grants

ADR 0003 makes Luau the Plugin layer, with `Lua::sandbox()` as the only boundary around a Plugin. ADR 0020 has a Plugin declare capabilities and the host grant them. ADR 0091 binds Luau through `mlua`. `docs/spec/plugin-manifest.md` leaves four things open under "Not covered here": the functions of each capability, whether Plugins are isolated from each other, resource limits, and error reporting. `odk-plugin` cannot be written until they are settled. The owner put Plugins in v1 on 2026-11-11. This record recommends an answer to each. It is `proposed`: nothing here is accepted until the owner says so, and the numbers it leaves unnamed are to be measured, not chosen here.

## Decision

1. **One `Lua` state for each Plugin.** Plugins never share a global table, a function or a `getfenv` reach. `docs/checks.md` ("Luau limits through `mlua`") found that a global written by one chunk is visible to every other, and that `Chunk::set_environment` is not isolation, because `getfenv` is still present and a Plugin may reach another's environment through a function it is handed. Separate states close that without relying on removing `getfenv` and `setfenv`. The cost is a state's memory for each Plugin; it was not measured, and the first implementation measures it with the limits below.

2. **Only the granted capabilities exist in a state.** The host builds each state with one global table for each capability granted: `app` for `app.state`, `ui`, `network` and `storage`. A capability that was not granted has no table, so a Plugin can see what it was given and a call into what it was not given is an error of its own, not a missing-permission check scattered through the host. A Plugin that lists a capability the host refuses is not loaded, and the host is told which were refused, as the manifest spec has it.

3. **Plain data crosses the boundary, and nothing else.** The values a Plugin and the host pass each other are `nil`, booleans, numbers, strings and tables of them, copied. No host object is handed to Luau as userdata with methods on it; where a Plugin must name a host thing (a command it added, a request in flight) it holds an opaque identifier the host looks up and can refuse. This keeps a Plugin from reaching host internals by way of a value it was given.

4. **`require` is the host's.** The state's `require` is replaced by a host function that loads only a `.luau` file inside the Plugin's own directory, by the rule the manifest gives `entry` (relative, no `..`, no absolute path, no symlink leaving the directory), once for each state. The `./`-relative `require` that the sandbox leaves in place (`docs/checks.md`) is not reachable.

5. **The functions of each capability are one spec, `docs/spec/plugin-api.md`, written with the first implementation.** This record fixes only their shape:
   - `app.state` reads what the application chose to expose and subscribes to its changes. The application registers each readable value by name; there is no way to enumerate or reach the rest of its state.
   - `ui` adds to the interface through named extension points the application registers (a command, an item of a menu, a panel's content), and a Plugin gives a declarative description that ODK's own components draw. A Plugin is never handed a toolkit element, so it draws nothing the checklist does not govern, and a screen reader meets what a component says (ADR 0036, ADR 0089).
   - `network` hands a request to a `NetworkHandler` the application supplies and receives the answer later, never by blocking. ODK ships no HTTP client for applications: `ureq` is `odk-oma`'s (ADR 0066), and this record adds no dependency. What the handler allows, and the application's own sandbox network permission, bound it (ADR 0020).
   - `storage` reads, writes, lists and removes files under the Plugin's own private directory, `~/.local/share/oma/data/<identifier>/plugin-data/<plugin id>/`, by the path rule of `entry`, with a size cap that is a named constant of ODK's.
   - Plugins get no `Database` (ADR 0049) and drive no motion (`docs/spec/odk-motion.md`). Nothing in v1 gives them either.

6. **Limits are ODK's, named, and a Plugin that breaks one is unloaded.** Each state has a memory limit (`set_memory_limit`) and each call into it has a time budget (`set_interrupt`), both of which `docs/checks.md` found to work. Their values are named constants of `odk-plugin`, set from measurement of the first implementation and not here. A Plugin that exceeds either is unloaded for the rest of the session and reported, not resumed: `mlua` keeps the state usable after a memory error, but a script that was stopped half-way may have left what it was doing inconsistent, and a Plugin that is restarted by itself is a way round the limit. The interrupt cannot stop a long call into C library code (`docs/checks.md`), so the host exposes no function that takes long: `network` is asynchronous, and `storage` is bounded by its cap.

7. **A Plugin's failure never stops the host or another Plugin.** (`manifest` was added to the kinds on 2026-11-11, when `odk-plugin` was written: a `plugin.toml` that cannot be read, or an id that is already loaded, is a failure with no state to blame.) Every call into a Plugin returns a `Result` at the boundary, and a Plugin error is reported to the host as a value: the Plugin's `id`, the phase (`load` or `call`), the kind (`manifest`, `refused`, `syntax`, `runtime`, `memory`, `timeout`) and the message, through a callback the application supplies. ODK writes no log of its own. What the application does with it, such as a line in its interface, is the application's.

## Why one state each

A Plugin in the host's process has only Luau's sandbox around it (ADR 0003), so what one Plugin can see of another is a boundary of its own. The alternatives were one shared state with a `Chunk::set_environment` for each Plugin, which `docs/checks.md` found leaks through `getfenv`, and one shared state with `getfenv` and `setfenv` removed, which stops that one route and leaves shared globals and every route the spike did not look for. A state each is the one answer that does not depend on finding every route.

## What was not checked

- **The memory of a state**, and what a Plugin that does nothing costs at start. The first implementation measures both before the limits are named.
- **Whether the interrupt's overhead matters** on a hot path, since the interrupt runs inside Luau's loops.
- **A Plugin that blocks on a host function that is itself slow**, beyond keeping such functions out of the API.
- **Which extension points an application needs**, which is the application's to say and the first application that uses Plugins will show.

## Consequences

- `docs/spec/plugin-manifest.md`, "Not covered here", points at this record once it is accepted, and `docs/spec/plugin-api.md` is written with `odk-plugin`.
- `GLOSSARY.md` gains an entry for Capability, which it does not have, and its Plugin entry names the state.
- ADR 0003, ADR 0020 and ADR 0091 are unchanged. ADR 0020's second layer, "enforced by the Luau sandbox", is made concrete by points 1 to 3.
- The `plugin` feature of `odk` (ADR 0028) holds all of it, off by default.
