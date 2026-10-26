# ODK

The Omarchy Development Kit and the `.oma` application ecosystem built around it: how Omarchy applications are written, packaged, installed, launched and updated.

## Language

**Omarchy**:
The Linux desktop environment (Arch Linux, Hyprland, Wayland) that ODK and OMA exist for.

**ODK**:
The Omarchy Development Kit: the Rust libraries, embedded Luau machine and UI layer used to build Omarchy applications. Separate from OMA: an ODK application can be packaged as a Bundle, but oma does not depend on ODK.
_Avoid_: SDK, framework

**OMA**:
Omarchy Managed Application: the application bundle format and the ecosystem around it.

**Bundle**:
An application packaged as one `<App>.oma` file (ADR 0052): a read-only EROFS image of its Manifest, executable and resources, mounted rather than extracted. The code still installs a directory `<App>.oma/` until it catches up.
_Avoid_: package, archive, app image

**Publisher**:
The domain a Bundle's Manifest names. That domain's list of signing keys is the root of trust for the Bundle's signature (ADR 0055). A domain that was unreachable and comes back with a different key is a new publisher.

**Manifest**:
The declarative TOML file inside a Bundle that names the application and declares the permissions it requests.

**Identifier**:
The permanent reverse-DNS name (such as `com.example.foo`) that makes Bundles the same application at different versions.
_Avoid_: app name, bundle id

**Consent record**:
What the user has agreed to let a given application do, kept per Identifier. A launch that requests anything outside it asks again.

