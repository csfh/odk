# The component gallery

The gallery (`examples/gallery`) is ODK's design-system explorer and its test bench: every component in each of its states, the foundation tokens, the Adaptive Colour Resolver's reasoning, and tools for checking a theme, a density or a corner setting against all of it. It is a real ODK application: every preview is a real component drawn from the real tokens, so what is shown is what an application gets. It is not a promotional page.

It serves five purposes: exploring components, validating them against the design language (ADR 0040), testing themes, inspecting resolved tokens, and testing interaction (keyboard, pointer, focus, motion).

## Running it

- On a desktop: `cargo run -p gallery`. `GALLERY_PAGE=<id>` opens it on a page (ids below) and `GALLERY_THEME=<name or directory>` opens it previewing a theme, for example `GALLERY_THEME=nord` or a directory holding a `colors.toml`.
- Headless, which is the only way to run it in the development environment (`CLAUDE.md`, "Windows: headless only"): `GALLERY_PAGE=colors GALLERY_THEME=nord tools/headless-shot --size 1600x1000 target/shot.png -- ./target/debug/gallery`. `--keys "Tab Tab"` sends keys to that display only.
- Tests: `cargo test -p gallery`. Benchmarks: `cargo bench -p gallery`. Idle cost on the real renderer: `tools/idle-cpu --all`, the gate for any gallery change (2% idle, 8% with a spinner in view).

## The window

A toolbar, a sidebar, one page, and a status bar.

- **Toolbar**: the page's place (`ODK / Foundation / Colors`), then the settings that apply to every page at once: **Theme**, **Density**, **Corners**, **Motion**, **Content** (the demo text or a worst case of long labels and other scripts) and **Reset**. They are available on every page and do not move it.
- **Sidebar**: a search field over the pages, in groups: Overview, Actions, Forms, Navigation, Feedback, Overlays, Foundation, Tools. The sidebar is one Tab stop; the arrow keys move inside it and Enter opens a page. It scrolls by itself.
- **Page**: scrolls by itself under its title and description, and ends with its usage example and its keyboard and accessibility notes.
- **Status bar**: what is in force: the theme (and whether it is a preview), the density with its height, the corners with their radius, the motion setting, and the theme's generation.

In a window narrower than `layout::NARROW_BELOW` the sidebar narrows and the toolbar drops its labels (the groups keep their names for assistive technology). The window's structural numbers are in `examples/gallery/src/layout.rs` and nowhere else; spacing is the foundation's.

## Pages

| Group | Page (id) | What it shows |
|---|---|---|
| | Overview (`overview`) | How to use the gallery, what it covers, what is not built and why |
| Actions | Buttons (`buttons`) | Eight variants, three densities, content, states (a listening button that records a chord among them), emphasis, and the tokens of one button by state |
| Forms | Text fields (`fields`), Selection (`selection`), Form (`form`) | Input, search, password, number, textarea, input group; select, command list, check box, radio group, switch; every state and a complete form |
| Navigation | Navigation (`navigation`) | Tabs, segmented controls, navigation list, breadcrumbs |
| Feedback | Feedback (`feedback`) | Badges, status indicators, alerts, progress, spinners, skeletons, separators |
| Overlays | Overlays (`overlays`), Dialogs and sheets (`dialogs`) | Menus, tooltips, popovers, hover cards, command palette, kbd; dialogs, sheets, toasts |
| Data | Data (`data`) | Resizable panels in a row and a column, and what a resize reports; a data table of two hundred processes that sorts, selects, activates and has a menu per row, with its empty and loading states; charts drawn on a canvas in named inks (series that move between samples, a heat grid, a donut, a view that pans and zooms); images from bytes (an avatar, cropped and whole, waiting and broken); items as a settings page draws its rows (a switch, a select and two buttons at the end, a number field across, a status badge) |
| Foundation | Colors (`colors`) | The theme's source palette, and the semantic colours resolved from it |
| | Colour resolver (`resolver`) | Why the resolver chose each colour |
| | Typography, Spacing, Geometry, Motion (`typography`, `spacing`, `geometry`, `motion`) | The foundation tokens, with real components |
| | Accessibility (`interaction`) | Real controls and a log of the events they produce |
| Tools | Playground (`playground`) | A real button or form control with its properties and its code |
| | Theme comparison (`compare`) | Two themes side by side as tokens, and as real components in turn |
| | Theme diagnostics (`diagnostics`) | What the theme manager knows |

