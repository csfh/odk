use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, ElementId, InteractiveElement, IntoElement, KeyDownEvent,
    KeyUpEvent, ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled,
    Window, div, px,
};

use odk_theme::component::{
    ButtonAppearance, ButtonLayout, ButtonStyle, ButtonVariant, ComponentTokens, States,
};
use odk_theme::foundation::{Density, MotionDuration};
use odk_theme::semantic::Emphasis;

use odk_a11y::{Accessible, Describe, Role};

use crate::components;
use crate::decoration::{self, Applied, FlowKind, TextDecoration};
use crate::focus::{child_id, focus_handle, part_name};
use crate::icon::{self, Glyph};
use crate::interaction::{Pointer, focus_ring, pointer_states, progress};
use crate::layout::StyledExt;
use crate::spinner::Spinner;
use crate::theme::mono;

/// A control's size relative to the density it would otherwise take: `Md` is that density,
/// `Sm` one step tighter and `Lg` one step roomier, stopping at compact and comfortable, so a
/// control never takes a height the density table does not have (`docs/spec/odk-component-tokens.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Size {
    Sm,
    #[default]
    Md,
    Lg,
}

impl Size {
    /// The density a control of this size takes where it would otherwise take `base`.
    pub const fn density(self, base: Density) -> Density {
        match self {
            Self::Sm => base.smaller(),
            Self::Md => base,
            Self::Lg => base.larger(),
        }
    }
}

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
type ChordHandler = Rc<dyn Fn(Option<&Chord>, &mut Window, &mut App) + 'static>;

/// A key and the modifiers held with it, as a listening button hears it (`Button::listening`,
/// ADR 0076). `key` is the key's name as the platform gives it: a letter or digit (`a`, `5`),
/// a symbol (`,`, `.`), or a name (`enter`, `space`, `backspace`, `delete`, `left`, `up`, `f1`
/// to `f24`). `platform` is the Super key. An application turns it into its own words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chord {
    pub key: SharedString,
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
    pub platform: bool,
    pub function: bool,
}

/// The GPUI group a larger pointer target gives its button's face.
const TARGET_GROUP: &str = "odk-button-target";
/// The GPUI group a face that takes the pointer itself gives an icon it draws.
const FACE_GROUP: &str = "odk-button-face";

/// A button: a label, an icon before or after it, or an icon alone (`Button::icon`), in one of
/// eight variants (`ButtonVariant`) at the application's density or its own.
///
/// Tab reaches it and Enter or Space presses it, on the key's release, once per press. A click
/// presses it when the button goes down and comes up on it; released elsewhere, nothing
/// happens. While `loading` it stays in the Tab order and keeps its focus but ignores presses;
/// while `disabled` it ignores the pointer and the keyboard and is skipped by Tab.
///
/// While `listening` (ADR 0076) and focused, it takes the next key press, with the modifiers
/// held, as a `Chord` for `on_chord` instead of pressing: a key held down reports once, Escape
/// reports `None` (a cancel), Tab still moves focus, and focus leaving reports `None` once. A key
/// it took does not press it when released. The application says that it listens, in the label
/// ("Listening…"), and stops listening when it has the chord it wants.
///
/// It draws only component tokens: the colours of its variant and emphasis by state, and the
/// height, padding, icon size and corner radius of its density. Hover and press change colour at
/// once; a focused button has a ring outside it that moves nothing.
#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    leading: Option<Glyph>,
    trailing: Option<Glyph>,
    variant: ButtonVariant,
    emphasis: Option<Emphasis>,
    size: Size,
    density: Option<Density>,
    full_width: bool,
    target: Option<f32>,
    on_click: Option<ClickHandler>,
    listening: bool,
    on_chord: Option<ChordHandler>,
    loading: bool,
    disabled: bool,
    name: Option<SharedString>,
    /// Drawn inside a field's box (`ComponentTokens::field_action`): lower than the field, so
    /// it never makes the field taller. Set by the fields that hold a button.
    pub(crate) in_field: bool,
    /// Left out of the Tab order, for a field's button whose job the field's own keys do.
    pub(crate) skip_tab: bool,
    /// Takes no focus from the pointer, so a press leaves it where it was: a field's own
    /// buttons, which act on the field without taking its caret or selection.
    pub(crate) keeps_focus: bool,
    applied: Applied,
}