**oma**:
The program that runs a valid Bundle: it verifies, installs, registers, sandboxes and starts it, and can assemble a Bundle from an already built executable. It knows nothing about how the Bundle's application was built.
Called `omalaunch` in ADRs 0001 to 0024.
_Avoid_: the launcher (ambiguous with Omarchy's own application launcher), OMA runtime, omalaunch

**Plugin**:
A Luau extension loaded by an ODK application to add or change behaviour, without changing the application's Rust code.

**oma-consent**:
The small helper program that shows the consent dialog for oma and reports the user's answer. It decides nothing and holds no state. Called `omagate` in ADR 0026 and `omalaunch-consent` in the first draft of ADR 0025. `oma consent <target>` runs it.

**Inbox**:
The per-application directory where an application stages a new version of itself for oma to pick up. An application can write only to its own.

**Flea**:
The file explorer that presents a Bundle as a single application. It runs the file on open and mounts it for "Show contents" (ADR 0053). The code still describes the Bundle directory until it catches up.

**Theme**:
An Omarchy theme: a named set of colours and settings that Omarchy applies to the whole desktop. Stock themes ship with Omarchy; users install or write their own.
_Avoid_: skin, colour scheme

**Current theme**:
The theme Omarchy has selected for the session. ODK applications follow it as it changes.
_Avoid_: active theme, system theme

**Palette**:
The colours a theme names, by Omarchy's names (`background`, `red`, `bright_red`, ...) and with the values the theme wrote, before ODK gives any of them a meaning. The source layer of ADR 0040.
_Avoid_: colour scheme, raw colours

**Text size**:
How large the user wants text on their desktop, as Omarchy records it (`omarchy display text size`), which ODK applications follow: type, the controls around it and the space between them grow and shrink together, and nothing about how a control is drawn does. Twelve pixels is the size ODK's own sizes are specified at.
_Avoid_: font size (that is one size in the type scale), zoom, scale factor (that is the display's)

**Emergency palette**:
ODK's own fixed grey palette, drawn only when no theme has loaded since the application started. Not a theme.
_Avoid_: default theme, fallback theme

**Role**:
What a colour means in the interface: accent, neutral, danger, success, warning or info. A Palette colour has no role until the resolver gives it one, and in a monochrome theme two roles may share a grey.
_Avoid_: tone, status, semantic colour (for the role itself)

**Emphasis**:
How prominently a Role shows: solid (a filled background), muted (a tint with text in the role's colour) or subtle (no fill until hovered). Independent of which Palette colour plays the Role.
_Avoid_: variant, intensity, bright (a Palette name, not an emphasis)

**Treatment**:
The background, foreground and border one Role is drawn with at one Emphasis in one interaction state.
_Avoid_: style, skin

**Resolver**:
The part of ODK that turns a Palette into colours by Role and Emphasis, choosing among the Palette's colours and adjusting their lightness only as far as readability requires.
_Avoid_: colour mapper, theme engine

**Highlight**:
The item of a list or menu the keyboard is on. Not the selected item: one item can be highlighted while another is selected, and the pointer's hover is a third thing.
_Avoid_: focus (for an item), active item, cursor

**Focus ring**:
The mark drawn around a control while it has keyboard focus, outside the control and over whatever state it is in.
_Avoid_: focus state, outline, focus border

**Density**:
How tightly an application packs its controls: compact, default or comfortable. It sets how high a control is and how much room its content has, never a colour; a control may have its own.
_Avoid_: size (for the application's choice), scale, zoom

**Field**:
The box a value is typed into or a choice is shown in, as high as a button of its Density: a text input, a text area, and the closed box of a select.
_Avoid_: form field (for the box alone), text box, entry, control (for a field in particular)

**Form field**:
A control in a form together with its label, an optional description and an optional message, such as an error.
_Avoid_: field (for the whole), form item, form row

**Field action**:
A button drawn inside a Field, lower than it, that acts on the Field's value: clearing it, showing a password, stepping a number.
_Avoid_: adornment, addon (for a button), inline button

**Status mark**:
The character that says which status a notice is in, `i`, `✓`, `!` or `×` for info, success, warning and danger, so the status never rests on its colour.
_Avoid_: status icon, tone icon

**Status indicator**:
A small dot in a Role's colour beside a label that says what state something is in, such as "Online". The label is always shown; an inactive state is a hollow ring rather than a dot.
_Avoid_: status dot (for the whole), badge (a label in a box), light

**Current page**:
The page a navigation shows as where the user is: the last step of a breadcrumb, the marked row of a Navigation list, the selected tab's panel. Not a Highlight and not focus: the keyboard can be elsewhere.
_Avoid_: active item, active route

**Navigation list**:
A sidebar's rows, each of which opens a page, optionally under group headings, one of them the Current page. Moving through it with the keyboard opens nothing until a row is chosen.
_Avoid_: sidebar (the region it sits in), nav menu, tree

**Progress bar**:
A bar that shows how much of a piece of work is done. Work whose length is unknown has a spinner instead.
_Avoid_: progress indicator (for both), loader

**Skeleton**:
A still placeholder in the shape and size of content that has not arrived yet, so nothing moves when it does.
_Avoid_: shimmer, loading placeholder, placeholder (a field's hint text)

**Popover**:
Interactive content in a panel beside the control that opened it, which takes focus but does not block the window. A hover card or tooltip is not one: they show on hover and take no focus.
_Avoid_: popup, flyout, dropdown (a dropdown is a menu)

**Side panel**:
A sheet that does not block the window: no scrim, no trapped focus, the rest of the window stays live. A sheet that blocks the window is a modal sheet.
_Avoid_: drawer, non-modal sheet

**Dismissal**:
What closes an overlay without choosing anything in it: Escape, a press outside it, a click on its scrim. Each overlay says which of them it has.
_Avoid_: cancel, close policy

**Database**:
An application's own SQLite file, in its own data directory, which ODK opens, brings up to date with the application's migrations and gives the application as a cheap handle that never blocks the window (`odk::db`, the `db` feature of `odk`, off by default, ADR 0049). Not shared between applications.
_Avoid_: store, DB layer

**Reduced motion**:
A request that nothing move for its own sake: every transition shows its end state at once and a spinner stands still, while every state change still happens. The application can ask for it, and so can the user, by turning off the compositor's window animations (Hyprland's `animations:enabled`, which `odkd` reads and tells every application); either one is enough.
_Avoid_: no animation, prefers-reduced-motion (a browser's name for it)

**Activation**:
What a control reports when a press and its release both land on it, or a key that clicks it is released: one callback, once. A press that ends elsewhere, a key held down, and anything done to a disabled or busy control are not activations.
_Avoid_: click (for a key too), trigger

**Theme preview**:
A theme an ODK application draws in place of the one it follows, without choosing it: Omarchy's selection and every theme file stay as they are, and the followed theme keeps loading underneath, to be drawn again when the preview ends (`odk::ui::preview_theme`, `docs/spec/odk-source-palette.md`). The gallery's theme selector makes one.
_Avoid_: theme switch (that changes Omarchy's theme), override

**Gallery**:
`examples/gallery`: ODK's design-system explorer, an application that shows every component in each state, the foundation tokens and the resolver's reasoning, in the user's theme or a preview (`docs/gallery.md`).
