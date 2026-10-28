//! The ODK component gallery and design-system explorer: every ODK component in each of its
//! states, the foundation tokens and the colour resolver's reasoning, for looking at, for trying
//! with the keyboard and for checking a theme against. One page at a time, chosen in the sidebar
//! or found with its search; a toolbar above it sets the theme (a preview that leaves Omarchy's
//! alone), density, corners and motion for every page at once; a status bar below it says what
//! is in force. `docs/gallery.md` is the guide.
//!
//! Each page is a view of its own, filling the space beside the sidebar and scrolling inside it,
//! so GPUI can keep what a page drew and build only the page that changed: a spinner turning on
//! the Feedback page redraws that page, not the nineteen of them (`docs/checks.md`, "Frame
//! cost"). The toolbar, sidebar and status bar are outside the pages' cached views.
//!
//! A library as well as the program, so the benchmarks in `benches/` can open the same window.

pub mod buttons;
pub mod colors;
pub mod compare;
pub mod content;
pub mod data;
mod diagnostics;
pub mod dialogs;
pub mod feedback;
pub mod forms;
pub mod geometry;
pub mod interaction;
pub mod layout;
pub mod motion;
pub mod navigation;
pub mod overlays;
pub mod overview;
pub mod playground;
mod present;
pub mod registry;
pub mod resolver;
pub mod snippets;
pub mod spacing;
mod theme_page;
pub mod themes;
pub mod typography;

use odk::gpui::{AnyView, StyleRefinement, px, rgba};
use odk::prelude::*;
use odk::theme::foundation::CornerRadius;

pub use present::page_renders;
pub(crate) use present::{page, section};
pub use registry::{Category, Hit, Page, search};

use crate::buttons::choice_labelled;
use crate::layout::{NARROW_BELOW, SIDEBAR_NARROW_WIDTH, SIDEBAR_WIDTH, THEME_SELECT_WIDTH};
use crate::themes::{Themes, pretty};

/// How wide the gallery's text inputs may get: a field fills its column, so the column is
/// capped to look like a form.
pub(crate) const INPUT_WIDTH: f32 = 360.0;

/// The value of the theme selector that follows the system's theme.
const FOLLOW: &str = "~system";

/// The settings every page follows.
pub struct Settings {
    pub worst_case: bool,
}

/// What the sidebar lists: the pages, or what a search found.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SidebarContent {
    Pages,
    Results(usize),
    NoMatches,
}

pub struct Gallery {
    page: Page,
    settings: Entity<Settings>,
    themes: Entity<Themes>,
    query: SharedString,
    overview: Entity<overview::OverviewPage>,
    buttons: Entity<buttons::ButtonsPage>,
    fields: Entity<forms::FormsPage>,
    selection: Entity<forms::FormsPage>,
    form: Entity<forms::FormsPage>,
    navigation: Entity<navigation::NavigationPage>,
    feedback: Entity<feedback::FeedbackPage>,
    overlays: Entity<overlays::OverlaysPage>,
    dialogs: Entity<dialogs::DialogsPage>,
    data: Entity<data::DataPage>,
    colors: Entity<colors::ColorsPage>,
    resolver: Entity<resolver::ResolverPage>,
    typography: Entity<typography::TypographyPage>,
    spacing: Entity<spacing::SpacingPage>,
    geometry: Entity<geometry::GeometryPage>,
    motion: Entity<motion::MotionPage>,
    interaction: Entity<interaction::InteractionPage>,
    playground: Entity<playground::PlaygroundPage>,
    compare: Entity<compare::ComparePage>,
    diagnostics: Entity<theme_page::ThemePage>,
}

