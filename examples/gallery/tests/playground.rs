//! The playground builds the real component from its properties and writes the Rust that builds
//! the same one. The code is read back here into properties, so a line that named something
//! the component does not take, or left out something it does, would not round-trip.

mod common;

use common::{click, open, serial};
use gallery::Page;
use gallery::playground::{ButtonProps, Content, Control, FieldProps};
use odk::gpui::TestAppContext;
use odk::prelude::{ButtonVariant, Density, Emphasis};

/// The properties a button's code describes, read from its lines.
fn read_button(code: &str) -> ButtonProps {
    let mut props = ButtonProps {
        content: Content::Text,
        ..Default::default()
    };
    for line in code.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix(".variant(ButtonVariant::") {
            let name = rest.trim_end_matches(')');
            props.variant = ButtonVariant::ALL
                .into_iter()
                .find(|v| format!("{v:?}") == name)
                .unwrap_or_else(|| panic!("no variant {name}"));
        } else if let Some(rest) = line.strip_prefix(".emphasis(Emphasis::") {
            let name = rest.trim_end_matches(')');
            props.emphasis = Some(
                Emphasis::ALL
                    .into_iter()
                    .find(|e| format!("{e:?}") == name)
                    .unwrap_or_else(|| panic!("no emphasis {name}")),
            );
        } else if let Some(rest) = line.strip_prefix(".density(Density::") {
            let name = rest.trim_end_matches(')');
            props.density = Some(
                Density::ALL
                    .into_iter()
                    .find(|d| format!("{d:?}") == name)
                    .unwrap_or_else(|| panic!("no density {name}")),
            );
        } else if line == ".loading(true)" {
            props.loading = true;
        } else if line == ".disabled(true)" {
            props.disabled = true;
        } else if line.starts_with(".leading_icon(") {
            props.content = Content::Leading;
        } else if line.starts_with(".trailing_icon(") {
            props.content = Content::Trailing;
        } else if line.starts_with("Button::icon(") {
            props.content = Content::IconOnly;
        }
    }
    props
}

#[test]
fn a_buttons_code_reads_back_as_its_properties() {
    let mut n = 0;
    for variant in ButtonVariant::ALL {
        for emphasis in [
            None,
            Some(Emphasis::Solid),
            Some(Emphasis::Muted),
            Some(Emphasis::Subtle),
        ] {
            for density in [None, Some(Density::Compact), Some(Density::Comfortable)] {
                for content in [
                    Content::Text,
                    Content::Leading,
                    Content::Trailing,
                    Content::IconOnly,
                ] {
                    for (disabled, loading) in [(false, false), (true, false), (false, true)] {
                        let props = ButtonProps {
                            variant,
                            emphasis,
                            density,
                            content,
                            disabled,
                            loading,
                        };
                        let code = props.code();
                        assert_eq!(read_button(&code), props, "{code}");
                        assert!(code.starts_with("Button::"), "{code}");
                        n += 1;
                    }
                }
            }
        }
    }
    assert_eq!(n, 8 * 4 * 3 * 4 * 3);
}

/// A form control's code names only what applies to it, and the properties that do not apply
/// to the control on the bench are not offered.
#[test]
fn a_form_controls_code_names_only_what_applies() {
    let on = |control| FieldProps {
        control,
        invalid: true,
        disabled: true,
        read_only: true,
        required: true,
        checked: true,
    };
    let input = on(Control::Input).code();
    assert!(input.contains("Input::new") && input.contains(".read_only(true)"));
    assert!(input.contains(".invalid(true)") && input.contains(".disabled(true)"));
    assert!(input.starts_with("FormField::new") && input.contains(".required(true)"));
    assert!(!input.contains(".checked("));

    let check = on(Control::Checkbox).code();
    assert!(
        check.contains("Checkbox::new")
            && check.contains(".checked(true)")
            && check.contains(".invalid(true)")
    );
    assert!(!check.contains("read_only") && !check.contains("required"));

    let switch = on(Control::Switch).code();
    assert!(
        switch.contains("Switch::new")
            && switch.contains(".checked(true)")
            && switch.contains(".disabled(true)")
    );
    assert!(!switch.contains("invalid"));

    let select = on(Control::Select).code();
    assert!(
        select.contains("Select::new")
            && select.contains(".required(true)")
            && select.contains(".invalid(true)")
    );
    assert!(!select.contains("read_only") && !select.contains(".checked("));

    for (control, property, applies) in [
        (Control::Input, "read_only", true),
        (Control::Checkbox, "read_only", false),
        (Control::Switch, "invalid", false),
        (Control::Select, "checked", false),
        (Control::Select, "disabled", true),
    ] {
        assert_eq!(
            on(control).applies(property),
            applies,
            "{control:?} {property}"
        );
    }
}

/// Setting a property in the playground changes the real component at once: an icon-only button
/// is narrower than one with a label, and the properties are the ones set.
#[test]
fn the_controls_change_the_real_component() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let (gallery, cx) = open(&mut app, Page::Playground);
    let props = |cx: &mut odk::gpui::VisualTestContext| {
        gallery.read_with(cx, |g, cx| {
            let p = g.playground().read(cx);
            (p.button, p.field)
        })
    };
    let width = |cx: &mut odk::gpui::VisualTestContext| {
        cx.debug_bounds("pg-button/button").unwrap().size.width
    };
    let wide = width(cx);

    click(cx, "pg-content/item-3");
    assert_eq!(props(cx).0.content, Content::IconOnly);
    assert!(
        width(cx) < wide,
        "an icon-only button is narrower: {:?} against {wide:?}",
        width(cx)
    );
    click(cx, "pg-variant-more/item-0");
    assert_eq!(props(cx).0.variant, ButtonVariant::Destructive);
    click(cx, "pg-variant/item-1");
    assert_eq!(props(cx).0.variant, ButtonVariant::Secondary);
    click(cx, "pg-variant-more/item-0");
    click(cx, "pg-emphasis/item-2");
    assert_eq!(props(cx).0.emphasis, Some(Emphasis::Muted));
    click(cx, "pg-density/item-1");
    assert_eq!(props(cx).0.density, Some(Density::Compact));
    let compact = cx.debug_bounds("pg-button/button").unwrap().size.height;
    click(cx, "pg-density/item-3");
    assert!(cx.debug_bounds("pg-button/button").unwrap().size.height > compact);
    click(cx, "pg-disabled/box");
    click(cx, "pg-loading/box");
    let b = props(cx).0;
    assert!(b.disabled && b.loading);

    // The control on the bench.
    click(cx, "pg-control/item-1");
    assert_eq!(props(cx).1.control, Control::Checkbox);
    click(cx, "pg-f-invalid/box");
    assert!(props(cx).1.invalid);
}
