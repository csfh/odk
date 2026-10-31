//! The form pages: every form control in each of its states, at the density and corners the
//! toolbar sets, a complete form built from `FormField`s, and the diagnostics of one control
//! picked with the controls, read from the component tokens. Everything here is a real control
//! in the active Omarchy theme; hover it with the pointer and Tab to see its ring.

use odk::gpui::px;
use odk::prelude::*;

use crate::buttons::{caption, heights, row};
use crate::content::Content;
use crate::{INPUT_WIDTH, Settings, diagnostics};

/// The three pages the forms are shown on: one page of everything costs more than a frame
/// (`benches/pages.rs`), so each part is a page of its own.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FormsPart {
    /// Every text field.
    Fields,
    /// The selection controls, and every control at each density.
    Selection,
    /// The controls' states, a complete form, and their diagnostics.
    Form,
}

/// A page of the forms: its own state, and the Content switch it follows.
pub struct FormsPage {
    part: FormsPart,
    worst_case: bool,
    name: SharedString,
    country: SharedString,
    form: FormState,
    inspector: diagnostics::Inspector,
    diagnostics: diagnostics::Rows,
}

impl FormsPage {
    pub fn new(settings: &Entity<Settings>, part: FormsPart, cx: &mut Context<Self>) -> Self {
        cx.observe(settings, |this, settings, cx| {
            this.worst_case = settings.read(cx).worst_case;
            cx.notify();
        })
        .detach();
        Self {
            part,
            worst_case: settings.read(cx).worst_case,
            name: SharedString::default(),
            country: SharedString::default(),
            form: FormState::default(),
            inspector: diagnostics::Inspector::new(match part {
                FormsPart::Selection => "select",
                _ => "field",
            }),
            diagnostics: diagnostics::Rows::default(),
        }
    }
}

impl Render for FormsPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = Content::get(self.worst_case);
        let page = match self.part {
            FormsPart::Fields => crate::Page::Fields,
            FormsPart::Selection => crate::Page::Selection,
            FormsPart::Form => crate::Page::Form,
        };
        crate::page(page, section(self, &c, cx))
    }
}

/// What the example form holds.
#[derive(Default)]
pub struct FormState {
    name: SharedString,
    email: SharedString,
    plan: SharedString,
    seats: Option<f64>,
    agree: bool,
    /// Whether Create was pressed, after which missing answers show their errors.
    sent: bool,
    done: bool,
}

const FIELD_COMPONENTS: [&str; 4] = ["field", "field action", "form field", "form message"];

const SELECTION_COMPONENTS: [&str; 5] = ["select", "checkbox", "radio", "switch", "menu item"];

const COMPONENTS: [&str; 9] = [
    "field",
    "select",
    "field action",
    "checkbox",
    "radio",
    "switch",
    "menu item",
    "form field",
    "form message",
];

fn group(title: &'static str) -> impl IntoElement {
    Heading::new(title).level(3)
}

/// A column as wide as a form's at most. It keeps its content's height in the scrolling page.
/// Not `w_full` as well: GPUI 0.2.2 measures wrapping text in a column with both at the full
/// width, so the column comes out shorter than its content (`docs/checks.md`).
fn column() -> Stack {
    Stack::new()
        .flex_shrink_0()
        .gap(Space::Md)
        .max_w(px(INPUT_WIDTH))
}