impl Button {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: SharedString::default(),
            leading: None,
            trailing: None,
            variant: ButtonVariant::default(),
            emphasis: None,
            size: Size::default(),
            density: None,
            full_width: false,
            target: None,
            on_click: None,
            listening: false,
            on_chord: None,
            loading: false,
            disabled: false,
            name: None,
            in_field: false,
            skip_tab: false,
            keeps_focus: false,
            applied: Applied::default(),
        }
    }

    /// Attaches a text decoration. A later one of the same concern replaces the earlier. A button
    /// ends in `…` when its width is bounded; [`crate::Wrap`] lets the label wrap instead.
    pub fn decorate(mut self, decoration: impl TextDecoration) -> Self {
        decoration.apply_text(&mut self.applied);
        self
    }

    /// A button that is an icon alone: a square as wide as it is high. It has no text to be
    /// named by, so it takes its accessible name here. The icon is one of ODK's ([`crate::Icon`],
    /// ADR 0080), such as `Icon::Close`, or a character, such as `"⚙"`.
    pub fn icon(
        id: impl Into<ElementId>,
        icon: impl Into<Glyph>,
        name: impl Into<SharedString>,
    ) -> Self {
        Self::new(id).leading_icon(icon).accessible_name(name)
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    /// An icon before the label, at the density's icon size and gap. While loading, the
    /// spinner takes its place and the label stays.
    pub fn leading_icon(mut self, icon: impl Into<Glyph>) -> Self {
        self.leading = Some(icon.into());
        self
    }

    /// An icon after the label, such as [`crate::Icon::ArrowRight`] for an action that goes on.
    pub fn trailing_icon(mut self, icon: impl Into<Glyph>) -> Self {
        self.trailing = Some(icon.into());
        self
    }

    /// One of the eight variants of `ButtonVariant`. Each is a role at an
    /// emphasis; `emphasis` changes the emphasis and keeps the role.
    pub fn variant(mut self, variant: impl Into<ButtonVariant>) -> Self {
        self.variant = variant.into();
        self
    }

    /// Draws the variant's role at another emphasis: a muted `Destructive` for a less prominent
    /// destructive action, say. Every role takes every emphasis. A `Link` is drawn the same at
    /// every emphasis, since it has no fill.
    pub fn emphasis(mut self, emphasis: Emphasis) -> Self {
        self.emphasis = Some(emphasis);
        self
    }

    /// A step from the density the button would otherwise take; `density` wins over it.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The button's own density, whatever the application's and whatever its `size`.
    pub fn density(mut self, density: Density) -> Self {
        self.density = Some(density);
        self
    }

    /// Fills the width it is given rather than taking its content's. A label that does not fit
    /// ends in `…`. Without this a button is as wide as its content and never gives up width.
    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }

    /// The least width and height, in logical pixels, of the area that takes the pointer, for
    /// an application that wants larger targets than the button (WCAG 2.5.5's 44, say). The
    /// button is drawn at its own size in the middle of that area, which takes its space in the
    /// layout. A button is at least the foundation's `target_min` (24 px). A field action uses
    /// that floor when the room inside the field is smaller.
    pub fn target_size(mut self, size: f32) -> Self {
        self.target = Some(size);
        self
    }

    /// Shows a spinner and ignores presses while `loading` is true. The spinner takes the
    /// place of the icon, or is laid over the label when there is none, and the label keeps its
    /// space, so the button does not change size; it fades in over `motion.normal`, at once with
    /// reduced motion, and stands still then.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// A disabled button ignores the pointer and the keyboard, is not a tab stop, and draws its
    /// variant's disabled treatment.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The accessible name when the label alone does not say what the button does, as for
    /// an icon or `×`.
    pub fn accessible_name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Draws the button inside a field's box, lower than the field.
    pub(crate) fn in_field(mut self) -> Self {
        self.in_field = true;
        self
    }

    /// Leaves the button out of the Tab order, for a field's button whose job the field's own
    /// keys do: a search field's clear button (Escape), a number field's steppers (Up, Down).
    pub(crate) fn skip_tab(mut self) -> Self {
        self.skip_tab = true;
        self
    }

    /// A press on the button leaves focus where it was, for a field's own buttons.
    pub(crate) fn keep_focus(mut self) -> Self {
        self.keeps_focus = true;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }

    /// Takes the next key press as a chord rather than a press, while focused (ADR 0076).
    pub fn listening(mut self, listening: bool) -> Self {
        self.listening = listening;
        self
    }

    /// Hears the chord a listening button took, or `None` when Escape or focus leaving cancels.
    pub fn on_chord(
        mut self,
        handler: impl Fn(Option<&Chord>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_chord = Some(Rc::new(handler));
        self
    }

    fn appearance(&self) -> ButtonAppearance {
        let appearance = self.variant.appearance();
        match self.emphasis {
            Some(emphasis) => appearance.with_emphasis(emphasis),
            None => appearance,
        }
    }

    /// An icon with no label is an icon-only button; otherwise the label leads.
    fn layout(&self) -> ButtonLayout {
        match (&self.leading, &self.trailing, self.label.is_empty()) {
            (Some(_), None, true) | (None, Some(_), true) => ButtonLayout::IconOnly,
            (Some(_), _, _) => ButtonLayout::LeadingIcon,
            (None, Some(_), _) => ButtonLayout::TrailingIcon,
            (None, None, _) => ButtonLayout::Text,
        }
    }

    /// What the button draws with: its own density, else its size's step from the
    /// application's, its appearance, layout and states. The only place `render` gets a colour
    /// or a length from.
    fn style(&self, c: &ComponentTokens, focused: bool) -> ButtonStyle {
        let density = self
            .density
            .unwrap_or_else(|| self.size.density(c.density(None)));
        let states = States::NONE
            .focused(focused)
            .disabled(self.disabled)
            .loading(self.loading);
        if self.in_field {
            c.field_action(self.appearance(), density, self.layout(), states)
        } else {
            c.button(self.appearance(), density, self.layout(), states)
        }
    }
}