A page appears only when the component or the demonstration exists. What the brief for the gallery listed and ODK does not have is on the Overview page: a combobox (the Command palette is ODK's searchable list), pagination, a separate button group (the segmented control is it), forced hover and focus states on a live control, real components in two themes at once, syntax highlighting, collapsible sidebar groups and a docked inspector.

## Reference screenshots

`tools/visual-scenarios` takes the same screenshots every time, through `tools/headless-shot` (a private headless sway; the user's session is never touched), into `target/visual/`: the pages that show every component family in six themes (Tokyo Night, Nord, Catppuccin Mocha and Latte, Vantablack and the incomplete custom fixture), the buttons, fields and navigation pages at every density with square and subtle corners, keyboard focus, reduced motion, and a menu opened from the keyboard. `--only themes|geometry|states` takes one set; a run of all of them is 46 shots (24 themes, 18 geometry, 4 states) and takes some minutes.

What makes them repeatable is what the gallery reads from the environment: `GALLERY_PAGE`, `GALLERY_THEME` (a theme's name or a directory), `GALLERY_DENSITY` (`compact`, `default`, `comfortable`), `GALLERY_CORNERS` (`square`, `subtle`) and `GALLERY_MOTION` (`reduced`, `full`). The window is 1600 by 1000, and the machine's fonts and the user's text size apply, so the screenshots are for the eye and for comparing one run with the last on the same machine, not for a pixel diff: text shaping and the software renderer differ between machines, and a perceptual diff tool was not added (it would need a baseline kept per machine). The assertions are in the tests.

## Search

The sidebar's field searches the gallery's own table (`registry.rs`), with nothing indexed and nothing fetched. Every word must match a page's title, group, description, a component it shows, a keyword, or a token name: the palette keys (`palette.bright_yellow`), the semantic colours (`roles.warning.muted.default`, `surface.canvas`), the resolver's subjects (`warning solid fill`), type roles (`type.body`), spacing (`space.4`, `layout.dialog_padding`), geometry and motion (`radius.default`, `motion.fast`). Results show what matched. Enter in the field opens the best match; Tab reaches the results, where Up and Down move and Enter opens one; an empty result says so.

## Theme preview

The gallery follows Omarchy's current theme. The **Theme** selector lists the installed themes (`ThemeSource::installed()`: `~/.config/omarchy/themes` and `/usr/share/omarchy/themes`, plus the one in `GALLERY_THEME`); choosing one *previews* it: every window draws it, and Omarchy's selection and every theme file are untouched. "System theme" (or **Reset**) returns to the followed theme, as it is then: a theme change that arrives during a preview is kept and drawn when the preview ends. The preview is process-wide, since ODK keeps one theme store for the process (`docs/spec/odk-source-palette.md`, "Previewing a theme"). A theme that cannot be loaded leaves the drawn theme as it is and says why in the toolbar.

A previewed theme is loaded once, when chosen, with the application's colour options and the user's own text size and request for reduced motion; drawing never reads a file.

## Density, corners and motion

- **Density** (Compact, Default, Comfortable) and **Corners** (Square, Subtle) are the application's foundation settings (`odk::ui::configure`). They apply to every control on every page immediately, without resetting what a person typed or chose. Their values are the foundation's, not the gallery's: control heights come from the density, and the radius from `CornerRadius::px()`. Subtle is **2 px** (ADR 0042): a 0.8 px corner was drawn and could not be seen. The status bar always shows the value in force.
- **Motion** is Full or Reduced. Reduced makes every transition run for zero time. It can add to the user's own request, which is the compositor's (Hyprland's animations off, ADR 0048; a sandboxed application cannot ask it), and never take it back: when the user has asked, the toolbar says "Reduced by the system" instead of offering the choice. The user's own `[motion] reduced` key is ignored.
- Nothing is persisted: the gallery starts with the theme followed, default density, subtle corners and full motion, and **Reset** returns to that.
- A page's own state (the playground's properties, a form's values, the comparison's themes, the search text, the Content setting) survives every change of theme, density, corners, motion and window size.

## Token inspection

Every component page ends with its tokens (the Buttons page has its own inspector, the Playground one for the button and control it shows). They are read from the component tokens' diagnostics (`ComponentTokens::diagnostics`, `docs/spec/odk-component-tokens.md`), built once per published set of tokens and never while drawing. A line gives the style (variant, state, role and emphasis), its height, corner radius and type role, the contrast it reaches and the surface it is least on; under it, the background, foreground and border as swatches with their values and the semantic token each came from (`roles.warning.muted.default.background`, `surface.canvas`). The foundation's padding, border, focus ring and durations are listed with them. The inspector calculates no style of its own.

## Colour resolver diagnostics

The **Colors** page shows the source palette exactly as the theme wrote it (a key the theme leaves out is shown as left out, never filled in), the derived surfaces, text and borders, and every role at every emphasis with its resolved colours and contrast.

The **Colour resolver** page answers "why this colour?" from the resolver's own report, which the theme's snapshot keeps (`ThemeSnapshot::resolution`). Choose a role (or surfaces, text, borders): each decision shows its source token, the original and resolved colours with their lightness, chroma and hue, whether it was adjusted, the contrast it needed and reached before and after, the reason the resolver gave, and every candidate it weighed in priority order with its contrast, whether it passes as written and which was chosen. For Tokyo Night's warning fill, `yellow` is chosen because it is the first candidate that passes as the theme wrote it, and `bright_yellow`, which also passes, comes after it in priority. Notes (a fill too close to the canvas, a focus colour too close to its fill) are listed with the role. Where the resolver recorded no decision, the page says so rather than explaining one.

## Accessibility

The **Accessibility** page has real controls (buttons, a busy button, a disabled one, a check box, a switch, a radio group, tabs, a segmented control, a select, a dialog, a menu, a popover, a tooltip) and a log of what they did: the key or the pointer that activated each, the value each reported, every move of focus read back from the window, and dialog and popover opens and closes. Nothing in the log is simulated or inferred. Below it: the focus, motion and theme in force, status shown with and without colour, and the pair of each component with the least contrast to spare. The contrast figures are checks of the resolved tokens (WCAG 2.2: 4.5:1 for text, 3:1 for essential non-text). They are not a test with a screen reader or of an accessibility tree, and the gallery has had neither.

The **Motion** page lists the durations and the easing curve and drives the real switch, tab indicator, progress bar, popover and dialog. Hover and press change at once, since GPUI has no style transitions (`docs/spec/odk-interaction.md`).

## Playground

Choose a button's variant, emphasis, density, icon placement, disabled and loading, or a form control (Input, Checkbox, Switch, Select) with the properties that apply to it. The component above the code is the real one and changes at once; the Rust below it is written from the same properties, line by line, and **Copy** puts it on the clipboard. Corners are the whole gallery's.

## Theme comparison

Pick two installed themes. Each is loaded as a snapshot of its own, resolved the same way. The page shows the component tokens of every button variant built for each, and a table of the surface, text, border and role tokens with both values, whether they differ, and how many do. Because ODK's components read one process-wide theme, real components cannot be drawn in two themes at once; **Show A** and **Show B** preview each in turn so the real components below, and the whole gallery, are drawn in it, and **System theme** returns. Neither touches Omarchy's theme.

## Adding to the gallery

A new component or demonstration provides:

1. A page in `registry.rs`: id, title, group, description, the components it shows, keywords, token names (and the sidebar, the breadcrumb, the Overview and search all follow). Add a `Page` variant, its `ALL` entry, and its arms in `Gallery::view` and `observe_page`.
2. A view that draws real components, wrapped in `crate::page(Page::X, body)`, which gives it its title, description, usage example and notes. Keep state in the view, not in globals, so it survives appearance changes. Draw only theme tokens: no colour literal, no call into the colour maths or the resolver (a test scans the source).
3. Its relevant variants, densities and states, as real controls. No forced hover or focus: the real pointer and keyboard do those, and the inspector shows each state's tokens.
4. Token inspector integration: `diagnostics::panel` with the component names of `StyleDiagnostic::component`, or `row_lines` for the rows of one style.
5. A usage example as a compiled file `src/snippets/<name>.rs`, between `// snippet start` and `// snippet end`, listed in `snippets.rs`. It builds with the gallery, so it cannot name an API that is not there.
6. Keyboard and accessibility notes in the page's table entry.
7. Tests: a page needs no new test to be drawn (the performance tests draw every page and hold it to a quarter of a frame, idle, and no frame requests; `every_page_draws_in_every_fixture_theme` draws it in five themes); add behaviour tests in `examples/gallery/tests/` for what is interactive, and break the rule once by hand to see them fail.

A page that is not interactive does not need interaction states.

```rust
// registry.rs: the page's entry (shortened)
Page::Spacing => &Meta {
    id: "spacing",
    title: "Spacing",
    category: Category::Foundation,
    description: "The 4 px spacing scale and the spacing named for a purpose.",
    components: &[],
    keywords: &["space", "gap", "padding"],
    tokens: &[],
    accessibility: "",
},

// spacing.rs: the view
impl Render for SpacingPage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let f = odk::ui::foundation();
        crate::page(Page::Spacing, section("Scale").child(/* … */))
    }
}
```

## Limits

- Real components in two themes at once are not possible without scoping the theme store to a subtree; comparison is side by side as tokens and one after the other as components.
- Forced hover, press and focus on a live control would need a forcing hook in `Button`'s public API, which was not added.
- GPUI's text cannot be selected, so code is taken with **Copy**; it is not syntax-highlighted.
- Sidebar groups do not collapse and there is no docked inspector: the sidebar scrolls and each page carries its tokens.
- The gallery has not been used with a screen reader. Hover and press under a real pointer, and IME in the fields, are checked by hand (`docs/checks.md`).
- `GALLERY_THEME` and the selector name themes by the directory names Omarchy uses; a user theme that only overlays some files of a stock theme is listed under its name.