/// The section.
pub fn section(g: &mut FormsPage, c: &Content, cx: &mut Context<FormsPage>) -> Stack {
    // Uncontrolled controls keep their own state, so the Content switch gives them new ids.
    let k = usize::from(g.worst_case);
    Stack::new()
        .flex_shrink_0()
        .gap(Space::Lg)
        .child(caption(
            "Every control follows the density and corners chosen in the toolbar. Tab to see a ring; hover and press with the pointer.",
        ))
        .children(match g.part {
            FormsPart::Fields => vec![
                group("Text fields").into_any_element(),
                text_fields(g, c, k, cx).into_any_element(),
                group("Diagnostics").into_any_element(),
                caption("The tokens a text field draws with, where each came from, and its contrast.")
                    .into_any_element(),
                diagnostics::panel(
                    g,
                    "fields",
                    &FIELD_COMPONENTS,
                    |g| &mut g.diagnostics,
                    |g| &mut g.inspector,
                    cx,
                )
                .into_any_element(),
            ],
            FormsPart::Selection => vec![
                group("Selection").into_any_element(),
                selection(g, c, k, cx).into_any_element(),
                group("Densities").into_any_element(),
                caption(format!(
                    "Buttons, fields and selects share one height: {}.",
                    heights()
                ))
                    .into_any_element(),
                Stack::new()
                    .gap(Space::Lg)
                    .children(Density::ALL.iter().enumerate().map(|(n, &d)| density_row(n, d)))
                    .into_any_element(),
                group("Diagnostics").into_any_element(),
                caption("The tokens a selection control draws with, where each came from, and its contrast.")
                    .into_any_element(),
                diagnostics::panel(
                    g,
                    "selection",
                    &SELECTION_COMPONENTS,
                    |g| &mut g.diagnostics,
                    |g| &mut g.inspector,
                    cx,
                )
                .into_any_element(),
            ],
            FormsPart::Form => vec![
                group("States").into_any_element(),
                states(k).into_any_element(),
                group("A form").into_any_element(),
                example_form(g, c, k, cx).into_any_element(),
                group("Diagnostics").into_any_element(),
                caption("The tokens one control draws with, where each came from, and its contrast.")
                    .into_any_element(),
                diagnostics::panel(
                    g,
                    "forms",
                    &COMPONENTS,
                    |g| &mut g.diagnostics,
                    |g| &mut g.inspector,
                    cx,
                )
                .into_any_element(),
            ],
        })
}

fn text_fields(g: &mut FormsPage, c: &Content, k: usize, cx: &mut Context<FormsPage>) -> Stack {
    let [placeholder, value, read_only, disabled, echo] = c.inputs;
    let [area_placeholder, area_value, area_read_only] = c.textareas;
    let [before, after, group_placeholder, copy] = c.groups;
    column()
        .child(
            FormField::new("f-name", "Standard").child(
                Input::new("name", "Name")
                    .placeholder(placeholder)
                    .value(g.name.clone())
                    .on_change(cx.listener(|this, text: &str, _, cx| {
                        this.name = text.to_owned().into();
                        cx.notify();
                    })),
            ),
        )
        .child(Text::new(format!("{echo} {}", g.name)).muted())
        .child(
            FormField::new("f-filled", "Filled")
                .child(Input::new(("filled", k), "File name").default_value(value)),
        )
        .child(
            FormField::new("f-lead", "Leading icon").child(
                Input::new(("lead", k), "Handle")
                    .leading_icon(Icon::At)
                    .placeholder("username"),
            ),
        )
        .child(
            FormField::new("f-trail", "Trailing icon").child(
                Input::new(("trail", k), "Discount")
                    .trailing_icon(Icon::Percent)
                    .placeholder("0"),
            ),
        )
        .child(
            FormField::new("f-clear", "Clearable").child(
                Input::new(("clear", k), "Tag")
                    .default_value(value)
                    .clearable(true),
            ),
        )
        .child(
            FormField::new("f-search", "Search")
                .help("Escape clears it; Enter searches")
                .child(
                    SearchInput::new(("search", k), "Search settings")
                        .placeholder("Search settings…")
                        .shortcut(c.chords[0]),
                ),
        )
        .child(
            FormField::new("f-searching", "Search, loading").child(
                SearchInput::new(("searching", k), "Search files")
                    .default_value("report")
                    .loading(true),
            ),
        )
        .child(
            FormField::new("f-password", "Password")
                .description("The eye at the end shows it; it never reaches the clipboard")
                .child(PasswordInput::new(("password", k), "Password").default_value("hunter2")),
        )
        .child(
            FormField::new("f-number", "Number")
                .help("Up and Down step; 0 to 99")
                .child(
                    NumberInput::new(("number", k), "Quantity")
                        .min(0.0)
                        .max(99.0)
                        .default_value(3.0),
                ),
        )
        .child(
            FormField::new("f-decimal", "Number, in steps of 0.25").child(
                NumberInput::new(("decimal", k), "Gain")
                    .step(0.25)
                    .min(-1.0)
                    .max(1.0)
                    .default_value(0.5),
            ),
        )
        .child(
            FormField::new("f-area", "Textarea")
                .child(Textarea::new(("area", k), "What happened").placeholder(area_placeholder)),
        )
        .child(
            FormField::new("f-grow", "Textarea that grows")
                .help("From two rows up to five, then it scrolls")
                .child(
                    Textarea::new(("grow", k), "Notes")
                        .rows(2)
                        .max_rows(5)
                        .default_value(area_value),
                ),
        )
        .child(
            FormField::new("f-area-ro", "Textarea, read-only").child(
                Textarea::new(("area-ro", k), "Readme")
                    .rows(2)
                    .default_value(area_read_only)
                    .read_only(true),
            ),
        )
        .child(
            FormField::new("f-ro", "Read-only").child(
                Input::new(("read-only", k), "Location")
                    .default_value(read_only)
                    .read_only(true),
            ),
        )
        .child(
            FormField::new("f-off", "Disabled").child(
                Input::new(("disabled", k), "Owner")
                    .default_value(disabled)
                    .disabled(true),
            ),
        )
        .child(
            FormField::new("f-group", "Input group").child(
                InputGroup::new(("group-site", k), "Website")
                    .prefix(before)
                    .suffix(after)
                    .placeholder(group_placeholder),
            ),
        )
        .child(
            FormField::new("f-group-copy", "Input group with an action").child(
                InputGroup::new(("group-copy", k), "Share link")
                    .default_value("https://example.com/share/4f9a")
                    .read_only(true)
                    .action(Button::new(("group-copy-button", k)).label(copy)),
            ),
        )
        .child(
            FormField::new("f-group-off", "Input group, disabled").child(
                InputGroup::new(("group-off", k), "Locked")
                    .default_value("locked")
                    .prefix(before)
                    .disabled(true)
                    .action(Button::new(("group-off-button", k)).label(copy)),
            ),
        )
}