impl Describe for Button {
    fn accessible(&self) -> Accessible {
        let name = self.name.as_ref().unwrap_or(&self.label);
        Accessible::new(Role::Button, name.to_string())
            .busy(self.loading)
            .disabled(self.disabled)
    }
}

/// One icon of `style`'s size: a character in the button's text colour, or one of ODK's icons
/// in the colours a character there would take, following `group`'s pointer states (ADR 0080).
fn glyph(style: &ButtonStyle, icon: Glyph, id: ElementId, group: SharedString) -> AnyElement {
    let size = style.geometry.icon_size;
    let slot = div().flex().items_center().justify_center().size_full();
    match icon {
        Glyph::Text(text) => slot
            .font_family(mono())
            .text_size(px(size))
            .line_height(px(size))
            .child(text)
            .into_any_element(),
        Glyph::Icon(icon) => {
            let c = &style.colours;
            let inks = [c.default, c.hover, c.pressed].map(|t| t.foreground);
            let pointer = style.responds.then_some((group, id));
            slot.child(icon::drawn(icon, size, inks, pointer))
                .into_any_element()
        }
    }
}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // A disabled button tracks no focus handle, so it can be neither tabbed to nor clicked
        // into focus. A loading one keeps its place in the Tab order.
        let focus = (!self.disabled)
            .then(|| focus_handle(child_id(&self.id, "focus"), !self.skip_tab, window, cx));
        let focused = focus.as_ref().is_some_and(|f| f.is_focused(window));
        let s = self.style(&components(), focused);
        debug_assert!(
            s.layout != ButtonLayout::IconOnly || self.name.as_ref().is_some_and(|n| !n.is_empty()),
            "button {:?} is an icon alone with no accessible name: use Button::icon(id, icon, name)",
            self.id
        );
        let g = s.geometry;
        // How far the spinner has faded in.
        let busy = progress(
            child_id(&self.id, "busy"),
            self.loading,
            MotionDuration::Normal,
            window,
            cx,
        );
        #[cfg(test)]
        probe::record(&self.id, probe::Drawn { style: s, busy });

        // A larger pointer target is an element of its own around the face, which takes the
        // click and the focus; the face shows hover and press while the pointer is anywhere on it.
        let target = self
            .target
            .filter(|t| *t > g.height || s.width.is_some_and(|w| *t > w));
        let pointer = match target {
            Some(_) => Pointer::Group(TARGET_GROUP.into()),
            None => Pointer::Own,
        };
        let face_id = match target {
            Some(_) => child_id(&self.id, "face"),
            None => self.id.clone(),
        };
        // An icon follows the pointer through the group that takes it.
        let group: SharedString = match &pointer {
            Pointer::Group(group) => group.clone(),
            Pointer::Own => FACE_GROUP.into(),
        };

        let mut face = div()
            .id(face_id)
            .debug_selector(|| part_name(&self.id, "button"))
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .control_geometry(&g);
        face = match s.width {
            // An icon-only button is square.
            Some(w) => face.w(px(w)).flex_shrink_0(),
            None if self.full_width => face.w_full().min_w_0(),
            None => face.flex_shrink_0(),
        };
        let draws_icon = [&self.leading, &self.trailing]
            .into_iter()
            .any(|g| matches!(g, Some(Glyph::Icon(_))));
        if draws_icon && target.is_none() {
            face = face.group(FACE_GROUP);
        }
        if s.underline {
            face = face.underline();
        }
        // Last, so hover and press keep the type role and the underline.
        face = pointer_states(face, &s.colours, s.responds, &pointer);

        // The spinner takes the first icon's place, or is laid over the whole inside of the
        // button when it has none. Whatever it replaces keeps its space and fades out.
        let spinner = |part: &str| {
            let name = part_name(&self.id, part);
            div()
                .debug_selector(|| name)
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .opacity(busy)
                .child(Spinner::new(child_id(&self.id, "spinner")).glyph_size(s.spinner_size))
        };
        let spinner_on_icon = self.leading.is_some() || self.trailing.is_some();
        let icon = |content: AnyElement, part: &str, takes_spinner: bool| {
            let name = part_name(&self.id, part);
            let shown = if takes_spinner { 1.0 - busy } else { 1.0 };
            let slot = div()
                .debug_selector(|| name)
                .relative()
                .flex_shrink_0()
                .size(px(g.icon_size))
                .child(div().size_full().opacity(shown).child(content));
            if takes_spinner && busy > 0.0 {
                slot.child(spinner("spinner"))
            } else {
                slot
            }
        };

        let has_leading = self.leading.is_some();
        if let Some(leading) = self.leading {
            let content = glyph(&s, leading, child_id(&self.id, "glyph"), group.clone());
            face = face.child(icon(content, "icon", true));
        }
        let flow = self.applied.flow(FlowKind::End);
        let mut clipped = false;
        if s.layout != ButtonLayout::IconOnly {
            let name = part_name(&self.id, "label");
            let shown = if spinner_on_icon { 1.0 } else { 1.0 - busy };
            let size = px(components().foundation().typography.style(g.type_role).size);
            let lines = decoration::flow_lines(flow).unwrap_or(1);
            let (was, probe) =
                decoration::truncation_probe(&self.id, self.label.clone(), size, lines, window, cx);
            clipped = was && flow != FlowKind::Wrap;
            face = face.child(
                decoration::apply_flow(div().debug_selector(|| name).min_w_0(), flow)
                    .opacity(shown)
                    .child(self.label.clone())
                    .child(probe),
            );
        }
        if let Some(trailing) = self.trailing {
            let content = glyph(
                &s,
                trailing,
                child_id(&self.id, "trailing-glyph"),
                group.clone(),
            );
            face = face.child(icon(content, "trailing-icon", !has_leading));
        }
        if !spinner_on_icon && busy > 0.0 {
            face = face.child(spinner("spinner"));
        }
        if s.focus_visible {
            let name = part_name(&self.id, "ring");
            face =
                face.child(focus_ring(&s.focus, g.radius, g.border_width).debug_selector(|| name));
        }

        let interactive = match target {
            Some(t) => {
                let name = part_name(&self.id, "target");
                let mut outer = div()
                    .id(self.id.clone())
                    .debug_selector(|| name)
                    .group(TARGET_GROUP)
                    .flex()
                    .items_center()
                    .justify_center()
                    .min_h(px(t))
                    .min_w(px(t));
                outer = if self.full_width {
                    outer.w_full().min_w_0()
                } else {
                    outer.flex_shrink_0()
                };
                outer.child(face)
            }
            None => face,
        };
        let mut button = interactive;
        // Content width, not stretched across a column.
        button.style().align_self = Some(gpui::AlignSelf::Start);
        if let Some(focus) = &focus {
            button = button.track_focus(focus);
        }
        if let Some(handler) = self.on_chord.clone().filter(|_| !self.disabled) {
            button = listen(
                button,
                &self.id,
                self.listening,
                focused,
                handler,
                window,
                cx,
            );
        }
        if self.keeps_focus {
            // GPUI runs an element's own press listeners after it has marked the element pressed
            // and before it moves focus to it, and moves focus only if the press was not
            // prevented: so this keeps focus where it was and the pressed colours still show.
            button = button.on_mouse_down(gpui::MouseButton::Left, |_, window, _| {
                window.prevent_default();
            });
        }
        if s.responds {
            button = button.cursor_pointer();
            if let Some(handler) = self.on_click {
                button = button.on_click(handler);
            }
        }
        // The full label, only when the last frame measured it as truncated. The tooltip adds
        // no tab stop; Describe above still reports the full label (ADR 0058).
        decoration::overflow_tooltip(
            &self.id,
            self.label,
            clipped,
            if self.full_width {
                decoration::ClipFit::Fill
            } else {
                decoration::ClipFit::Hug
            },
            false,
            button,
        )
    }
}

