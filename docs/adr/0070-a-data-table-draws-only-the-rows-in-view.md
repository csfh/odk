---
status: accepted
---

# A data table draws only the rows in view, one row height for all

Atmos lists processes, services and packages, live, re-sorted at every sample, and a process list reaches hundreds of rows or a thousand. ODK draws every child of every element on every frame it draws the window (`docs/checks.md`, "Frame cost"), so a list of a thousand rows of six cells would cost a frame of its own on every redraw. `docs/spec/odk-ui-components.md` kept `DataTable` blocked until a virtualised list was designed. GPUI 0.2.2 has `uniform_list`, which lays out and draws only the items in view, given that every item is as high as the first.

## Decision

**Rows are virtualised on GPUI's `uniform_list`, one height for every row:** the height of a control at the table's density, so a table of a thousand rows draws only the twenty or thirty in view. A row is one line; a cell's text ends in `…` and never wraps. Nothing taller than a row goes in a cell: a badge, a status dot or a small button fits, and a detail goes beside the table (a `Resizable` with the detail in the other panel) or in a sheet.

**`DataTable` is the first.** Generic over the application's row, with columns that say how to read it: `Column::new(key, header)` with `text(|row| …)` or `cell(|row, window, cx| element)`, a width as a weight of what is left (`grow`) or as a number of characters of the table's text (`chars`, which grows with the user's text size, and no pixel number in its API), `align_end` for numbers, and `sort_by(|row| key)` for a column that sorts. The application gives every row and a `row_key(|row| …)` that names it across updates; it filters the rows itself, so any filter is its own. The header is outside the scrolling rows, so it stays in place.

**Selection and keys.** One row is selected, by its key, so it stays selected when a sample re-sorts the rows. The rows are one tab stop: Up and Down move the selection, Home and End, Page Up and Page Down a page; the selected row is scrolled into view; Enter calls `on_activate`. A press selects; a double-click activates, and so does a press on the selected row when `activate_on_click` asks. The header's sortable columns are one tab stop before the rows: Left and Right move between them, Enter or Space sorts by the focused one, and again reverses it; a press on a header does the same. Sorting is the table's own unless the application passes `sort` and follows `on_sort`. The sorted column's header has `▲` or `▼` beside it, so the order never rests on colour.

**A row's menu.** `row_menu(|row, menu| …)` gives a row a context menu: it opens at the pointer on a right-click, which selects the row, and below the selected row with the Menu key or Shift+F10. A button drawn in a cell takes the pointer only, as a field's action does: its action must also be in the row's menu, so the keyboard reaches it.

**Empty and loading.** With no rows the table shows its `empty` text in the rows' place; while `loading` and empty it shows skeleton rows. A table that has rows and is loading keeps them.

**Truncation (amending ADR 0058).** A cell's text that does not fit shows its full text in a tooltip on hover. A row is selected as a whole and holds several cells, so the keyboard opens no cell's tooltip; the full text of every cell of the selected row is in its accessible description.

**Accessibility.** `odk-a11y` gains `Table`, `Row`, `Cell` and `ColumnHeader`. The table reports `Table` with its name, each header `ColumnHeader` with its sort as its value, each row `Row` with its cells' text, selected or not.

## Considered Options

- **GPUI's `list` (`ListState`).** It measures rows of different heights and keeps a sum tree of them; it costs more per row and needs the application to splice the state when rows change. A row that needs more room belongs in the detail.
- **Draw every row.** Atmos's QML does, with a cap of 80 rows; a cap hides what the user asked to see, and ODK's frame budget (checklist item 36) would fail long before a thousand rows.
- **Cells as tab stops (WAI-ARIA's grid).** A grid moves focus cell by cell, which suits a spreadsheet; a list of processes is chosen by row, and every action of a row is in its menu.
- **Sorting by the application only.** It would re-sort and hand over new rows on every header press; the table sorts the rows it was given, and tells the application, which can keep it.
