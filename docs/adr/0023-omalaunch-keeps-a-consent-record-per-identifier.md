---
status: amended by ADR 0060
---

# omalaunch keeps a consent record per Identifier and prompts on anything not in it

omalaunch stores what the user has consented to in `~/.local/share/oma/grants/<identifier>.toml`, a path applications can never reach (ADR 0012). On every launch it compares the Bundle's requested permissions with that record and shows the consent dialog (ADR 0016) if any requested permission is missing from it. This holds wherever the Bundle came from: opened from `~/Downloads` (ADR 0018), extracted straight into `~/Applications` with no install step, or arriving as an upgrade (ADR 0015). Without a record, a Bundle dropped into `~/Applications` would register and launch with no prompt, and "needs consent only if permissions grow" would have no baseline to compare against.
