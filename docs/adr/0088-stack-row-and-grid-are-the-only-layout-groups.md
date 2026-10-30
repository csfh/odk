---
status: accepted
---

# Stack, Row and Grid are the only layout groups

An application and every component set elements in a column with `v_flex()`, in a line with `h_flex()` and a page's column with `content_column()`: three functions that return GPUI's `Div`, so a column was a `Div` that someone had told to be one. While building the component stories (`stories/`), the owner said on 2026-10-30: "Get rid of v_flex and content column. I want Stack to be the only vertical group of any elements", and that the layout vocabulary is Grid, Stack and a horizontal Stack. This ADR makes that the rule.

## Decision

- **`Stack` and `Row`** are the two ways to group elements in a line: `Stack::new()` one under the next, `Row::new()` side by side and centred across (as `h_flex()` was). `Grid`, a page's equal columns (`docs/spec/odk-ui-components.md`), is the third. `v_flex`, `h_flex` and `content_column` are removed, not deprecated.
- **Each is a newtype over `Div`** that implements GPUI's `Styled`, `InteractiveElement`, `ParentElement` and `Element` by delegation, the way GPUI's own `Stateful<E>` wraps an element. Every method the `Div` had still works on it (`.child`, `.gap(Space)`, `.w(px)`, `.debug_selector`, `.on_key_down`, `.when`), and `.id()` gives a `Stateful<Stack>` that an overlay accepts. Moving 369 `v_flex()` and 105 `h_flex()` sites is a rename.
- **`Stack::page()`** is what `content_column()` was: the foundation's content width as the most it takes, centred, with the panel padding and the section gap. `Row::stretch()` is a Row that does not centre across, for the frame of an application with a sidebar.
- **`ContentColumn` and `content_column_for_grid`**, the page column that counts a Grid's columns, keep their names and behaviour and are built on `Stack::page()`.
- **A group whose direction is chosen while drawing** (a resizable group, a placement that is beside or below) uses one crate-private function in `stack.rs`, so the choice is not two types.
- **What is not a group:** `scroll_y()` is a scrolling block; a bare `.flex()` that centres one element inside a component's own drawing (an icon in its slot) is not a group and stays; `oma-consent` does not use ODK. The checklist (item 43) scans `odk-ui` for `v_flex(`, `h_flex(`, `.flex_col()` and `.flex_row()` outside `stack.rs`, and for nothing else.

## Considered options

- **A Stack with only its own methods** (gap, padding, align, width). It reads better and suits an application that may use no pixel values, but 369 sites chain raw GPUI methods, odk-ui's overlay panels need a `Stateful` from `.id()`, and an element with no way to say `min_w_0` cannot follow ADR 0058. Rejected for the newtype, which keeps both.
- **`Row`** over `Inline`, `Cluster` and `HStack`; `Column` is the name of a `DataTable`'s column.
- **Keeping `v_flex` and `content_column` as wrappers over Stack** until Atmos moves. Rejected: the owner asked to get rid of them.

## Consequences

- Atmos (`frontend/`, which depends on this tree by path) uses `v_flex` (87 sites), `content_column` (6) and `h_flex` and does not build until it moves to `Stack::new()`, `Stack::page()` and `Row::new()`. `docs/migration.md` has the mapping.
- A method that is inherent on `Div` and not on a GPUI trait is not on `Stack` or `Row` until it is forwarded.
- ADR 0058 is not changed: the width and wrapping rules apply to a Stack as they did to a `Div`.
