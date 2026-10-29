//! The Playground page: a real button and a real form control whose properties are set with
//! controls beside them, with the Rust that builds what is shown. The code is made from the same
//! state that builds the component, line by line, so it names only methods that exist; the
//! gallery's tests check each line against the component's behaviour.

use odk::gpui::SharedString;
use odk::prelude::*;
use odk::theme::foundation::CornerRadius;

use crate::buttons::{choice, choice_labelled, row};
use crate::present::{code_panel_owned, flag, group, page, section};
use crate::registry::Page;

/// Where a button's icon goes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Content {
    Text,
    Leading,
    Trailing,
    IconOnly,
}

/// Which form control is on the bench.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Control {
    Input,
    Checkbox,
    Switch,
    Select,
}

/// A button's properties.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ButtonProps {
    pub variant: ButtonVariant,
    pub emphasis: Option<Emphasis>,
    pub density: Option<Density>,
    pub content: Content,
    pub disabled: bool,
    pub loading: bool,
}

impl Default for ButtonProps {
    fn default() -> Self {
        Self {
            variant: ButtonVariant::Primary,
            emphasis: None,
            density: None,
            content: Content::Text,
            disabled: false,
            loading: false,
        }
    }
}

impl ButtonProps {
    /// The button these properties describe.
    pub fn build(&self, id: &'static str) -> Button {
        let mut button = match self.content {
            Content::Text => Button::new(id).label("Save changes"),
            Content::Leading => Button::new(id)
                .leading_icon(Icon::Check)
                .label("Save changes"),
            Content::Trailing => Button::new(id)
                .label("Save changes")
                .trailing_icon(Icon::ArrowRight),
            Content::IconOnly => Button::icon(id, "⚙", "Settings"),
        }
        .variant(self.variant);
        if let Some(e) = self.emphasis {
            button = button.emphasis(e);
        }
        if let Some(d) = self.density {
            button = button.density(d);
        }
        button.loading(self.loading).disabled(self.disabled)
    }

    /// The Rust that builds the same button, a line for each call `build` makes.
    pub fn code(&self) -> String {
        let mut lines = vec![match self.content {
            Content::Text => "Button::new(\"save\")\n    .label(\"Save changes\")".to_owned(),
            Content::Leading => {
                "Button::new(\"save\")\n    .leading_icon(\"✓\")\n    .label(\"Save changes\")"
                    .to_owned()
            }
            Content::Trailing => {
                "Button::new(\"save\")\n    .label(\"Save changes\")\n    .trailing_icon(\"→\")"
                    .to_owned()
            }
            Content::IconOnly => "Button::icon(\"save\", \"⚙\", \"Settings\")".to_owned(),
        }];
        lines.push(format!("    .variant(ButtonVariant::{:?})", self.variant));
        if let Some(e) = self.emphasis {
            lines.push(format!("    .emphasis(Emphasis::{e:?})"));
        }
        if let Some(d) = self.density {
            lines.push(format!("    .density(Density::{d:?})"));
        }
        if self.loading {
            lines.push("    .loading(true)".into());
        }
        if self.disabled {
            lines.push("    .disabled(true)".into());
        }
        lines.join("\n")
    }
}

/// A form control's properties.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FieldProps {
    pub control: Control,
    pub invalid: bool,
    pub disabled: bool,
    pub read_only: bool,
    pub required: bool,
    pub checked: bool,
}

impl Default for FieldProps {
    fn default() -> Self {
        Self {
            control: Control::Input,
            invalid: false,
            disabled: false,
            read_only: false,
            required: false,
            checked: true,
        }
    }
}

impl FieldProps {
    /// The properties that mean something for the control on the bench.
    pub fn applies(&self, property: &str) -> bool {
        matches!(
            (self.control, property),
            (_, "disabled")
                | (Control::Input, "invalid" | "read_only" | "required")
                | (Control::Select, "invalid" | "required")
                | (Control::Checkbox, "invalid" | "checked")
                | (Control::Switch, "checked")
        )
    }

