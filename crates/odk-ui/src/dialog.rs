use std::rc::Rc;

use gpui::{
    AnyElement, App, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use odk_a11y::{Accessible, Describe, Role};

use odk_theme::foundation::MotionDuration;

use crate::focus::{child_id, part_name};
use crate::layout::{StyledExt, h_flex, inside_window, v_flex};
use crate::overlay::{Action, Dismiss, Transition, layer_over, modal, modal_header};
use crate::{Text, components};

type Build = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement + 'static>;

/// The widest a dialog is, in logical pixels. ODK's own (ADR 0035).
const DIALOG_WIDTH: f32 = 480.0;

/// A modal panel centred over a scrim: a title and close button, an optional description, a
/// body and its actions, laid out by the dialog in a row aligned to the end that wraps when
/// they do not fit, so no action is ever pushed out of reach.
///
/// While open, Tab stays inside it and the window behind it takes no pointer or key; Escape, the
/// close button or a click on the scrim asks the application to close it through `on_close`,
/// and focus goes back to where it was. [`Dialog::close_on_escape`], [`Dialog::close_on_backdrop`]
/// and [`Dialog::close_button`] take those away one by one, as a confirmation of something
/// destructive or a dialog that is busy should; with all three off the dialog closes only when
/// the application stops saying `open`. It fades in with a slight rise over the foundation's
/// `enter` and out over its `exit`, at once with reduced motion. It is never larger than the
/// window: a body too tall scrolls while the header and actions stay. Controlled: the
/// application owns `open`. While it closes it keeps drawing the title, description, body and
/// actions it last had open.
///
/// Its surface, border, corners, padding and gaps are the dialog's component token
/// (`ComponentTokens::dialog`); its title is a `Heading`, its description a `Text`, and its
/// actions are the application's own `Button`s, so a destructive action is a button with
/// `ButtonVariant::Destructive`. A form dialog holds the form controls as its content; a busy
/// one disables its buttons (`Button::loading`) and the three ways out; one that failed
/// validation shows an `Alert` above its fields and stays open.
#[derive(IntoElement)]
pub struct Dialog {
    id: ElementId,
    open: bool,
    title: SharedString,
    description: Option<SharedString>,
    content: Option<Build>,
    actions: Vec<Build>,
    on_close: Option<Action>,
    close_on_escape: bool,
    close_on_backdrop: bool,
    close_button: bool,
}

impl Dialog {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            open: false,
            title: title.into(),
            description: None,
            content: None,
            actions: Vec::new(),
            on_close: None,
            close_on_escape: true,
            close_on_backdrop: true,
            close_button: true,
        }
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// A line or two under the title saying what the dialog is for.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Builds the body, each frame while the dialog is shown.
    pub fn content<E: IntoElement>(
        mut self,
        build: impl Fn(&mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.content = Some(Rc::new(move |w, cx| build(w, cx).into_any_element()));
        self
    }

    /// Adds an action at the bottom, usually a button; actions keep the order they are added
    /// in, the last usually the main one.
    pub fn action<E: IntoElement>(
        mut self,
        build: impl Fn(&mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.actions
            .push(Rc::new(move |w, cx| build(w, cx).into_any_element()));
        self
    }

    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }

    /// Whether Escape asks to close; true by default.
    pub fn close_on_escape(mut self, close: bool) -> Self {
        self.close_on_escape = close;
        self
    }

    /// Whether a click on the scrim asks to close; true by default. Turn it off for a
    /// confirmation of something that cannot be undone, which only its buttons should answer.
    pub fn close_on_backdrop(mut self, close: bool) -> Self {
        self.close_on_backdrop = close;
        self
    }

    /// Whether the header has a close button; true by default.
    pub fn close_button(mut self, show: bool) -> Self {
        self.close_button = show;
        self
    }
}

impl Describe for Dialog {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Dialog, self.title.to_string())
    }
}

/// Everything a dialog needs to draw its exit.
#[derive(Clone)]
struct Shown {
    title: SharedString,
    description: Option<SharedString>,
    content: Option<Build>,
    actions: Vec<Build>,
}

impl RenderOnce for Dialog {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = components();
        let f = c.foundation().clone();
        let frame = layer_over(
            child_id(&self.id, "layer"),
            self.open,
            true,
            Transition::of(MotionDuration::Enter, MotionDuration::Exit),
            window,
            cx,
        );
        let Some(amount) = frame.amount else {
            return div().into_any_element();
        };
        let amount = f.motion.easing.apply(amount);
        // Keep what the dialog opened with until its exit has run.
        let wanted = Shown {
            title: self.title,
            description: self.description,
            content: self.content,
            actions: self.actions,
        };
        let first = wanted.clone();
        let kept = window.use_keyed_state(child_id(&self.id, "shown"), cx, |_, _| first);
        let open = self.open;
        let Shown {
            title,
            description,
            content,
            actions,
        } = kept.update(cx, |k, _| {
            if open {
                *k = wanted;
            }
            k.clone()
        });
        let close: Action = self.on_close.unwrap_or_else(|| Rc::new(|_, _| {}));
        let mut dismiss = Dismiss::new(close.clone());
        dismiss.escape = self.close_on_escape;
        dismiss.backdrop = self.close_on_backdrop;

        let style = c.dialog();
        let g = style.geometry;
        let viewport = window.viewport_size();
        let margin = px(f.spacing.s4);
        let (pad_x, pad_y) = (px(g.padding_x), px(g.padding_y));
        let panel = v_flex()
            .id(child_id(&self.id, "panel"))
            .debug_selector(|| part_name(&self.id, "panel"))
            .relative()
            // Never larger than the window, less a margin on every side.
            .w(px(DIALOG_WIDTH).min(inside_window(viewport.width, margin)))
            .max_h(inside_window(viewport.height, margin))
            .treatment(&style.surface)
            .border(px(g.border_width))
            .rounded(px(g.radius))
            .opacity(amount)
            // A slight rise as it fades in.
            .top(px((1.0 - amount) * f.spacing.s2))
            .child(
                v_flex()
                    .gap(px(f.spacing.s2))
                    .px(pad_x)
                    .pt(pad_y)
                    .pb(px(f.spacing.s2))
                    .child(modal_header(
                        &self.id,
                        title,
                        self.close_button,
                        close.clone(),
                    ))
                    .children(description.map(|d| Text::new(d).muted())),
            )
            .child(
                // The body scrolls when the dialog would be taller than the window.
                div()
                    .id(child_id(&self.id, "body"))
                    .debug_selector(|| part_name(&self.id, "body"))
                    .flex_1()
                    .overflow_y_scroll()
                    .px(pad_x)
                    .children(content.map(|build| build(window, cx))),
            )
            .child(
                h_flex()
                    .debug_selector(|| part_name(&self.id, "footer"))
                    .flex_wrap()
                    .justify_end()
                    .gap(px(f.spacing.s2))
                    .px(pad_x)
                    .pb(pad_y)
                    .pt(px(g.gap))
                    .children(actions.iter().map(|build| build(window, cx))),
            );
        modal(
            &self.id,
            amount,
            &dismiss,
            &frame.content,
            panel,
            window,
            cx,
        )
    }
}