/// A listening button's keys (ADR 0076): the next press is a chord, Escape a cancel, and the
/// release of a key it took presses nothing. Focus leaving while it listens cancels, once.
fn listen<E: InteractiveElement>(
    button: E,
    id: &ElementId,
    listening: bool,
    focused: bool,
    handler: ChordHandler,
    window: &mut Window,
    cx: &mut App,
) -> E {
    // The key it took, until that key is released.
    let taken = window.use_keyed_state(child_id(id, "chord-taken"), cx, |_, _| None::<String>);
    // Whether it has said that focus left, so it says so once.
    let left = window.use_keyed_state(child_id(id, "chord-left"), cx, |_, _| false);
    let said = *left.read(cx);
    match (listening, focused) {
        (true, false) if !said => {
            left.update(cx, |l, _| *l = true);
            let handler = handler.clone();
            window.defer(cx, move |window, cx| handler(None, window, cx));
        }
        (true, false) => {}
        _ if said => left.update(cx, |l, _| *l = false),
        _ => {}
    }
    let released = taken.clone();
    let button = button.capture_key_up(move |e: &KeyUpEvent, window, cx| {
        let was = released.update(cx, |t, _| t.take_if(|k| *k == e.keystroke.key));
        if was.is_some() {
            window.prevent_default();
            cx.stop_propagation();
        }
    });
    if !listening {
        return button;
    }
    button.on_key_down(move |e: &KeyDownEvent, window, cx| {
        let k = &e.keystroke;
        if k.key == "tab" {
            return;
        }
        cx.stop_propagation();
        window.prevent_default();
        if e.is_held {
            return;
        }
        taken.update(cx, |t, _| *t = Some(k.key.clone()));
        if k.key == "escape" && !k.modifiers.modified() {
            handler(None, window, cx);
            return;
        }
        let m = k.modifiers;
        let chord = Chord {
            key: SharedString::from(k.key.clone()),
            control: m.control,
            alt: m.alt,
            shift: m.shift,
            platform: m.platform,
            function: m.function,
        };
        handler(Some(&chord), window, cx);
    })
}

/// What each button drew on its last frame, for tests (`checklist/button.rs`): the style it
/// looked up and how far its spinner had faded in. Kept per thread, like the test theme.
#[cfg(test)]
pub(crate) mod probe {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use gpui::ElementId;
    use odk_theme::component::ButtonStyle;

    use crate::focus::id_path;

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub(crate) struct Drawn {
        pub style: ButtonStyle,
        pub busy: f32,
    }

    thread_local! {
        static DRAWN: RefCell<HashMap<String, Drawn>> = RefCell::new(HashMap::new());
    }

    pub(crate) fn record(id: &ElementId, drawn: Drawn) {
        DRAWN.with(|d| d.borrow_mut().insert(id_path(id), drawn));
    }

    /// What the button with id `id` drew on its last frame.
    pub(crate) fn last(id: &str) -> Drawn {
        DRAWN.with(|d| {
            *d.borrow()
                .get(id)
                .unwrap_or_else(|| panic!("no button {id:?} has been drawn"))
        })
    }
}
