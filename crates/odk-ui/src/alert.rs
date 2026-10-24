//! An inline notice, on component tokens: a Role at an Emphasis with its status mark
//! (`docs/spec/odk-component-tokens.md`).

use gpui::{
    AnyElement, App, ClickEvent, ElementId, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, SharedString, Styled, Window, div, px,
};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::{AlertStyle, ButtonLayout, ButtonVariant, States};
use odk_theme::semantic::Emphasis;

use crate::focus::{child_id, part_name};
use crate::layout::{StyledExt, h_flex, v_flex};
use crate::theme::{colour, mono};
use crate::{Button, Size, Tone, components};

type Handler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// An inline notice that does not interrupt: the tone's status mark, a title, an optional
/// description, an optional action and an optional dismiss button, on a quiet tint of the tone
/// (muted) with a 1 px edge, or with no fill and an edge in the tone's colour (subtle). The mark
/// and the words carry the meaning, so it reads in a monochrome theme. It never takes focus; its
/// action and dismiss button are ordinary buttons, reached with Tab and pressed with Enter or
/// Space.
#[derive(IntoElement)]
pub struct Alert {
    id: ElementId,
    tone: Tone,
    emphasis: Emphasis,
    compact: bool,
    title: SharedString,
    description: Option<SharedString>,
    action: Option<AnyElement>,
    on_dismiss: Option<Handler>,
}

impl Alert {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            tone: Tone::default(),
            emphasis: Emphasis::Muted,
            compact: false,
            title: title.into(),
            description: None,
            action: None,
            on_dismiss: None,
        }
    }

    /// Info (the default), success, warning or danger.
    pub fn variant(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// Muted (the default) is a tint of the tone with its text in the tone's colour; subtle has
    /// no fill, ordinary text and an edge in the tone's colour; solid is a filled panel, for the
    /// rare notice that must not be missed.
    pub fn emphasis(mut self, emphasis: Emphasis) -> Self {
        self.emphasis = emphasis;
        self
    }

    /// Tighter padding and gap, for a notice inline in a dense place, such as under a field.
    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// A control on the notice, usually a small `Button` (`size(Size::Sm)`), whose centre lines
    /// up with the title's line.
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_any_element());
        self
    }

    /// Shows a dismiss button, named "Dismiss", that calls `handler`; the application removes
    /// the alert. Focus is not moved when it goes.
    pub fn on_dismiss(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_dismiss = Some(Box::new(handler));
        self
    }
}

impl Describe for Alert {
    fn accessible(&self) -> Accessible {
        let name = match &self.description {
            Some(d) => format!("{}: {d}", self.title),
            None => self.title.to_string(),
        };
        Accessible::new(Role::Alert, name)
    }
}

impl RenderOnce for Alert {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let c = components();
        let s: AlertStyle = c.alert(self.tone.into(), self.emphasis);
        #[cfg(test)]
        crate::drawn::record(&crate::focus::id_path(&self.id), (s, self.compact));
        let g = if self.compact { s.compact } else { s.geometry };
        let f = c.foundation();
        let title_line = f.typography.style(g.title).line_height;
        // The dismiss button, and an action that is a small button, sit with their centre on the
        // title's line: as far above the line as they are taller than it.
        let dismiss_size = Size::Sm.density(c.density(None));
        let small = c.button(
            ButtonVariant::Ghost,
            dismiss_size,
            ButtonLayout::IconOnly,
            States::NONE,
        );
        let lift = px(-((small.geometry.height - title_line) / 2.0).max(0.0));

        // `min_w_0` lets the text be narrower than its longest word, so long words and
        // URLs wrap inside the box and the trailing buttons keep their place.
        let mut text = v_flex()
            .debug_selector(|| part_name(&self.id, "text"))
            .flex_1()
            .min_w_0()
            .gap(px(f.spacing.s1))
            .child(div().text_role(g.title).child(self.title));
        if let Some(d) = self.description {
            text = text.child(
                div()
                    .text_role(g.body)
                    .text_color(colour(s.description))
                    .child(d),
            );
        }
        let icon = px(g.icon_size);
        let mut alert = h_flex()
            .debug_selector(|| part_name(&self.id, "alert"))
            .items_start()
            .gap(px(g.gap))
            .px(px(g.padding_x))
            .py(px(g.padding_y))
            .rounded(px(g.radius))
            .border(px(g.border_width))
            .treatment(&s.surface)
            .text_role(g.body)
            .child(
                div()
                    .debug_selector(|| part_name(&self.id, "mark"))
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_shrink_0()
                    .w(icon)
                    .h(px(title_line))
                    .font_family(mono())
                    .text_size(icon)
                    .line_height(icon)
                    .text_color(colour(s.mark))
                    .child(self.tone.mark()),
            )
            .child(text)
            .children(
                self.action
                    .map(|a| div().flex_shrink_0().mt(lift).mb(lift).child(a)),
            );
        if let Some(handler) = self.on_dismiss {
            alert = alert.child(
                div().flex_shrink_0().mt(lift).mb(lift).child(
                    Button::icon(child_id(&self.id, "dismiss"), "×", "Dismiss")
                        .variant(ButtonVariant::Ghost)
                        .density(dismiss_size)
                        .on_click(handler),
                ),
            );
        }
        alert
    }
}
