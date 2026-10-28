//! The gallery's pages as data: what each is called, which sidebar group it belongs to, what it
//! shows, and what to search it by. The sidebar, the breadcrumb, the page header, the Overview's
//! coverage list and the search all read this one table, so a page is added in one place
//! (`docs/gallery.md`, "Adding a page").

use std::sync::OnceLock;

use odk::theme::PaletteKey;
use odk::theme::foundation::TypeRole;
use odk::theme::semantic::{Emphasis, Role, State};

/// The sidebar's groups, in order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    Overview,
    Actions,
    Forms,
    Navigation,
    Feedback,
    Overlays,
    Data,
    Foundation,
    Tools,
}

impl Category {
    pub const ALL: [Category; 9] = [
        Category::Overview,
        Category::Actions,
        Category::Forms,
        Category::Navigation,
        Category::Feedback,
        Category::Overlays,
        Category::Data,
        Category::Foundation,
        Category::Tools,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Category::Overview => "Overview",
            Category::Actions => "Actions",
            Category::Forms => "Forms",
            Category::Navigation => "Navigation",
            Category::Feedback => "Feedback",
            Category::Overlays => "Overlays",
            Category::Data => "Data",
            Category::Foundation => "Foundation",
            Category::Tools => "Tools",
        }
    }
}

/// The pages, in the order of the sidebar. The ids of the first nine are the ones the gallery
/// has always had (`GALLERY_PAGE`, `tools/idle-cpu`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Overview,
    Buttons,
    Fields,
    Selection,
    Form,
    Navigation,
    Feedback,
    Overlays,
    Dialogs,
    Data,
    Colors,
    Resolver,
    Typography,
    Spacing,
    Geometry,
    Motion,
    Interaction,
    Playground,
    Compare,
    Diagnostics,
}

/// What the table says about a page.
pub struct Meta {
    pub id: &'static str,
    pub title: &'static str,
    pub category: Category,
    pub description: &'static str,
    /// The components the page shows, for the Overview's coverage list and for search.
    pub components: &'static [&'static str],
    /// Words a person might search for.
    pub keywords: &'static [&'static str],
    /// Token names the page shows, for search. Names generated from the token enums are in
    /// [`tokens`].
    pub tokens: &'static [&'static str],
    /// What to know about using the page's components with the keyboard and assistive
    /// technology; empty for a page with none.
    pub accessibility: &'static str,
}

impl Page {
    pub const ALL: [Page; 20] = [
        Page::Overview,
        Page::Buttons,
        Page::Fields,
        Page::Selection,
        Page::Form,
        Page::Navigation,
        Page::Feedback,
        Page::Overlays,
        Page::Dialogs,
        Page::Data,
        Page::Colors,
        Page::Resolver,
        Page::Typography,
        Page::Spacing,
        Page::Geometry,
        Page::Motion,
        Page::Interaction,
        Page::Playground,
        Page::Compare,
        Page::Diagnostics,
    ];

