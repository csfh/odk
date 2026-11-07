# Spikes

Throwaway programs from step 0 of `docs/plan.md`. Each answers one question from `docs/checks.md`; the answer is recorded there, and the code is kept only so the result can be reproduced. Nothing here is a design or a starting point for the real crates. Run one with `cargo run` inside its directory.

| Spike | Question |
|---|---|
| `landlock-scope` | Does Landlock's abstract-unix-socket scoping stop a connection to a socket made outside the domain? |
| `fexecve-init` | Can the same binary be re-executed as PID 1 through an fd after its path is hidden? |
| `mlua-limits` | Do `mlua`'s memory limit and interrupt work with Luau and the sandbox on? |
| `rune-ui` | Could ODK's components be built on Rune (`github.com/anomalyco/rune`) instead of GPUI? Run `cargo test` only; the binary opens a window, so give it to `tools/headless-shot` or `tools/idle-cpu`, never run it bare. |

The nested-user-namespace result came from a shell command, recorded in `docs/checks.md`.