    pub fn code(&self) -> String {
        let mut calls: Vec<String> = Vec::new();
        let head = match self.control {
            Control::Input => "Input::new(\"name\", \"Name\")",
            Control::Checkbox => "Checkbox::new(\"terms\", \"I accept the terms\")",
            Control::Switch => "Switch::new(\"wifi\", \"Wi-Fi\")",
            Control::Select => "Select::new(\"region\", \"Region\")",
        };
        if self.control == Control::Select {
            calls.push(".item(\"a\", \"Region A\")".into());
        }
        if self.applies("checked") && self.checked {
            calls.push(".checked(true)".into());
        }
        if self.applies("read_only") && self.read_only {
            calls.push(".read_only(true)".into());
        }
        if self.applies("invalid") && self.invalid {
            calls.push(".invalid(true)".into());
        }
        if self.applies("required") && self.required && self.control != Control::Input {
            calls.push(".required(true)".into());
        }
        if self.disabled {
            calls.push(".disabled(true)".into());
        }
        let mut code = String::from(head);
        for call in calls {
            code.push_str("\n    ");
            code.push_str(&call);
        }
        if self.control == Control::Input && self.required {
            code = format!(
                "FormField::new(\"name-field\", \"Name\")\n    .required(true)\n    .child({})",
                code.replace('\n', "\n    ")
            );
        }
        code
    }
}

pub struct PlaygroundPage {
    pub button: ButtonProps,
    pub field: FieldProps,
    diagnostics: crate::diagnostics::Rows,
}

impl PlaygroundPage {
    pub fn new() -> Self {
        Self {
            button: ButtonProps::default(),
            field: FieldProps::default(),
            diagnostics: crate::diagnostics::Rows::default(),
        }
    }
}

impl PlaygroundPage {
    /// The component-token diagnostics of the button and of the control on the bench: only the
    /// styles of the one being shown, at the density it has.
    fn tokens_view(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let tokens = odk::ui::components();
        let rows = self.diagnostics.get(&tokens);
        let config = odk::ui::foundation_config(cx);
        let b = self.button;
        let appearance = match b.emphasis {
            Some(e) => b.variant.appearance().with_emphasis(e),
            None => b.variant.appearance(),
        };
        let density = b.density.unwrap_or(config.density);
        let mut view = v_flex().gap(Space::Md).child(group("The button"));
        view = view.child(Text::new(format!(
            "{} at {} density: {} px high",
            appearance.name(),
            density.name(),
            odk::ui::foundation().controls.for_density(density).height
        )));
        for row in rows.iter().filter(|r| {
            r.component == "button" && r.variant == appearance.name() && r.density == Some(density)
        }) {
            view = view.child(crate::diagnostics::row_lines(row));
        }
        let component = match self.field.control {
            Control::Input => "field",
            Control::Checkbox => "checkbox",
            Control::Switch => "switch",
            Control::Select => "select",
        };
        view = view.child(group("The control"));
        for row in rows
            .iter()
            .filter(|r| r.component == component && r.density.is_none_or(|d| d == config.density))
        {
            view = view.child(crate::diagnostics::row_lines(row));
        }
        view
    }
}

impl Default for PlaygroundPage {
    fn default() -> Self {
        Self::new()
    }
}

fn toggle(
    id: &'static str,
    label: &'static str,
    value: bool,
    set: impl Fn(&mut PlaygroundPage, bool) + 'static,
    cx: &mut Context<PlaygroundPage>,
) -> Checkbox {
    Checkbox::new(id, label).checked(value).on_change(flag(
        cx,
        move |this: &mut PlaygroundPage, on, cx| {
            set(this, on);
            cx.notify();
        },
    ))
}