impl Gallery {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let settings = cx.new(|_| Settings { worst_case: false });
        let themes = cx.new(|_| Themes::new(None));
        let me = cx.entity().downgrade();
        Self {
            page: Page::Overview,
            query: SharedString::default(),
            overview: cx.new(|_| overview::OverviewPage::new(me)),
            buttons: cx.new(|cx| buttons::ButtonsPage::new(&settings, cx)),
            fields: cx.new(|cx| forms::FormsPage::new(&settings, forms::FormsPart::Fields, cx)),
            selection: cx
                .new(|cx| forms::FormsPage::new(&settings, forms::FormsPart::Selection, cx)),
            form: cx.new(|cx| forms::FormsPage::new(&settings, forms::FormsPart::Form, cx)),
            navigation: cx.new(|cx| navigation::NavigationPage::new(&settings, cx)),
            feedback: cx.new(|cx| feedback::FeedbackPage::new(&settings, cx)),
            overlays: cx.new(|cx| overlays::OverlaysPage::new(&settings, cx)),
            dialogs: cx.new(|cx| dialogs::DialogsPage::new(&settings, cx)),
            data: cx.new(|cx| data::DataPage::new(&settings, cx)),
            colors: cx.new(|_| colors::ColorsPage),
            resolver: cx.new(|_| resolver::ResolverPage::new()),
            typography: cx.new(|_| typography::TypographyPage),
            spacing: cx.new(|_| spacing::SpacingPage),
            geometry: cx.new(|_| geometry::GeometryPage),
            motion: cx.new(|_| motion::MotionPage::new()),
            interaction: cx.new(|cx| interaction::InteractionPage::new(&settings, cx)),
            playground: cx.new(|_| playground::PlaygroundPage::new()),
            compare: cx.new(|_| compare::ComparePage::new(themes.clone())),
            diagnostics: cx.new(|_| theme_page::ThemePage::new(themes.clone())),
            themes,
            settings,
        }
    }

    /// The lines of the Interaction page's event log, oldest first. For the tests.
    pub fn interaction_log(&self, cx: &App) -> Vec<String> {
        self.interaction.read(cx).lines()
    }

    /// The page being shown.
    pub fn current(&self) -> Page {
        self.page
    }

    /// Shows `page`.
    pub fn show(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        if page == Page::Compare {
            self.compare.update(cx, |p, cx| p.ensure_loaded(cx));
        }
        cx.notify();
    }

    /// Switches the Content setting between the demo content and the worst case.
    pub fn set_worst_case(&mut self, worst_case: bool, cx: &mut Context<Self>) {
        self.settings.update(cx, |s, cx| {
            s.worst_case = worst_case;
            cx.notify();
        });
    }

    /// The text in the sidebar's search field.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Searches the pages and tokens, as the sidebar's field does.
    pub fn set_query(&mut self, query: &str, cx: &mut Context<Self>) {
        self.query = query.to_owned().into();
        cx.notify();
    }

    /// What the sidebar lists now.
    pub fn sidebar_content(&self) -> SidebarContent {
        if self.query.trim().is_empty() {
            SidebarContent::Pages
        } else {
            match self.hits().len() {
                0 => SidebarContent::NoMatches,
                n => SidebarContent::Results(n),
            }
        }
    }

    /// What the sidebar's search finds now.
    pub fn hits(&self) -> Vec<Hit> {
        search(&self.query)
    }

    /// The themes the gallery can show.
    pub fn themes(&self) -> &Entity<Themes> {
        &self.themes
    }

    /// The comparison page's state, for the tests.
    pub fn comparison(&self) -> &Entity<compare::ComparePage> {
        &self.compare
    }

    /// The playground's state, for the tests.
    pub fn playground(&self) -> &Entity<playground::PlaygroundPage> {
        &self.playground
    }

    /// The resolver page's state, for the tests.
    pub fn resolver(&self) -> &Entity<resolver::ResolverPage> {
        &self.resolver
    }

    /// Whether the user asks for reduced motion, as the compositor says (ADR 0048), which the
    /// gallery's switch can add to but never take back: the toolbar then says so instead of offering
    /// the choice.
    pub fn motion_forced_by_user(&self) -> bool {
        odk::ui::theme_snapshot().reduced_motion
    }

    /// The theme being previewed, if one is.
    pub fn previewing(&self, cx: &App) -> Option<String> {
        self.themes.read(cx).choice().map(str::to_owned)
    }

    /// Draws `theme`, a name or a directory, in place of the system's. Omarchy's selection and
    /// files are not touched.
    pub fn preview_theme(&mut self, theme: &str, cx: &mut Context<Self>) -> bool {
        let shown = self.themes.update(cx, |themes, cx| {
            let name = themes.add(theme);
            themes.preview(&name, cx)
        });
        cx.notify();
        shown
    }

    /// Draws the system's theme again.
    pub fn follow_system(&mut self, cx: &mut Context<Self>) {
        self.themes.update(cx, |themes, cx| themes.follow(cx));
        cx.notify();
    }

    /// Density, corners, motion and theme back to what the gallery starts with.
    pub fn reset_appearance(&mut self, cx: &mut Context<Self>) {
        self.follow_system(cx);
        odk::ui::configure(cx, |c| {
            c.density = Density::Default;
            c.corner_radius = CornerRadius::default();
            c.reduced_motion = false;
        });
    }

    /// Calls `f` whenever what `page` shows is marked as changed, which is what asks for it to be
    /// drawn again. For the tests that count redraws.
    pub fn observe_page(
        &self,
        page: Page,
        cx: &mut App,
        f: impl FnMut() + 'static,
    ) -> odk::gpui::Subscription {
        let f = std::rc::Rc::new(std::cell::RefCell::new(f));
        macro_rules! observe {
            ($page:expr) => {
                cx.observe(&$page, move |_, _| (f.borrow_mut())())
            };
        }
        match page {
            Page::Overview => observe!(self.overview),
            Page::Buttons => observe!(self.buttons),
            Page::Fields => observe!(self.fields),
            Page::Selection => observe!(self.selection),
            Page::Form => observe!(self.form),
            Page::Navigation => observe!(self.navigation),
            Page::Feedback => observe!(self.feedback),
            Page::Overlays => observe!(self.overlays),
            Page::Dialogs => observe!(self.dialogs),
            Page::Data => observe!(self.data),
            Page::Colors => observe!(self.colors),
            Page::Resolver => observe!(self.resolver),
            Page::Typography => observe!(self.typography),
            Page::Spacing => observe!(self.spacing),
            Page::Geometry => observe!(self.geometry),
            Page::Motion => observe!(self.motion),
            Page::Interaction => observe!(self.interaction),
            Page::Playground => observe!(self.playground),
            Page::Compare => observe!(self.compare),
            Page::Diagnostics => observe!(self.diagnostics),
        }
    }

    /// The view of `page`.
    pub fn view(&self, page: Page) -> AnyView {
        match page {
            Page::Overview => self.overview.clone().into(),
            Page::Buttons => self.buttons.clone().into(),
            Page::Fields => self.fields.clone().into(),
            Page::Selection => self.selection.clone().into(),
            Page::Form => self.form.clone().into(),
            Page::Navigation => self.navigation.clone().into(),
            Page::Feedback => self.feedback.clone().into(),
            Page::Overlays => self.overlays.clone().into(),
            Page::Dialogs => self.dialogs.clone().into(),
            Page::Data => self.data.clone().into(),
            Page::Colors => self.colors.clone().into(),
            Page::Resolver => self.resolver.clone().into(),
            Page::Typography => self.typography.clone().into(),
            Page::Spacing => self.spacing.clone().into(),
            Page::Geometry => self.geometry.clone().into(),
            Page::Motion => self.motion.clone().into(),
            Page::Interaction => self.interaction.clone().into(),
            Page::Playground => self.playground.clone().into(),
            Page::Compare => self.compare.clone().into(),
            Page::Diagnostics => self.diagnostics.clone().into(),
        }
    }

    /// The toolbar's theme selector chose `value`.
    fn choose_theme(&mut self, value: &str, cx: &mut Context<Self>) {
        if value == FOLLOW {
            self.follow_system(cx);
        } else {
            self.preview_theme(value, cx);
        }
    }

    fn toolbar(&mut self, narrow: bool, cx: &mut Context<Self>) -> impl IntoElement {
        // In a narrow window the groups keep their names for assistive technology and drop them
        // from the screen, so the toolbar is two lines and not four.
        let labels = !narrow;
        let config = odk::ui::foundation_config(cx);
        let (names, choice_now, error) = {
            let themes = self.themes.read(cx);
            (
                themes.names().to_vec(),
                themes.choice().map(str::to_owned),
                themes.error().map(str::to_owned),
            )
        };
        let system = themes::system_name(cx);
        let mut select = Select::new("theme", "Theme")
            .item(FOLLOW, format!("System ({system})"))
            .value(choice_now.clone().unwrap_or_else(|| FOLLOW.to_owned()));
        for name in &names {
            select = select.item(name.clone(), pretty(name));
        }
        let select =
            select.on_change(cx.listener(|this, value: &str, _, cx| this.choose_theme(value, cx)));

        let user_reduces = self.motion_forced_by_user();
        let reduced = odk::ui::foundation().motion.reduced;
        let motion = if user_reduces {
            h_flex()
                .gap(Space::Md)
                .child(Text::new("Motion").muted())
                .child(Text::new("Reduced by the system"))
                .into_any_element()
        } else {
            choice_labelled(
                "motion",
                "Motion",
                labels,
                vec![(false, "Full"), (true, "Reduced")],
                reduced,
                |_, reduced, cx| odk::ui::set_reduced_motion(cx, reduced),
                cx,
            )
            .into_any_element()
        };
        let worst = self.settings.read(cx).worst_case;
        let page = self.page.meta();
        h_flex()
            .flex_shrink_0()
            .flex_wrap()
            .gap(Space::Md)
            .gap_x(Space::Xl)
            .px(Space::Lg)
            .py(Space::Sm)
            .border_b_1()
            .border_color(present::edge())
            .child(
                h_flex()
                    .gap(Space::Sm)
                    .child(Text::new("ODK").muted())
                    .child(Text::new("/").muted())
                    .child(Text::new(page.category.title()).muted())
                    .child(Text::new("/").muted())
                    .child(Heading::new(page.title).level(3)),
            )
            .child(
                h_flex()
                    .gap(Space::Md)
                    .when(labels, |row| row.child(Text::new("Theme").muted()))
                    .child(div().w(px(THEME_SELECT_WIDTH)).child(select))
                    .when(choice_now.is_some(), |row| {
                        row.child(
                            Button::new("theme-follow")
                                .label("System theme")
                                .variant(ButtonVariant::Ghost)
                                .on_click(cx.listener(|this, _, _, cx| this.follow_system(cx))),
                        )
                    }),
            )
            .child(choice_labelled(
                "density",
                "Density",
                labels,
                Density::ALL.map(|d| (d, density_title(d))).to_vec(),
                config.density,
                |_, d, cx| odk::ui::configure(cx, |c| c.density = d),
                cx,
            ))
            .child(choice_labelled(
                "corners",
                "Corners",
                labels,
                [CornerRadius::Square, CornerRadius::Subtle]
                    .map(|r| (r, corner_title(r)))
                    .to_vec(),
                config.corner_radius,
                |_, r, cx| odk::ui::configure(cx, |c| c.corner_radius = r),
                cx,
            ))
            .child(motion)
            .child(choice_labelled(
                "content",
                "Content",
                labels,
                vec![(false, "Demo"), (true, "Worst case")],
                worst,
                |this, worst, cx| this.set_worst_case(worst, cx),
                cx,
            ))
            .child(
                Button::new("reset-appearance")
                    .label("Reset")
                    .variant(ButtonVariant::Ghost)
                    .on_click(cx.listener(|this, _, _, cx| this.reset_appearance(cx))),
            )
            .when_some(error, |row, error| {
                row.child(Text::new(format!("⚠ {error}")))
            })
    }

    fn sidebar(&mut self, narrow: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let width = if narrow {
            SIDEBAR_NARROW_WIDTH
        } else {
            SIDEBAR_WIDTH
        };
        let theme = odk::ui::theme_snapshot();
        let searching = self.sidebar_content() != SidebarContent::Pages;

        let nav = if searching {
            let hits = self.hits();
            if hits.is_empty() {
                Text::new(format!(
                    "Nothing matches “{}”. Try a component, a category, or a token such as space.4.",
                    self.query.trim()
                ))
                .muted()
                .into_any_element()
            } else {
                let mut list = NavList::new("results", "Search results");
                for hit in &hits {
                    list = list.item(hit.page.id(), hit.page.title());
                    if let Some(matched) = &hit.matched {
                        list = list.counter(hit.page.id(), matched.clone());
                    }
                }
                list.on_change(cx.listener(|this, value: &SharedString, _, cx| {
                    if let Some(page) = Page::from_id(value) {
                        this.query = SharedString::default();
                        this.show(page, cx);
                    }
                }))
                .into_any_element()
            }
        } else {
            let mut list = NavList::new("pages", "Pages").value(self.page.id());
            for category in Category::ALL {
                if category != Category::Overview {
                    list = list.group(category.title());
                }
                for page in Page::ALL
                    .into_iter()
                    .filter(|p| p.meta().category == category)
                {
                    list = list.item(page.id(), page.title());
                }
            }
            list.on_change(cx.listener(|this, value: &SharedString, _, cx| {
                if let Some(page) = Page::from_id(value) {
                    this.show(page, cx);
                }
            }))
            .into_any_element()
        };

        v_flex()
            .flex_shrink_0()
            .w(px(width))
            .h_full()
            .bg(rgba(theme.semantic.surface.recessed.to_rgba_u32()))
            .border_r_1()
            .border_color(present::edge())
            .child(
                div().p(Space::Sm).child(
                    SearchInput::new("search", "Search components and tokens")
                        .placeholder("Search")
                        .value(self.query.clone())
                        .on_change(cx.listener(|this, value: &str, _, cx| {
                            this.set_query(value, cx);
                        }))
                        .on_submit(cx.listener(|this, _: &str, _, cx| {
                            if let Some(hit) = this.hits().first() {
                                let page = hit.page;
                                this.query = SharedString::default();
                                this.show(page, cx);
                            }
                        })),
                ),
            )
            .child(
                div()
                    .id("sidebar-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(Space::Sm)
                    .pb(Space::Sm)
                    .child(nav),
            )
    }

    fn status_bar(&self, cx: &App) -> impl IntoElement {
        let config = odk::ui::foundation_config(cx);
        let f = odk::ui::foundation();
        let previewing = self.themes.read(cx).choice().map(pretty);
        let theme = match previewing {
            Some(name) => format!(
                "Theme: {name} (preview; system is {})",
                themes::system_name(cx)
            ),
            None => format!("Theme: {} (following the system)", themes::system_name(cx)),
        };
        h_flex()
            .flex_shrink_0()
            .flex_wrap()
            .gap(Space::Xl)
            .px(Space::Lg)
            .py(Space::Xs)
            .border_t_1()
            .border_color(present::edge())
            .child(Text::new(theme).muted())
            .child(
                Text::new(format!(
                    "Density: {} ({} px)",
                    density_title(config.density),
                    f.controls.metrics().height
                ))
                .muted(),
            )
            .child(
                Text::new(format!(
                    "Corners: {} ({} px)",
                    corner_title(config.corner_radius),
                    f.geometry.radius.default
                ))
                .muted(),
            )
            .child(
                Text::new(format!(
                    "Motion: {}",
                    if f.motion.reduced { "reduced" } else { "full" }
                ))
                .muted(),
            )
            .child(
                Text::new(format!(
                    "Generation {}",
                    odk::ui::theme_snapshot().generation
                ))
                .muted(),
            )
    }
}

