# ODK apps are Rust and GPUI, with Luau as the plugin layer

ODK applications are written in Rust with a GPUI user interface. Lua is embedded so that Omarchy applications can be extended with Plugins; it is not the language applications are written in. Lua-authored applications are out of scope.

The embedded flavor is Luau, via `mlua` with `Lua::sandbox()` enabled. Plugins run inside the application's process, so the OS-level sandbox cannot separate a Plugin from its host app; the Lua sandbox is the only boundary around a Plugin. `mlua` is not sandboxed by default, and its sandbox mode is available for Luau only. PUC Lua 5.4 would have meant building and auditing that isolation ourselves. The cost is that Plugins are written in Luau, not standard Lua, so third-party Lua libraries do not just work.