    pub fn id(self) -> &'static str {
        self.meta().id
    }

    pub fn title(self) -> &'static str {
        self.meta().title
    }

    pub fn from_id(id: &str) -> Option<Page> {
        Page::ALL.into_iter().find(|p| p.id() == id)
    }

    pub fn meta(self) -> &'static Meta {
        match self {
            Page::Overview => &Meta {
                id: "overview",
                title: "Overview",
                category: Category::Overview,
                description: "What the gallery is for, how to use it, and which components it covers.",
                components: &[],
                keywords: &[
                    "start",
                    "help",
                    "coverage",
                    "about",
                    "keyboard",
                    "shortcuts",
                ],
                tokens: &[],
                accessibility: "",
            },
            Page::Buttons => &Meta {
                id: "buttons",
                title: "Buttons",
                category: Category::Actions,
                description: "Eight variants at three densities, with text, icons, loading and disabled states.",
                components: &["Button", "Icon button"],
                keywords: &[
                    "action",
                    "click",
                    "primary",
                    "secondary",
                    "outline",
                    "ghost",
                    "destructive",
                    "success",
                    "warning",
                    "link",
                    "emphasis",
                    "loading",
                ],
                tokens: &[
                    "button.primary",
                    "button.secondary",
                    "button.outline",
                    "button.ghost",
                    "button.destructive",
                    "button.success",
                    "button.warning",
                    "button.link",
                ],
                accessibility: "Tab reaches a button and shows its 2 px ring; Enter or Space activates it once, on the key's release. A disabled button is skipped by Tab. A loading button keeps its label, ignores presses and says it is busy. An icon-only button needs an accessible name.",
            },
            Page::Fields => &Meta {
                id: "fields",
                title: "Text fields",
                category: Category::Forms,
                description: "Input, search, password and number fields, and the textarea, in every state.",
                components: &[
                    "Input",
                    "SearchInput",
                    "PasswordInput",
                    "NumberInput",
                    "Textarea",
                    "InputGroup",
                ],
                keywords: &[
                    "text",
                    "field",
                    "input",
                    "search",
                    "password",
                    "number",
                    "textarea",
                    "invalid",
                    "required",
                    "read-only",
                    "readonly",
                    "validation",
                ],
                tokens: &["field.default", "field.invalid", "field.disabled"],
                accessibility: "A field is one Tab stop and takes the keyboard when focused. Its label, helper text, required and invalid marks are announced; an invalid field is marked by a glyph as well as its colour. The number field steps with Up and Down.",
            },
            Page::Selection => &Meta {
                id: "selection",
                title: "Selection",
                category: Category::Forms,
                description: "Select, command list, check box, radio group and switch.",
                components: &["Select", "Command", "Checkbox", "RadioGroup", "Switch"],
                keywords: &[
                    "select",
                    "combobox",
                    "dropdown",
                    "checkbox",
                    "radio",
                    "switch",
                    "toggle",
                    "choose",
                    "searchable list",
                    "command",
                ],
                tokens: &[
                    "checkbox.checked",
                    "radio.selected",
                    "switch.on",
                    "select.open",
                ],
                accessibility: "Space toggles a check box or a switch. A radio group is one Tab stop; the arrow keys move and select inside it. A select opens with Enter, Space or Down, moves with the arrow keys and typeahead, and closes with Escape, giving focus back to the field.",
            },
            Page::Form => &Meta {
                id: "form",
                title: "Form",
                category: Category::Forms,
                description: "Every state of the controls, and a complete form with labels, helper text, validation and actions.",
                components: &["FormField", "form()"],
                keywords: &[
                    "form",
                    "label",
                    "helper",
                    "validation",
                    "required",
                    "error",
                    "submit",
                    "field composition",
                    "formfield",
                ],
                tokens: &["form_field.label", "form_field.message"],
                accessibility: "Every control has a visible label; required and invalid are announced and marked by a glyph, not by colour alone. An error message is associated with its control.",
            },
            Page::Navigation => &Meta {
                id: "navigation",
                title: "Navigation",
                category: Category::Navigation,
                description: "Tabs, segmented controls, navigation lists and breadcrumbs.",
                components: &["Tabs", "ToggleGroup", "NavList", "Breadcrumb"],
                keywords: &[
                    "tabs",
                    "segmented",
                    "toggle group",
                    "sidebar",
                    "nav",
                    "breadcrumb",
                    "link",
                    "button group",
                ],
                tokens: &["tab.selected", "segment.selected", "nav_item.current"],
                accessibility: "Each composite control is one Tab stop; the arrow keys move inside it and Home and End jump to its ends. Tabs select on focus unless they are manually activated; a navigation list opens a page only on Enter, Space or a click.",
            },
            Page::Feedback => &Meta {
                id: "feedback",
                title: "Feedback",
                category: Category::Feedback,
                description: "Badges, status indicators, alerts, progress, spinners, skeletons and separators.",
                components: &[
                    "Badge",
                    "StatusIndicator",
                    "Alert",
                    "Progress",
                    "Spinner",
                    "Skeleton",
                    "Separator",
                ],
                keywords: &[
                    "badge",
                    "status",
                    "alert",
                    "progress",
                    "spinner",
                    "loading",
                    "skeleton",
                    "separator",
                    "tone",
                    "danger",
                    "warning",
                    "success",
                    "info",
                ],
                tokens: &[
                    "badge.danger",
                    "alert.warning",
                    "status_dot.success",
                    "progress.accent",
                ],
                accessibility: "Status never rests on colour alone: each tone carries a mark (i, ✓, !, ×) and a word. Progress reports its value; a spinner stands still when motion is reduced.",
            },
            Page::Overlays => &Meta {
                id: "overlays",
                title: "Overlays",
                category: Category::Overlays,
                description: "Dropdown and context menus, popovers, hover cards, tooltips and toasts.",
                components: &[
                    "DropdownMenu",
                    "ContextMenu",
                    "Popover",
                    "HoverCard",
                    "Tooltip",
                    "Toast",
                    "Kbd",
                ],
                keywords: &[
                    "menu",
                    "dropdown",
                    "context menu",
                    "popover",
                    "hover card",
                    "tooltip",
                    "toast",
                    "notification",
                    "placement",
                    "collision",
                    "kbd",
                    "shortcut",
                ],
                tokens: &["menu.item", "popover", "tooltip", "toast.info"],
                accessibility: "Enter or Space opens a trigger; Escape closes the overlay and gives focus back to the trigger; a press outside dismisses it and goes no further. Menus move with the arrow keys and typeahead. A toast does not take focus when it appears. Its stack is a tab stop, and it pauses while hovered or focused.",
            },
            Page::Dialogs => &Meta {
                id: "dialogs",
                title: "Dialogs and sheets",
                category: Category::Overlays,
                description: "Modal dialogs and sheets, and a side panel that does not block the page.",
                components: &["Dialog", "Sheet"],
                keywords: &[
                    "dialog",
                    "modal",
                    "confirm",
                    "sheet",
                    "drawer",
                    "panel",
                    "focus trap",
                    "scrim",
                ],
                tokens: &["dialog", "sheet", "surface.scrim"],
                accessibility: "A modal holds focus: Tab cycles inside it, Escape closes it, and focus returns to the control that opened it. The window behind it does not respond.",
            },
            Page::Data => &Meta {
                id: "data",
                title: "Data",
                category: Category::Data,
                description: "Resizable panels, a data table that draws only the rows in view, charts on a canvas, and images.",
                components: &["Resizable", "DataTable", "Chart", "Image"],
                keywords: &[
                    "resizable",
                    "split",
                    "splitter",
                    "panel",
                    "pane",
                    "drag",
                    "handle",
                    "layout",
                    "table",
                    "data table",
                    "grid",
                    "sort",
                    "rows",
                    "virtualised",
                    "processes",
                    "chart",
                    "graph",
                    "plot",
                    "sparkline",
                    "heatmap",
                    "canvas",
                    "image",
                    "avatar",
                    "picture",
                    "wallpaper",
                ],
                tokens: &["resize handle", "table row", "table header", "chart"],
                accessibility: "Each handle is a tab stop and a separator whose value is the size of the panel before it. The arrow keys along the group move it, Home and End take that panel to its limits, and Escape puts back a drag in progress. A table's sortable headers are one tab stop (Left and Right, Enter sorts) and its rows the next (Up and Down select, Enter activates, the Menu key opens a row's menu); a cell's button takes the pointer only, and its action is in the menu. A chart reports an image with a description in words; one that pans or zooms is a tab stop, with the arrows, + and - and Enter.",
            },
            Page::Colors => &Meta {
                id: "colors",
                title: "Colors",
                category: Category::Foundation,
                description: "The theme's source palette, and the semantic colours resolved from it.",
                components: &[],
                keywords: &[
                    "colour", "color", "palette", "theme", "semantic", "role", "accent", "neutral",
                    "danger", "success", "warning", "info", "solid", "muted", "subtle", "contrast",
                    "swatch", "hex",
                ],
                tokens: &[
                    "surface.canvas",
                    "text.primary",
                    "border.default",
                    "roles.accent.solid",
                ],
                accessibility: "Contrast ratios here are measured on the resolved tokens (WCAG 2.2 text 4.5:1, non-text 3:1). They are not a test of a platform accessibility tree.",
            },
            Page::Resolver => &Meta {
                id: "resolver",
                title: "Colour resolver",
                category: Category::Foundation,
                description: "Why the Adaptive Colour Resolver chose each colour: candidates, contrast, and the reason.",
                components: &[],
                keywords: &[
                    "resolver",
                    "adaptive",
                    "oklch",
                    "candidate",
                    "bright",
                    "regular",
                    "contrast",
                    "adjusted",
                    "strategy",
                    "decision",
                    "warning",
                    "note",
                    "why",
                    "explain",
                ],
                tokens: &[],
                accessibility: "",
            },
            Page::Typography => &Meta {
                id: "typography",
                title: "Typography",
                category: Category::Foundation,
                description: "The type roles: family, size, line height, weight and letter spacing.",
                components: &["Text", "Heading"],
                keywords: &[
                    "font",
                    "type",
                    "text",
                    "size",
                    "line height",
                    "weight",
                    "mono",
                    "monospace",
                    "heading",
                    "title",
                    "caption",
                    "letter spacing",
                    "truncate",
                    "wrap",
                ],
                tokens: &[],
                accessibility: "Text follows the user's text size (`omarchy display text size`) and Omarchy's font.",
            },
            Page::Spacing => &Meta {
                id: "spacing",
                title: "Spacing",
                category: Category::Foundation,
                description: "The 4 px spacing scale and the spacing named for a purpose.",
                components: &[],
                keywords: &[
                    "space",
                    "gap",
                    "padding",
                    "margin",
                    "scale",
                    "grid",
                    "layout",
                    "panel",
                    "dialog",
                    "section",
                    "form gap",
                    "field gap",
                ],
                tokens: &[],
                accessibility: "",
            },
            Page::Geometry => &Meta {
                id: "geometry",
                title: "Geometry",
                category: Category::Foundation,
                description: "Corner radius, borders, the focus ring, and control heights at each density.",
                components: &[],
                keywords: &[
                    "radius",
                    "corner",
                    "square",
                    "subtle",
                    "border",
                    "focus",
                    "ring",
                    "height",
                    "density",
                    "compact",
                    "comfortable",
                    "default",
                ],
                tokens: &[],
                accessibility: "The focus indicator is 2 px, drawn outside the control, so a ring never moves layout or meets a solid fill.",
            },
            Page::Motion => &Meta {
                id: "motion",
                title: "Motion",
                category: Category::Foundation,
                description: "Durations and easing, and the components that use them, with play and reset.",
                components: &[],
                keywords: &[
                    "animation",
                    "duration",
                    "easing",
                    "transition",
                    "reduced motion",
                    "fast",
                    "normal",
                    "slow",
                    "enter",
                    "exit",
                    "instant",
                    "curve",
                    "bezier",
                ],
                tokens: &[],
                accessibility: "With reduced motion every transition runs for zero time: the end state shows at once and nothing waits for an animation. A spinner stays visible but stands still.",
            },
            Page::Interaction => &Meta {
                id: "interaction",
                title: "Accessibility",
                category: Category::Foundation,
                description: "Real controls with a log of the events they produce: focus, keys, dialogs, motion and status without colour.",
                components: &[],
                keywords: &[
                    "accessibility",
                    "a11y",
                    "keyboard",
                    "focus",
                    "tab",
                    "event log",
                    "reduced motion",
                    "screen reader",
                    "radio",
                    "menu",
                    "checkbox",
                    "escape",
                ],
                tokens: &[],
                accessibility: "The log records events the controls actually emitted. Contrast figures are token-level; the gallery has not been tested with a screen reader.",
            },
            Page::Playground => &Meta {
                id: "playground",
                title: "Playground",
                category: Category::Tools,
                description: "Change the properties of a real button or form control and copy the code.",
                components: &["Button", "Input", "Checkbox", "Switch"],
                keywords: &[
                    "playground",
                    "configure",
                    "variant",
                    "emphasis",
                    "properties",
                    "code",
                    "copy",
                    "try",
                ],
                tokens: &[],
                accessibility: "",
            },
            Page::Compare => &Meta {
                id: "compare",
                title: "Theme comparison",
                category: Category::Tools,
                description: "Two installed themes side by side as resolved tokens, and one after the other as real components.",
                components: &[],
                keywords: &[
                    "compare",
                    "theme",
                    "difference",
                    "diff",
                    "light",
                    "dark",
                    "side by side",
                    "a/b",
                ],
                tokens: &[],
                accessibility: "",
            },
            Page::Diagnostics => &Meta {
                id: "diagnostics",
                title: "Theme diagnostics",
                category: Category::Tools,
                description: "What the theme manager knows: the system theme, the preview, the resolver's strategy, warnings and contrast issues.",
                components: &[],
                keywords: &[
                    "diagnostics",
                    "theme",
                    "generation",
                    "fallback",
                    "warnings",
                    "missing",
                    "strategy",
                    "monochrome",
                    "issues",
                ],
                tokens: &[],
                accessibility: "",
            },
        }
    }
}

