//! An inline notice on the Rune host: a Role at an Emphasis with its status mark
//! (`docs/spec/odk-component-tokens.md`).

use std::rc::Rc;

use crate::button::Button;
use crate::describe::apply;
use crate::host::Hosted;
use crate::icon::Icon;
use crate::ink::ink;
use crate::style;
use crate::text::{Flow, words};
use crate::tone::Tone;
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::{ButtonLayout, ButtonVariant, States};
use odk_theme::semantic::Emphasis;
use rune::{El, col, el, row};

/// What the application does when the alert is dismissed.
type Dismiss<T> = Rc<dyn Fn(&mut T)>;

/// An inline notice that does not interrupt: the tone's status mark, a title, an optional
/// description, an optional action and an optional dismiss button, on a quiet tint of the tone
/// (muted) with an edge, or with no fill and an edge in the tone's colour (subtle). The mark and
/// the words carry the meaning, so it reads in a monochrome theme. It never takes focus; its
/// action and dismiss button are ordinary buttons, reached with Tab and pressed with Enter or
/// Space.
pub struct Alert<T> {
    id: String,
    tone: Tone,
    emphasis: Emphasis,
    compact: bool,
    title: String,
    description: Option<String>,
    action: Option<El>,
    on_dismiss: Option<Dismiss<T>>,
}

impl<T: 'static> Alert<T> {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
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
    pub fn tone(mut self, tone: Tone) -> Self {
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

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// A control on the notice, usually a small `Button`, whose centre lines up with the title's
    /// line.
    pub fn action(mut self, action: El) -> Self {
        self.action = Some(action);
        self
    }

    /// Shows a dismiss button, named "Dismiss", that calls `handler`; the application removes the
    /// alert.
    pub fn on_dismiss(mut self, handler: impl Fn(&mut T) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(handler));
        self
    }

    pub fn show(self, hosted: &Hosted<T>) -> El {
        let accessible = self.accessible();
        let paint = style::paint(hosted);
        let s = paint.tokens.alert(self.tone.into(), self.emphasis);
        let g = if self.compact { s.compact } else { s.geometry };
        let gap = paint.tokens.foundation().spacing.s1;
        let title_line = paint.role(g.title).line_height;
        // The dismiss button, and an action that is a small button, sit with their centre on the
        // title's line: as far above the line as they are taller than it.
        let dismiss_density = paint.tokens.density(None).smaller();
        let small = paint.tokens.button(
            ButtonVariant::Ghost,
            dismiss_density,
            ButtonLayout::IconOnly,
            States::NONE,
        );
        let lift = ((small.geometry.height - title_line) / 2.0).max(0.0);
        let lifted = |element: El| row().mt(-lift).mb(-lift).child(element);

        let mut text = col().grow(1.0).shrink().gap(gap).child(words(
            &paint,
            self.title,
            g.title,
            s.surface.foreground,
            Flow::Wrap,
        ));
        if let Some(description) = self.description {
            text = text.child(words(
                &paint,
                description,
                g.body,
                s.description,
                Flow::Wrap,
            ));
        }
        let mark = el().w(g.icon_size).h(title_line).center().child(
            Icon::status(self.tone.into())
                .map_or_else(el, |icon| icon.mark(g.icon_size, ink(s.mark))),
        );
        let mut alert = apply(row(), &accessible)
            .items_start()
            .gap(g.gap)
            .px(g.padding_x)
            .py(g.padding_y)
            .radius(g.radius)
            .fill(ink(s.surface.background))
            .outline(g.border_width, 0.0, ink(s.surface.border))
            .child(mark)
            .child(text);
        if let Some(action) = self.action {
            alert = alert.child(lifted(action));
        }
        if let Some(handler) = self.on_dismiss {
            alert = alert.child(lifted(
                Button::icon(
                    format!("{}-dismiss", self.id),
                    Icon::Close,
                    "Dismiss",
                    move |app: &mut T| handler(app),
                )
                .variant(ButtonVariant::Ghost)
                .density(dismiss_density)
                .show(hosted),
            ));
        }
        alert
    }
}

impl<T> Describe for Alert<T> {
    fn accessible(&self) -> Accessible {
        let name = match &self.description {
            Some(description) => format!("{}: {description}", self.title),
            None => self.title.clone(),
        };
        Accessible::new(Role::Alert, name)
    }
}