impl Render for PlaygroundPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let config = odk::ui::foundation_config(cx);
        let b = self.button;
        let mut emphases = vec![(None, "variant's own")];
        emphases.extend(Emphasis::ALL.map(|e| (Some(e), e.name())));
        let mut densities = vec![(None, "the application's")];
        densities.extend(Density::ALL.map(|d| (Some(d), d.name())));

        let button_controls = v_flex()
            .gap(Space::Sm)
            // Eight variants are two rows of four, so they fit a narrow window.
            .child(choice(
                "pg-variant",
                "Variant",
                ButtonVariant::ALL[..4]
                    .iter()
                    .map(|v| (*v, v.name()))
                    .collect(),
                b.variant,
                |g: &mut PlaygroundPage, v, _| g.button.variant = v,
                cx,
            ))
            .child(choice_labelled(
                "pg-variant-more",
                "More variants",
                false,
                ButtonVariant::ALL[4..]
                    .iter()
                    .map(|v| (*v, v.name()))
                    .collect(),
                b.variant,
                |g: &mut PlaygroundPage, v, _| g.button.variant = v,
                cx,
            ))
            .child(choice(
                "pg-emphasis",
                "Emphasis",
                emphases,
                b.emphasis,
                |g, e, _| g.button.emphasis = e,
                cx,
            ))
            .child(choice(
                "pg-density",
                "Density",
                densities,
                b.density,
                |g, d, _| g.button.density = d,
                cx,
            ))
            .child(choice(
                "pg-content",
                "Icon",
                vec![
                    (Content::Text, "none"),
                    (Content::Leading, "leading"),
                    (Content::Trailing, "trailing"),
                    (Content::IconOnly, "icon only"),
                ],
                b.content,
                |g, c, _| g.button.content = c,
                cx,
            ))
            .child(choice(
                "pg-corners",
                "Corners (whole gallery)",
                vec![
                    (CornerRadius::Square, "Square"),
                    (CornerRadius::Subtle, "Subtle"),
                ],
                config.corner_radius,
                |_, r, cx| odk::ui::configure(cx, |c| c.corner_radius = r),
                cx,
            ))
            .child(
                row()
                    .child(toggle(
                        "pg-disabled",
                        "Disabled",
                        b.disabled,
                        |g, v| g.button.disabled = v,
                        cx,
                    ))
                    .child(toggle(
                        "pg-loading",
                        "Loading",
                        b.loading,
                        |g, v| g.button.loading = v,
                        cx,
                    )),
            );

        let f = self.field;
        let applies = |p: &str| f.applies(p);
        let mut checks = row();
        for (id, label, property, value) in [
            ("pg-f-invalid", "Invalid", "invalid", f.invalid),
            ("pg-f-disabled", "Disabled", "disabled", f.disabled),
            ("pg-f-readonly", "Read-only", "read_only", f.read_only),
            ("pg-f-required", "Required", "required", f.required),
            ("pg-f-checked", "Checked", "checked", f.checked),
        ] {
            if applies(property) {
                checks = checks.child(toggle(
                    id,
                    label,
                    value,
                    move |g, v| match property {
                        "invalid" => g.field.invalid = v,
                        "disabled" => g.field.disabled = v,
                        "read_only" => g.field.read_only = v,
                        "required" => g.field.required = v,
                        _ => g.field.checked = v,
                    },
                    cx,
                ));
            }
        }
        let field_controls = v_flex()
            .gap(Space::Sm)
            .child(choice(
                "pg-control",
                "Control",
                vec![
                    (Control::Input, "Input"),
                    (Control::Checkbox, "Checkbox"),
                    (Control::Switch, "Switch"),
                    (Control::Select, "Select"),
                ],
                f.control,
                |g, c, _| g.field.control = c,
                cx,
            ))
            .child(checks);

        let bench = match f.control {
            Control::Input => {
                let input = Input::new("pg-input", "Name")
                    .default_value("Ada Lovelace")
                    .read_only(f.read_only)
                    .invalid(f.invalid)
                    .disabled(f.disabled);
                FormField::new("pg-input-field", "Name")
                    .required(f.required)
                    .child(input)
                    .into_any_element()
            }
            Control::Checkbox => Checkbox::new("pg-checkbox", "I accept the terms")
                .checked(f.checked)
                .invalid(f.invalid)
                .disabled(f.disabled)
                .on_change(flag(cx, |this: &mut PlaygroundPage, on, cx| {
                    this.field.checked = on;
                    cx.notify();
                }))
                .into_any_element(),
            Control::Switch => Switch::new("pg-switch", "Wi-Fi")
                .checked(f.checked)
                .disabled(f.disabled)
                .on_change(flag(cx, |this: &mut PlaygroundPage, on, cx| {
                    this.field.checked = on;
                    cx.notify();
                }))
                .into_any_element(),
            Control::Select => FormField::new("pg-select-field", "Region")
                .child(
                    Select::new("pg-select", "Region")
                        .item("a", "Region A")
                        .item("b", "Region B")
                        .default_value("a")
                        .invalid(f.invalid)
                        .required(f.required)
                        .disabled(f.disabled),
                )
                .into_any_element(),
        };

        let tokens_view = self.tokens_view(cx);
        let button_code = SharedString::from(b.code());
        let field_code = SharedString::from(f.code());

        page(
            Page::Playground,
            v_flex()
                .gap(Space::Lg)
                .child(Text::new("Everything here is a real component built from the properties you set. Changes apply at once and keep what you have typed. Density here is this button's own; to change the whole gallery's, use the toolbar.").muted())
                .child(section("Button"))
                .child(button_controls)
                .child(h_flex().py(Space::Md).child(b.build("pg-button")))
                .child(code_panel_owned("playground-button", button_code))
                .child(section("Form control"))
                .child(field_controls)
                .child(div().py(Space::Md).max_w(odk::gpui::px(crate::INPUT_WIDTH)).child(bench))
                .child(group("Code"))
                .child(code_panel_owned("playground-field", field_code))
                .child(section("Tokens"))
                .child(Text::new("The resolved tokens of the button and the control above, where each colour came from, and its contrast. Read from the component tokens' diagnostics, once per theme.").muted())
                .child(tokens_view)

        )
    }
}