/// A search result: the page, and what matched when it was not the page's own name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub page: Page,
    pub matched: Option<String>,
}

/// The token names generated from the token enums, with the page that shows each. Built once;
/// the static names of [`Meta::tokens`] are searched as well.
pub fn tokens() -> &'static [(String, Page)] {
    static NAMES: OnceLock<Vec<(String, Page)>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let mut names = Vec::new();
        for key in PaletteKey::ALL {
            names.push((format!("palette.{}", key.name()), Page::Colors));
        }
        for role in Role::ALL {
            names.push((format!("roles.{role}.focus"), Page::Colors));
            for emphasis in Emphasis::ALL {
                for state in State::ALL {
                    names.push((
                        format!("roles.{role}.{}.{}", emphasis.name(), state.name()),
                        Page::Colors,
                    ));
                }
            }
            for what in ["solid fill", "text", "focus"] {
                names.push((format!("{role} {what}"), Page::Resolver));
            }
        }
        for token in crate::colors::SURFACES {
            names.push((format!("surface.{}", token.name()), Page::Colors));
        }
        for token in crate::colors::TEXTS {
            names.push((format!("text.{}", token.name()), Page::Colors));
        }
        for token in crate::colors::BORDERS {
            names.push((format!("border.{}", token.name()), Page::Colors));
        }
        for role in TypeRole::ALL {
            names.push((format!("type.{}", role.name()), Page::Typography));
        }
        for step in crate::spacing::STEPS {
            names.push((format!("space.{}", step.name), Page::Spacing));
        }
        for (name, _) in crate::spacing::LAYOUT {
            names.push((format!("layout.{name}"), Page::Spacing));
        }
        for name in [
            "radius.none",
            "radius.default",
            "radius.small",
            "radius.overlay",
            "border.width",
            "focus.width",
            "focus.offset",
            "thumb.gap",
            "control.height.compact",
            "control.height.default",
            "control.height.comfortable",
        ] {
            names.push((name.into(), Page::Geometry));
        }
        for name in crate::motion::DURATIONS.iter().map(|(n, _)| *n) {
            names.push((format!("motion.{name}"), Page::Motion));
        }
        names.push(("motion.easing".into(), Page::Motion));
        names
    })
}