fn selection(g: &mut FormsPage, c: &Content, k: usize, cx: &mut Context<FormsPage>) -> Stack {
    let (placeholder, [first, second, third, fourth], echo) = c.select;
    let options = move |select: Select| {
        select
            .item("first", first)
            .item("second", second)
            .item("third", third)
            .item("fourth", fourth)
            .disable("third")
    };
    let [hidden, wrap, all, managed] = c.checkboxes;
    let [wifi, bluetooth, airplane] = c.switches;
    let radios = move |id: (&'static str, usize)| {
        RadioGroup::new(id, "Option")
            .item("first", first)
            .item("second", second)
            .item("third", third)
            .disable("third")
    };
    column()
        .child(
            FormField::new("f-country", "Select").child(options(
                Select::new("country", "Country")
                    .placeholder(placeholder)
                    .value(g.country.clone())
                    .on_change(cx.listener(|this, value: &str, _, cx| {
                        this.country = value.to_owned().into();
                        cx.notify();
                    })),
            )),
        )
        .child(Text::new(format!("{echo} {}", g.country)).muted())
        .child(FormField::new("f-chosen", "Select, chosen").child(options(
            Select::new(("chosen", k), "Region").default_value("second"),
        )))
        .child(FormField::new("f-many", "Select with a long list").child(
            (0..60).fold(
                Select::new("many", "Number").default_value("n20"),
                |select, i| select.item(format!("n{i}"), format!("Number {i}")),
            ),
        ))
        .child(FormField::new("f-select-off", "Select, disabled").child(options(
            Select::new(("off", k), "Owner")
                .default_value("first")
                .disabled(true),
        )))
        .child(caption(
            "Combobox: ODK has none yet; the command palette (below) is its searchable list, on the same field and menu tokens.",
        ))
        .child(
            Stack::new()
                .gap(Space::Sm)
                .child(Checkbox::new(("hidden", k), hidden))
                .child(Checkbox::new(("wrap", k), wrap).default_checked(true))
                .child(Checkbox::new(("all", k), all).checked(false).indeterminate(true))
                .child(
                    Checkbox::new(("managed", k), managed)
                        .checked(true)
                        .disabled(true),
                ),
        )
        .child(
            FormField::new("f-radio", "Radio group")
                .description("The arrow keys move and choose; the third is disabled")
                .child(radios(("radio", k)).default_value("first")),
        )
        .child(FormField::new("f-radio-row", "Radio group in a row").child(
            radios(("radio-row", k)).horizontal(true).default_value("second"),
        ))
        .child(
            Stack::new()
                .gap(Space::Sm)
                .child(Switch::new(("wifi", k), wifi).default_checked(true))
                .child(Switch::new(("bluetooth", k), bluetooth))
                .child(Switch::new(("airplane", k), airplane).disabled(true))
                .child(
                    Switch::new(("airplane-on", k), "Location (managed)")
                        .default_checked(true)
                        .disabled(true),
                ),
        )
}

