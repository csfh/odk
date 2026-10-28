---
status: accepted
---

# An image is drawn from bytes the application holds

Atmos shows the user's avatar on its Accounts page and the current wallpaper on its Background page. Both are files of the user's (`~/.face`, a wallpaper under `~/.config/omarchy`), and a sandboxed application cannot read them: the sandbox grants six named user directories and nothing else (ADR 0012). Atmos's daemon, outside the sandbox, can. ODK had no image component, and an application built from the prelude alone cannot use GPUI's `img`.

## Decision

**`Image` draws an encoded image from bytes the application gives it**, not from a path: `Image::new(id, name).source(bytes)`, where the bytes are an `Arc<[u8]>` of a PNG, JPEG, WebP, GIF or BMP file, which ODK recognises by its first bytes. A Bundle's daemon reads the file and sends the bytes; the window never needs the path to be readable. An application that can read the file itself reads it and passes the bytes. There is no path API, so no image is read from the disk while drawing.

**It fills the box its parent gives it**, with `fit(ImageFit::Cover)` (the default, cropped to fill) or `Contain` (all of it shown), in the application's corner radius. `Image::avatar(id, name)` is the one round image: a circle as wide as the foundation's largest spacing step, `s12` (48 px at the default text size, growing with it), cropped to fill.

**Before the bytes arrive it is a skeleton** of its own size; bytes that are not an image it knows draw a quiet placeholder with a mark, and it reports that the image could not be shown. The decoded image is kept while the application passes the same `Arc` again, so a redraw neither hashes nor decodes the bytes again.

**Accessibility.** It reports `Image` with its name, which says what it shows ("Your avatar"), never the file's name.

This decides nothing for Atmos's daemon, which needs an operation that answers with the bytes of the avatar and of the wallpaper: its `host.read` answers with text.

## Considered Options

- **A path, read by ODK.** It would fail in every sandbox for exactly the files these pages show, and it would read a file while drawing.
- **A grant of the file to the sandbox.** The permission vocabulary has no single files (ADR 0012), and the wallpaper changes path with the theme.
- **A thumbnail from `odkd`.** `odkd` answers every application alike (ADR 0062) and would become a reader of the user's files for any of them.