/// The pages that match `query`, best first: every word of the query must be part of the page's
/// title, group, description, a component, a keyword or a token name. A page whose title
/// matches comes first, then one that matches by a component or a token name, then the rest;
/// pages keep the sidebar's order within each. An empty query matches nothing.
pub fn search(query: &str) -> Vec<Hit> {
    let words: Vec<String> = query.split_whitespace().map(|w| w.to_lowercase()).collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<(u8, Hit)> = Vec::new();
    for page in Page::ALL {
        let meta = page.meta();
        let title = meta.title.to_lowercase();
        let group = meta.category.title().to_lowercase();
        let plain: Vec<String> = meta
            .components
            .iter()
            .chain(meta.keywords)
            .map(|s| s.to_lowercase())
            .collect();
        let names: Vec<&str> = meta
            .tokens
            .iter()
            .copied()
            .chain(
                tokens()
                    .iter()
                    .filter(|(_, p)| *p == page)
                    .map(|(n, _)| n.as_str()),
            )
            .collect();
        let description = meta.description.to_lowercase();

        // The thing each word matched, if it was not the title or the group.
        let mut matched: Option<String> = None;
        let mut rank = 0u8;
        let mut all = true;
        for word in &words {
            if title.contains(word.as_str()) || group.contains(word.as_str()) {
                continue;
            }
            if let Some(name) = names
                .iter()
                .find(|n| n.to_lowercase().contains(word.as_str()))
            {
                matched.get_or_insert_with(|| (*name).to_owned());
                rank = rank.max(1);
            } else if let Some(word) = meta
                .components
                .iter()
                .chain(meta.keywords)
                .zip(plain.iter())
                .find(|(_, lower)| lower.contains(word.as_str()))
                .map(|(shown, _)| *shown)
            {
                matched.get_or_insert_with(|| word.to_owned());
                rank = rank.max(1);
            } else if description.contains(word.as_str()) {
                rank = rank.max(2);
            } else {
                all = false;
                break;
            }
        }
        if all {
            hits.push((rank, Hit { page, matched }));
        }
    }
    hits.sort_by_key(|(rank, _)| *rank);
    hits.into_iter().map(|(_, hit)| hit).collect()
}