fn density_title(d: Density) -> &'static str {
    match d {
        Density::Compact => "Compact",
        Density::Default => "Default",
        Density::Comfortable => "Comfortable",
    }
}

fn corner_title(r: CornerRadius) -> &'static str {
    match r {
        CornerRadius::Square => "Square",
        CornerRadius::Subtle => "Subtle",
    }
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let narrow = window.viewport_size().width < px(NARROW_BELOW);
        root()
            .child(self.toolbar(narrow, cx))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    .child(self.sidebar(narrow, cx))
                    .child(
                        div().flex_1().min_w_0().h_full().child(
                            self.view(self.page)
                                .cached(StyleRefinement::default().size_full()),
                        ),
                    ),
            )
            .child(self.status_bar(cx))
    }
}

/// The density, corners and motion `GALLERY_DENSITY` (`compact`, `default`, `comfortable`),
/// `GALLERY_CORNERS` (`square`, `subtle`) and `GALLERY_MOTION` (`reduced`, `full`) ask for. A
/// value that is not one of those is ignored, like a variable that is not set.
fn appearance_from_environment(cx: &mut App) {
    let var = |name| std::env::var(name).ok().map(|v| v.to_lowercase());
    let density =
        var("GALLERY_DENSITY").and_then(|v| Density::ALL.into_iter().find(|d| d.name() == v));
    let corners = var("GALLERY_CORNERS").and_then(|v| match v.as_str() {
        "square" => Some(CornerRadius::Square),
        "subtle" => Some(CornerRadius::Subtle),
        _ => None,
    });
    let reduced = var("GALLERY_MOTION").and_then(|v| match v.as_str() {
        "reduced" => Some(true),
        "full" => Some(false),
        _ => None,
    });
    odk::ui::configure(cx, |c| {
        c.density = density.unwrap_or(c.density);
        c.corner_radius = corners.unwrap_or(c.corner_radius);
        c.reduced_motion = reduced.unwrap_or(c.reduced_motion);
    });
}

/// Opens the gallery's window, as the program does. `GALLERY_PAGE`, one of the pages' ids, picks
/// the page it opens on, and `GALLERY_THEME`, an installed theme's name or a theme directory,
/// the theme it previews, for screenshots and measurements. `GALLERY_DENSITY`,
/// `GALLERY_CORNERS` and `GALLERY_MOTION` set the toolbar's other three settings, so that a
/// screenshot can be taken in any of them.
pub fn open(cx: &mut App) {
    let first = std::env::var("GALLERY_PAGE")
        .ok()
        .and_then(|id| Page::from_id(&id));
    let theme = std::env::var("GALLERY_THEME").ok();
    appearance_from_environment(cx);
    let window = WindowConfig::new("com.example.gallery")
        .title("ODK Gallery")
        .size(1200.0, 800.0)
        .min_size(640.0, 480.0);
    odk::ui::open_window(cx, window, |_, cx| {
        cx.new(|cx| {
            let mut gallery = Gallery::new(cx);
            if let Some(page) = first {
                gallery.show(page, cx);
            }
            if let Some(theme) = &theme {
                gallery.preview_theme(theme, cx);
            }
            gallery
        })
    })
    .unwrap();
}
