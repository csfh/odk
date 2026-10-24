//! The Overlays page: the keyboard chord, dropdown and context menus, tooltips, popovers, the
//! hover card, the command palette and the diagnostics of the overlay tokens: everything that
//! opens beside what it belongs to. Dialogs, sheets and toasts, which cover or interrupt the
//! page, are on the Dialogs page. Everything here is a real component in the active Omarchy theme;
//! Tab to reach it, and use the toolbar's density and corner switches to see each at every
//! setting.

use odk::gpui::px;
use odk::prelude::*;
use odk::ui::{
    Align, Button, Command, CommandItem, ContextMenu, DropdownMenu, HoverCard, Kbd, MenuItem,
    Placement, Popover, Switch,
};

use crate::buttons::caption;
use crate::content::Content;
use crate::{Settings, diagnostics, page, section};

pub struct OverlaysPage {
    worst_case: bool,
    wrap: bool,
    density_view: Density,
    last: SharedString,
    command: bool,
    inspector: diagnostics::Inspector,
    diagnostics: diagnostics::Rows,
}

impl OverlaysPage {
    pub fn new(settings: &Entity<Settings>, cx: &mut Context<Self>) -> Self {
        cx.observe(settings, |this, settings, cx| {
            this.worst_case = settings.read(cx).worst_case;
            cx.notify();
        })
        .detach();
        Self {
            worst_case: settings.read(cx).worst_case,
            wrap: true,
            density_view: Density::Default,
            last: "nothing yet".into(),
            command: false,
            inspector: diagnostics::Inspector::new("menu item"),
            diagnostics: diagnostics::Rows::default(),
        }
    }
}

/// The components the diagnostics panel offers.
const COMPONENTS: [&str; 8] = [
    "menu item",
    "menu",
    "popover",
    "tooltip",
    "command",
    "kbd",
    "dialog",
    "toast",
];