fn density_row(n: usize, d: Density) -> Row {
    row()
        .items_center()
        .child(
            Button::new(("dr-button", n))
                .label("Save")
                .variant(ButtonVariant::Primary)
                .density(d),
        )
        .child(
            div().w(px(150.0)).child(
                Input::new(("dr-input", n), "Name")
                    .placeholder("Name")
                    .density(d),
            ),
        )
        .child(
            div().w(px(150.0)).child(
                Select::new(("dr-select", n), "Fruit")
                    .placeholder("Fruit")
                    .item("apple", "Apple")
                    .density(d),
            ),
        )
        .child(
            div().w(px(170.0)).child(
                SearchInput::new(("dr-search", n), "Search")
                    .default_value("dock")
                    .density(d),
            ),
        )
        .child(
            div().w(px(130.0)).child(
                NumberInput::new(("dr-number", n), "Quantity")
                    .default_value(1.0)
                    .density(d),
            ),
        )
        .child(
            Checkbox::new(("dr-check", n), "Check")
                .default_checked(true)
                .density(d),
        )
        .child(
            RadioGroup::new(("dr-radio", n), "Radio")
                .item("a", "Radio")
                .default_value("a")
                .density(d),
        )
        .child(
            Switch::new(("dr-switch", n), "Switch")
                .default_checked(true)
                .density(d),
        )
        .child(caption(format!("{} {} px", d.name(), d.metrics().height)))
}

fn states(k: usize) -> Stack {
    let state = |label: &'static str, input: Input| {
        Stack::new()
            .gap(Space::Xs)
            .w(px(170.0))
            .child(caption(label))
            .child(input)
    };
    Stack::new()
        .gap(Space::Md)
        .child(
            row()
                .child(state("Default", Input::new(("st-default", k), "Default").placeholder("Text")))
                .child(state(
                    "Invalid",
                    Input::new(("st-invalid", k), "Invalid")
                        .default_value("not an address")
                        .invalid(true),
                ))
                .child(state(
                    "Read-only",
                    Input::new(("st-ro", k), "Read-only")
                        .default_value("read only")
                        .read_only(true),
                ))
                .child(state(
                    "Disabled",
                    Input::new(("st-off", k), "Disabled")
                        .default_value("disabled")
                        .disabled(true),
                )),
        )
        .child(caption(
            "Focus and invalid compose: Tab into the invalid field and it keeps its danger edge, with the danger ring outside it.",
        ))
        .child(
            row()
                .child(
                    div().w(px(170.0)).child(
                        Select::new(("st-select", k), "Fruit")
                            .item("a", "Apple")
                            .default_value("a")
                            .invalid(true),
                    ),
                )
                .child(Checkbox::new(("st-check", k), "Invalid").invalid(true))
                .child(
                    RadioGroup::new(("st-radio", k), "Invalid")
                        .item("a", "Invalid")
                        .invalid(true),
                )
                .child(Checkbox::new(("st-checked", k), "Checked").default_checked(true))
                .child(Switch::new(("st-on", k), "On").default_checked(true)),
        )
}

