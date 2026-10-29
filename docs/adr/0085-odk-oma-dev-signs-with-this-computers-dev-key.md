---
status: accepted
---

# `odk oma dev` signs with this computer's dev key

`odk oma dev` builds a project and installs its Bundle over the installed copy (ADR 0051, ADR 0073). It still installs a directory. ADR 0060 has no unsigned Bundle file, and `odk oma bundle --sign` needs a release key authorized by a root that a host serves. That is the publisher's process for a release. A developer should not need it for every build. On 2026-10-29 the owner asked whether `odk oma` can make a generic dev key when there is none, and chose to write this record and build it.

## Decision

- **The dev key.** The first `odk oma dev` that builds creates two Ed25519 seeds in the state directory: a dev root at `dev-key/root` and a dev release key at `dev-key/release`.
  - Each is 32 bytes from `getrandom`, mode `0600`, created only if it does not exist, in a `0700` directory.
  - `odk oma` says once that it made them, with the dev root's key id.
  - A seed that is not 32 bytes, or that the group or others can read, is refused. It is never replaced: a new root would leave every dev build's data under the old one.
  - The seeds go into no Bundle, sandbox, record or output. Only `odk oma dev` signs with them.
- **A dev build is a Bundle file.** `dev` writes the Bundle as `odk oma bundle --sign` would, signed by the dev release key under the dev root.
  - Its version is the time in Unix seconds, or one more than the pair's high-water mark when that is higher. Two builds in one second, or a clock that went back, still install.
  - It is issued now and expires in 365 days, with the default update URL.
  - The Manifest needs `publisher`, as every Bundle file does. The publisher stays display text.
- **Its key set.** Each `dev` run signs a key set with the dev root: the dev release key with a `not_after` 365 days on, at a version higher than the one kept. It is kept as ADR 0082 keeps a fetched one, and checked by the same rules.
- **The dev root's set is never fetched.** For a file under this computer's dev root, `odk-oma` uses the kept set, verified under the dev root, wherever it would fetch one: `install`, opening, `--register` and the inbox. The publisher's host serves its own root's set, which would refuse every dev build.
- **The pin.** `dev` pins the dev root for the app ids it installs, without the first-install dialog. The person ran `dev` on their own project, and that dialog asks whether to trust a publisher's key. The dev key belongs to this computer.
  - Permissions are asked as before. When the pair lacks some, `dev` shows Install or Cancel, with a note that this build is signed with this computer's dev key.
  - A Cancel leaves the installed copy, its daemon and its pair as they were. The dev key, once made, stays.
- **A pair of its own.** A dev build is the pair (dev root, app id), so its data, previous file, inbox and consent are its own (ADR 0060, section 2). ADR 0051 rejected a separate data location for dev copies. With pairs it follows from the root, and it is accepted.
- **One installed copy per app id.** A dev copy and a released one would share `~/Applications/<Name>.oma`, the `.desktop` entry, the daemon's unit and its socket.
  - `dev` refuses an app id installed as a Bundle file under another root, and names the file.
  - `odk oma install` and opening a file refuse an app id installed under another root, matched by app id and not by file name.
  - This choice is open to change.
- **Replacing a directory Bundle.** An installed directory Bundle of the same Identifier, the old `dev` output, is replaced by the file.
  - The directory is moved aside and removed only once the file is in place.
  - Its data directory is renamed to the pair's data directory, after the Cancel point and before the file is placed. If placing the file fails, both are put back. When the pair already has a data directory, `dev` refuses before asking anything, and merges nothing.
  - Its consent record is deleted: it was the directory Bundle's, and a later directory Bundle of the Identifier is asked again.
- **Later builds.** They replace the dev file through the pair's previous file (ADR 0011), so `odk oma rollback` goes back one build. The daemon is stopped first, as ADR 0051 says.

## Consequences

- ADR 0060 is amended. A root is pinned without the first-install dialog in one case: the dev root this computer made, for the app ids `odk oma dev` installs. That root's key set is the kept one, never one fetched.
- ADR 0051 is amended. Its rejected separate location for dev copies is how dev copies are kept now, and a dev build is a Bundle file.
- A dev build not rebuilt for a year is past its window. It runs until the next `--register` pass, which deletes it (ADR 0060, section 7), as it would a released file. The next build installs it again, and its data is kept.
- On another computer a dev build is refused: its root is not that computer's dev root, so its key set is fetched from the publisher, whose set names another root.