impl Render for OverlaysPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = Content::get(self.worst_case);
        let entity = cx.entity();

        let wrap = self.wrap;
        let file = c.file_menu;
        let menus = h_flex()
            .flex_wrap()
            .gap(Space::Lg)
            .child(DropdownMenu::new("file", "File").menu({
                let e = entity.clone();
                move |m| {
                    let item = |(label, hint): (&'static str, &'static str)| {
                        let item = MenuItem::new(label);
                        if hint.is_empty() {
                            item
                        } else {
                            item.hint(hint)
                        }
                    };
                    m.item(item(file[0]).icon("▤").on_select(pick(&e, file[0].0)))
                        .item(item(file[1]).icon("▥").on_select(pick(&e, file[1].0)))
                        .separator()
                        .item(item(file[2]).checked(wrap).on_select({
                            let e = e.clone();
                            move |_, cx| {
                                e.update(cx, |g, cx| {
                                    g.wrap = !g.wrap;
                                    cx.notify();
                                })
                            }
                        }))
                        .item(item(file[3]).disabled(true))
                        .separator()
                        .item(item(file[4]).danger().on_select(pick(&e, file[4].0)))
                }
            }))
            .child(DropdownMenu::new("view", "View").menu({
                let e = entity.clone();
                let current = self.density_view;
                move |m| {
                    let mut m = m.heading("Density");
                    for d in Density::ALL {
                        m = m.item(MenuItem::new(d.name()).radio(current == d).on_select({
                            let e = e.clone();
                            move |_, cx| {
                                e.update(cx, |g, cx| {
                                    g.density_view = d;
                                    g.last = format!("{} density", d.name()).into();
                                    cx.notify();
                                })
                            }
                        }));
                    }
                    m.separator()
                        .heading("Panels")
                        .item(MenuItem::new("Sidebar").checked(true))
                        .item(MenuItem::new("Terminal").checked(false).hint("Ctrl+`"))
                }
            }))
            .child(
                ContextMenu::new(
                    "ctx",
                    "Clipboard",
                    div()
                        .p(Space::Lg)
                        .border_1()
                        .border_dashed()
                        .child(Text::new(c.context_target).muted()),
                )
                .menu({
                    let e = entity.clone();
                    move |m| {
                        m.item(
                            MenuItem::new("Copy")
                                .hint("Ctrl+C")
                                .on_select(pick(&e, "Copy")),
                        )
                        .item(
                            MenuItem::new("Paste")
                                .hint("Ctrl+V")
                                .on_select(pick(&e, "Paste")),
                        )
                        .separator()
                        .item(
                            MenuItem::new("Remove")
                                .danger()
                                .on_select(pick(&e, "Remove")),
                        )
                    }
                }),
            )
            .child(Text::new(format!("Last chosen: {}", self.last)).muted());

        let [save, close] = c.tooltips;
        let placements = h_flex().flex_wrap().gap(Space::Sm).children(
            [
                (Placement::Top, "Top"),
                (Placement::Right, "Right"),
                (Placement::Bottom, "Bottom"),
                (Placement::Left, "Left"),
            ]
            .map(|(placement, name)| {
                Popover::new(
                    SharedString::from(format!("pop-{name}")),
                    name,
                    Button::new(SharedString::from(format!("pop-{name}-button"))).label(name),
                )
                .placement(placement)
                .title(format!("{name} popover"))
                .content(move |_, _| {
                    Text::new("It goes to the other side when there is no room.").muted()
                })
            }),
        );
        let (hover_trigger, hover_title, hover_body) = c.hover;

        page(
            crate::Page::Overlays,
            v_flex()
                .flex_shrink_0()
                .gap(Space::Lg)
                .child(caption(
                    "Everything here follows the density and corners chosen in the toolbar. Tab reaches each trigger; Enter opens it; Escape closes it and gives focus back.",
                ))
                .child(section("Kbd"))
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap(Space::Sm)
                        .child(Text::new("Open the palette with"))
                        .child(Kbd::new(c.chords[0]))
                        .child(Text::new("or"))
                        .child(Kbd::new(c.chords[1])),
                )
                .child(section("Dropdown Menu and Context Menu"))
                .child(caption(
                    "Icons, shortcuts, a check, a radio set under a heading, a disabled item and a destructive one. Up and Down skip what cannot be chosen; type a letter to jump.",
                ))
                .child(menus)
                .child(section("Tooltip"))
                .child(caption(
                    "Hover rests 500 ms before it opens; focus opens it at once. It never takes focus. The last one is at the window's edge, so it sits on the other side.",
                ))
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap(Space::Sm)
                        .child(Tooltip::new(
                            "tip-save",
                            save,
                            Button::new("tip-save-button").label("Save"),
                        ))
                        .child(Tooltip::new(
                            "tip-close",
                            close,
                            Button::new("tip-close-button")
                                .label("×")
                                .variant(ButtonVariant::Ghost)
                                .accessible_name("Close"),
                        ))
                        .child(
                            Tooltip::new(
                                "tip-long",
                                "A tooltip that needs more than one line wraps at a comfortable width and stays readable.",
                                Button::new("tip-long-button").label("Multiline"),
                            )
                            .placement(Placement::Bottom),
                        )
                        .child(
                            Tooltip::new(
                                "tip-side",
                                "Beside the button",
                                Button::new("tip-side-button").label("Right"),
                            )
                            .placement(Placement::Right),
                        ),
                )
                .child(
                    h_flex().justify_end().child(
                        Tooltip::new(
                            "tip-edge",
                            "At the edge of the window",
                            Button::new("tip-edge-button").label("Edge"),
                        )
                        .align(Align::End),
                    ),
                )
                .child(section("Popover"))
                .child(caption(
                    "Opens on a press or Enter and takes focus; Tab walks its controls and carries on past them. A press outside closes it. Four placements:",
                ))
                .child(placements)
                .child(h_flex().flex_wrap().gap(Space::Sm).child(filter_popover()).child(
                    h_flex().flex_1().justify_end().child(
                        Popover::new(
                            "pop-edge",
                            "At the edge",
                            Button::new("pop-edge-button").label("Edge"),
                        )
                        .align(Align::End)
                        .title("Near the edge")
                        .content(|_, _| {
                            Text::new("Lines up with the end of its trigger and is shifted back inside the window.")
                                .muted()
                        }),
                    ),
                ))
                .child(section("Hover Card"))
                .child(h_flex().child(
                    HoverCard::new("hover", "About the account", Text::new(hover_trigger)).content(
                        move |_, _| {
                            v_flex()
                                .gap(Space::Sm)
                                .child(Heading::new(hover_title).level(3))
                                .child(Text::new(hover_body).muted())
                                .child(Button::new("follow").label("Follow").size(Size::Sm))
                        },
                    ),
                ))
                .child(section("Command"))
                .child(
                    Button::new("open-command")
                        .label("Open the palette…")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.command = true;
                            cx.notify();
                        })),
                )
                .child({
                    let (placeholder, labels, empty) = c.commands;
                    let close = entity.clone();
                    let mut command = Command::new("command", "Commands")
                        .open(self.command)
                        .placeholder(placeholder)
                        .empty(empty)
                        .on_close(move |_, cx| {
                            close.update(cx, |g, cx| {
                                g.command = false;
                                cx.notify();
                            })
                        });
                    for (n, label) in labels.into_iter().enumerate() {
                        let item = CommandItem::new(label).on_select(pick(&entity, label));
                        command = command.item(match n {
                            0 => item.hint("Ctrl+O").keywords("load"),
                            1 => item.hint("Ctrl+S"),
                            2 => item.disabled(true),
                            3 => item.keywords("quit exit"),
                            _ => item,
                        });
                    }
                    command
                })
                .child(section("Diagnostics"))
                .child(caption(
                    "The tokens one overlay draws with, where each came from, and its contrast.",
                ))
                .child(diagnostics::panel(
                    self,
                    "overlays",
                    &COMPONENTS,
                    |g| &mut g.diagnostics,
                    |g| &mut g.inspector,
                    cx,
                )),
        )
    }
}

/// A popover with form controls in it: a field and a switch, and a button that closes it.
fn filter_popover() -> Popover {
    Popover::new(
        "pop-filter",
        "Filter",
        Button::new("pop-filter-button").label("Filter…"),
    )
    .title("Filter results")
    .content(|_, _| {
        v_flex()
            .gap(Space::Sm)
            .min_w(px(240.0))
            .child(Input::new("pop-filter-text", "Contains").placeholder("Contains…"))
            .child(Switch::new("pop-filter-unread", "Unread only"))
    })
}

/// A menu action that records which item was chosen.
fn pick(
    page: &Entity<OverlaysPage>,
    what: &'static str,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let page = page.clone();
    move |_, cx| {
        page.update(cx, |g, cx| {
            g.last = what.into();
            cx.notify();
        })
    }
}