fn example_form(g: &mut FormsPage, c: &Content, k: usize, cx: &mut Context<FormsPage>) -> Stack {
    let t = &c.form;
    let f = &g.form;
    let email_wrong = f.sent && !f.email.contains('@');
    let name_missing = f.sent && f.name.trim().is_empty();
    let agree_missing = f.sent && !f.agree;
    let [cancel, create] = t.actions;

    let mut name = FormField::new("ex-name", t.name.0)
        .required(true)
        .description(t.name.1)
        .child(
            Input::new(("ex-name-input", k), t.name.0)
                .value(f.name.clone())
                .on_change(cx.listener(|this, text: &str, _, cx| {
                    this.form.name = text.to_owned().into();
                    cx.notify();
                })),
        );
    if name_missing {
        name = name.error("Enter your name");
    }
    let mut email = FormField::new("ex-email", t.email.0).required(true).child(
        Input::new(("ex-email-input", k), t.email.0)
            .placeholder(t.email.1)
            .leading_icon(Icon::At)
            .value(f.email.clone())
            .on_change(cx.listener(|this, text: &str, _, cx| {
                this.form.email = text.to_owned().into();
                cx.notify();
            })),
    );
    if email_wrong {
        email = email.error(t.email.2);
    } else if f.email.contains('@') {
        email = email.success("Looks right");
    }
    let mut agree = FormField::new("ex-agree", "Terms").required(true).child(
        Checkbox::new(("ex-agree-box", k), t.agree.0)
            .checked(f.agree)
            .on_change({
                let gallery = cx.entity();
                move |on, _, cx| {
                    gallery.update(cx, |g, cx| {
                        g.form.agree = on;
                        cx.notify();
                    })
                }
            }),
    );
    if agree_missing {
        agree = agree.error(t.agree.1);
    }
    let [personal, team, enterprise] = t.plan.1;
    let plan_value = if f.plan.is_empty() {
        SharedString::from("personal")
    } else {
        f.plan.clone()
    };

    let mut form = odk::ui::form()
        .max_w(px(INPUT_WIDTH * 1.25))
        .p(Space::Lg)
        .border_1()
        .border_color(odk::gpui::rgba(
            odk::ui::theme_snapshot()
                .semantic
                .border
                .subtle
                .to_rgba_u32(),
        ))
        .child(name)
        .child(email)
        .child(
            FormField::new("ex-plan", t.plan.0).required(true).child(
                RadioGroup::new(("ex-plan-group", k), t.plan.0)
                    .item("personal", personal)
                    .item("team", team)
                    .item("enterprise", enterprise)
                    .disable("enterprise")
                    .value(plan_value)
                    .on_change(cx.listener(|this, value: &str, _, cx| {
                        this.form.plan = value.to_owned().into();
                        cx.notify();
                    })),
            ),
        )
        .child(
            FormField::new("ex-seats", t.seats.0).help(t.seats.1).child(
                NumberInput::new(("ex-seats-input", k), t.seats.0)
                    .min(1.0)
                    .max(25.0)
                    .value(f.seats.unwrap_or(1.0))
                    .on_change({
                        let gallery = cx.entity();
                        move |n, _, cx| {
                            if n.is_some() {
                                gallery.update(cx, |g, cx| {
                                    g.form.seats = n;
                                    cx.notify();
                                })
                            }
                        }
                    }),
            ),
        )
        .child(
            FormField::new("ex-notes", t.notes.0)
                .optional(true)
                .warning(t.notes.1)
                .child(Textarea::new(("ex-notes-input", k), t.notes.0).rows(2)),
        )
        .child(agree)
        .child(
            Row::new()
                .flex_wrap()
                .gap(Space::Sm)
                .child(Button::new("ex-cancel").label(cancel).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.form = FormState::default();
                        cx.notify();
                    },
                )))
                .child(
                    Button::new("ex-create")
                        .label(create)
                        .variant(ButtonVariant::Primary)
                        .on_click(cx.listener(|this, _, _, cx| {
                            let f = &mut this.form;
                            f.sent = true;
                            f.done = !f.name.trim().is_empty() && f.email.contains('@') && f.agree;
                            cx.notify();
                        })),
                ),
        );
    if f.done {
        form = form.child(Alert::new("ex-done", t.done).variant(Tone::Success));
    }
    form
}
