//! An Electron/VS Code–style IDE shell showing off rust-ui:
//! frameless window with custom title bar and menus, activity bar,
//! resizable + collapsible split panes in every direction, tabs, tree view,
//! a settings inspector full of widgets, live theme switching and more.
//!
//! Run:        cargo run --release --example showcase
//! Screenshot: cargo run --release --example showcase -- --screenshot out.png

use rust_ui::prelude::*;

#[derive(Clone, Copy, PartialEq)]
enum Activity {
    Explorer,
    Search,
    Git,
    Extensions,
}

struct Nebula {
    dark: bool,
    accent: usize,
    activity: Activity,
    sidebar_open: bool,
    panel_open: bool,
    inspector_open: bool,
    open_menu: Option<usize>,
    tabs: Vec<&'static str>,
    active_tab: usize,
    selected_file: &'static str,
    expanded: Vec<&'static str>,
    search: String,
    name: String,
    font_size: f32,
    autosave: bool,
    minimap: bool,
    telemetry: bool,
    density: usize,
    gray: usize,
    radius: f32,
    panel_tab: usize,
    context_menu: Option<Point>,
    show_modal: bool,
    notifications: u32,
}

#[derive(Clone)]
enum Msg {
    Activity(Activity),
    ToggleSidebar,
    TogglePanel,
    ToggleInspector,
    SidebarCollapsed(bool),
    PanelCollapsed(bool),
    InspectorCollapsed(bool),
    Menu(Option<usize>),
    SelectTab(usize),
    CloseTab(usize),
    Open(&'static str),
    ToggleFolder(&'static str),
    Search(String),
    Name(String),
    FontSize(f32),
    Autosave,
    Minimap,
    Telemetry,
    Density(usize),
    Gray(usize),
    Radius(f32),
    Accent(usize),
    ToggleTheme,
    PanelTab(usize),
    ContextMenu(Point),
    CloseContext,
    ShowModal(bool),
    ClearNotifications,
    Noop,
}

const GRAYS: [(&str, GrayTint); 4] =
    [("Zinc", GrayTint::Zinc), ("Slate", GrayTint::Slate), ("Mauve", GrayTint::Mauve), ("Sand", GrayTint::Sand)];

const ACCENTS: [&str; 6] = ["#5b8cff", "#a371f7", "#3ecf8e", "#f5a524", "#f0616d", "#22d3ee"];

impl Nebula {
    fn new() -> Self {
        Self {
            dark: true,
            accent: 0,
            activity: Activity::Explorer,
            sidebar_open: true,
            panel_open: true,
            inspector_open: true,
            open_menu: None,
            tabs: vec!["main.rs", "app.rs", "theme.rs"],
            active_tab: 0,
            selected_file: "main.rs",
            expanded: vec!["src", "widgets"],
            search: String::new(),
            name: "Nebula Studio".into(),
            font_size: 13.0,
            autosave: true,
            minimap: false,
            telemetry: true,
            density: 1,
            gray: 0,
            radius: 6.0,
            panel_tab: 0,
            context_menu: None,
            show_modal: false,
            notifications: 3,
        }
    }
}

impl App for Nebula {
    type Msg = Msg;

    fn theme(&self) -> Theme {
        let base = if self.dark { ThemeConfig::dark() } else { ThemeConfig::light() };
        Theme::from_config(ThemeConfig {
            accent: hex(ACCENTS[self.accent]),
            gray: GRAYS[self.gray].1,
            radius: self.radius,
            density: [Density::Compact, Density::Default, Density::Comfortable][self.density],
            ..base
        })
    }

    fn on_key(&self, e: &KeyEvent) -> Option<Msg> {
        match (&e.key, e.mods.command()) {
            (Key::Char('b'), true) => Some(Msg::ToggleSidebar),
            (Key::Char('j'), true) => Some(Msg::TogglePanel),
            (Key::Char('i'), true) => Some(Msg::ToggleInspector),
            (Key::Escape, _) => Some(Msg::Menu(None)),
            _ => None,
        }
    }

    fn update(&mut self, msg: Msg, cx: &mut Cx) {
        match msg {
            Msg::Activity(a) => {
                if self.activity == a && self.sidebar_open {
                    self.sidebar_open = false;
                } else {
                    self.activity = a;
                    self.sidebar_open = true;
                }
            }
            Msg::ToggleSidebar => self.sidebar_open = !self.sidebar_open,
            Msg::TogglePanel => self.panel_open = !self.panel_open,
            Msg::ToggleInspector => self.inspector_open = !self.inspector_open,
            Msg::SidebarCollapsed(c) => self.sidebar_open = !c,
            Msg::PanelCollapsed(c) => self.panel_open = !c,
            Msg::InspectorCollapsed(c) => self.inspector_open = !c,
            Msg::Menu(m) => self.open_menu = m,
            Msg::SelectTab(i) => {
                self.active_tab = i;
                self.selected_file = self.tabs[i];
            }
            Msg::CloseTab(i) => {
                self.tabs.remove(i);
                if self.active_tab >= self.tabs.len() {
                    self.active_tab = self.tabs.len().saturating_sub(1);
                }
            }
            Msg::Open(f) => {
                self.selected_file = f;
                if let Some(i) = self.tabs.iter().position(|t| *t == f) {
                    self.active_tab = i;
                } else {
                    self.tabs.push(f);
                    self.active_tab = self.tabs.len() - 1;
                }
            }
            Msg::ToggleFolder(f) => {
                if let Some(i) = self.expanded.iter().position(|x| *x == f) {
                    self.expanded.remove(i);
                } else {
                    self.expanded.push(f);
                }
            }
            Msg::Search(s) => self.search = s,
            Msg::Name(s) => {
                self.name = s;
                cx.set_title(self.name.clone());
            }
            Msg::FontSize(v) => self.font_size = v,
            Msg::Autosave => self.autosave = !self.autosave,
            Msg::Minimap => self.minimap = !self.minimap,
            Msg::Telemetry => self.telemetry = !self.telemetry,
            Msg::Density(d) => self.density = d,
            Msg::Gray(g) => self.gray = g,
            Msg::Radius(r) => self.radius = r,
            Msg::Accent(a) => self.accent = a,
            Msg::ToggleTheme => {
                self.dark = !self.dark;
                self.open_menu = None;
            }
            Msg::PanelTab(t) => self.panel_tab = t,
            Msg::ContextMenu(p) => self.context_menu = Some(p),
            Msg::CloseContext => self.context_menu = None,
            Msg::ShowModal(s) => {
                self.show_modal = s;
                self.open_menu = None;
                self.context_menu = None;
            }
            Msg::ClearNotifications => self.notifications = 0,
            Msg::Noop => {
                self.open_menu = None;
                self.context_menu = None;
            }
        }
    }

    fn view(&self) -> Element<Msg> {
        let th = theme();
        let c = &th.colors;
        let main = hsplit(
            "main",
            vec![
                Pane::fixed(260.0, self.sidebar())
                    .min(170.0)
                    .max(520.0)
                    .collapsible(true)
                    .collapsed(!self.sidebar_open),
                Pane::fill(
                    vsplit(
                        "center",
                        vec![
                            Pane::fill(self.editor()).min(120.0),
                            Pane::fixed(210.0, self.bottom_panel())
                                .min(90.0)
                                .max(600.0)
                                .collapsible(true)
                                .collapsed(!self.panel_open),
                        ],
                    )
                    .on_collapse(|_, c| Msg::PanelCollapsed(c)),
                ),
                Pane::fixed(300.0, self.inspector())
                    .min(240.0)
                    .max(560.0)
                    .collapsible(true)
                    .collapsed(!self.inspector_open),
            ],
        )
        .on_collapse(|i, c| if i == 0 { Msg::SidebarCollapsed(c) } else { Msg::InspectorCollapsed(c) });

        let mut root = col()
            .size_full()
            .bg(c.surface)
            .child(self.titlebar())
            .child(row().grow(1.0).min_h(0.0).child(self.activity_bar()).child(main))
            .child(self.status_bar());
        if let Some(p) = self.context_menu {
            root = root.child(context_menu(
                p,
                vec![
                    MenuItem::action("New File", Msg::Noop).icon(Icon::File).shortcut("Ctrl+N"),
                    MenuItem::action("New Folder", Msg::Noop).icon(Icon::Folder),
                    MenuItem::Separator,
                    MenuItem::action("Rename…", Msg::Noop).shortcut("F2"),
                    MenuItem::action("Delete", Msg::ShowModal(true)).shortcut("Del"),
                    MenuItem::Separator,
                    MenuItem::action("Copy Path", Msg::Noop).disabled(true),
                ],
                Msg::CloseContext,
            ));
        }
        if self.show_modal {
            root = root.child(modal(
                "Delete file?",
                col()
                    .gap(8.0)
                    .child(text(format!(
                        "\"{}\" will be moved to the trash. You can restore it later.",
                        self.selected_file
                    )))
                    .child(checkbox("Don't ask again", false).on_click(Msg::Noop)),
                vec![
                    button("Cancel").on_click(Msg::ShowModal(false)),
                    danger_button("Move to Trash").on_click(Msg::ShowModal(false)),
                ],
                Msg::ShowModal(false),
            ));
        }
        root
    }
}

impl Nebula {
    fn titlebar(&self) -> Element<Msg> {
        let th = theme();
        let c = &th.colors;
        let logo = div()
            .square(20.0)
            .rounded(6.0)
            .ml(12.0)
            .mr(4.0)
            .gradient(135.0, [(0.0, c.accent.lighten(0.2)), (1.0, c.accent.darken(0.25))])
            .center()
            .child(icon(Icon::Layers).font_size(12.0).color(Color::WHITE).bold());
        let menus = vec![
            Menu::new(
                "File",
                vec![
                    MenuItem::action("New File", Msg::Noop).icon(Icon::File).shortcut("Ctrl+N"),
                    MenuItem::action("Open Folder…", Msg::Noop).icon(Icon::Folder).shortcut("Ctrl+O"),
                    MenuItem::Separator,
                    MenuItem::action("Save", Msg::Noop).shortcut("Ctrl+S"),
                    MenuItem::action("Save As…", Msg::Noop).shortcut("Ctrl+Shift+S"),
                    MenuItem::Separator,
                    MenuItem::action("Exit", Msg::Noop),
                ],
            ),
            Menu::new(
                "Edit",
                vec![
                    MenuItem::action("Undo", Msg::Noop).shortcut("Ctrl+Z"),
                    MenuItem::action("Redo", Msg::Noop).shortcut("Ctrl+Y"),
                    MenuItem::Separator,
                    MenuItem::action("Find", Msg::Noop).icon(Icon::Search).shortcut("Ctrl+F"),
                ],
            ),
            Menu::new(
                "View",
                vec![
                    MenuItem::Header("Layout".into()),
                    MenuItem::check("Primary Side Bar", self.sidebar_open, Msg::ToggleSidebar),
                    MenuItem::check("Panel", self.panel_open, Msg::TogglePanel),
                    MenuItem::check("Inspector", self.inspector_open, Msg::ToggleInspector),
                    MenuItem::Separator,
                    MenuItem::action(if self.dark { "Light Theme" } else { "Dark Theme" }, Msg::ToggleTheme)
                        .icon(if self.dark { Icon::Sun } else { Icon::Moon }),
                ],
            ),
            Menu::new("Help", vec![MenuItem::action("About Nebula", Msg::ShowModal(false)).icon(Icon::Info)]),
        ];
        let left = row().h_full().items_center().child(logo).child(menu_bar(menus, self.open_menu, Msg::Menu));
        let layout_btn = |i: Icon, on: bool, m: Msg, tip: &str| {
            icon_button(i).on_click(m).tooltip(tip).when(on, |b| b.color(theme().colors.text))
        };
        let right = row()
            .h_full()
            .items_center()
            .gap(2.0)
            .pr(6.0)
            .child(layout_btn(
                Icon::SidebarLeft,
                self.sidebar_open,
                Msg::ToggleSidebar,
                "Toggle Primary Side Bar (Ctrl+B)",
            ))
            .child(layout_btn(Icon::PanelBottom, self.panel_open, Msg::TogglePanel, "Toggle Panel (Ctrl+J)"))
            .child(layout_btn(
                Icon::SidebarRight,
                self.inspector_open,
                Msg::ToggleInspector,
                "Toggle Inspector (Ctrl+I)",
            ));
        titlebar(format!("{} — {}", self.selected_file, self.name), left, right, window_info().maximized)
    }

    fn activity_bar(&self) -> Element<Msg> {
        let th = theme();
        let c = &th.colors;
        let item = |a: Activity, i: Icon, tip: &str| {
            let active = self.activity == a && self.sidebar_open;
            div()
                .center()
                .w(48.0)
                .h(46.0)
                .color(if active { c.text } else { c.text_faint })
                .transition(0.1)
                .hover(|s| s.color(c.text))
                .on_click(Msg::Activity(a))
                .tooltip(tip)
                .cursor(Cursor::Default)
                .child(icon(i).font_size(22.0))
                .when(active, |d| {
                    d.child(div().absolute().left(0.0).top(10.0).bottom(10.0).w(2.0).rounded(1.0).bg(c.accent))
                })
        };
        let bell = div()
            .center()
            .w(48.0)
            .h(46.0)
            .color(c.text_faint)
            .hover(|s| s.color(c.text))
            .on_click(Msg::ClearNotifications)
            .tooltip("Notifications")
            .child(icon(Icon::Bell).font_size(21.0))
            .when(self.notifications > 0, |d| {
                d.child(
                    badge(self.notifications.to_string())
                        .absolute()
                        .top(7.0)
                        .right(7.0)
                        .h(16.0)
                        .px(5.0)
                        .font_size(10.0),
                )
            });
        col()
            .w(48.0)
            .shrink(0.0)
            .bg(c.chrome)
            .border_r(1.0, c.border)
            .child(item(Activity::Explorer, Icon::Files, "Explorer"))
            .child(item(Activity::Search, Icon::Search, "Search"))
            .child(item(Activity::Git, Icon::GitBranch, "Source Control"))
            .child(item(Activity::Extensions, Icon::Blocks, "Extensions"))
            .child(spacer())
            .child(bell)
            .child(
                div()
                    .center()
                    .w(48.0)
                    .h(46.0)
                    .child(avatar("DC", hex(ACCENTS[(self.accent + 1) % ACCENTS.len()])))
                    .tooltip("Account"),
            )
            .child(
                div()
                    .center()
                    .w(48.0)
                    .h(46.0)
                    .color(c.text_faint)
                    .hover(|s| s.color(c.text))
                    .on_click(Msg::ToggleInspector)
                    .tooltip("Settings")
                    .child(icon(Icon::Settings).font_size(21.0)),
            )
    }

    fn sidebar(&self) -> Element<Msg> {
        let th = theme();
        let c = &th.colors;
        let title = match self.activity {
            Activity::Explorer => "Explorer",
            Activity::Search => "Search",
            Activity::Git => "Source Control",
            Activity::Extensions => "Extensions",
        };
        let header =
            row().items_center().justify_between().h(36.0).pr(6.0).shrink(0.0).child(section_header(title)).child(
                row()
                    .gap(0.0)
                    .child(icon_button(Icon::Plus).tooltip("New File"))
                    .child(icon_button(Icon::Refresh).tooltip("Refresh"))
                    .child(icon_button(Icon::More)),
            );
        let body = match self.activity {
            Activity::Explorer => self.file_tree(),
            Activity::Search => {
                col().gap(10.0).p(12.0).child(search_input(self.search.clone(), Msg::Search).id("search")).child(
                    text(if self.search.is_empty() {
                        "Type to search across files.".to_string()
                    } else {
                        format!("No results for \u{201c}{}\u{201d}", self.search)
                    })
                    .color(c.text_faint),
                )
            }
            Activity::Git => col()
                .gap(10.0)
                .p(12.0)
                .child(text_input("", |_| Msg::Noop).placeholder("Message (Ctrl+Enter to commit)"))
                .child(primary_button("Commit").with_icon(Icon::Check).w_full())
                .child(section_header("Changes").px(0.0))
                .children(["main.rs", "theme.rs", "Cargo.toml"].into_iter().map(|f| {
                    row()
                        .items_center()
                        .gap(8.0)
                        .child(icon(Icon::File).font_size(14.0).color(c.text_faint))
                        .child(text(f).grow(1.0))
                        .child(tag("M", c.warning))
                })),
            Activity::Extensions => col().gap(10.0).p(12.0).children(
                [
                    ("Rust Analyzer", "Rust language support", "#f74c00"),
                    ("Material Icons", "File icon theme", "#42a5f5"),
                    ("GitLens", "Supercharged Git", "#2bb673"),
                ]
                .into_iter()
                .map(|(n, d, col_)| {
                    row().gap(10.0).p(10.0).rounded(th.radius).bg(c.hover).child(avatar(&n[..1], hex(col_))).child(
                        col()
                            .grow(1.0)
                            .min_w(0.0)
                            .child(text(n).semibold().ellipsis())
                            .child(text(d).font_size(12.0).color(c.text_faint).ellipsis()),
                    )
                }),
            ),
        };
        col()
            .bg(c.panel)
            .border_r(1.0, c.border)
            .on_context_menu(Msg::ContextMenu)
            .child(header)
            .child(body.grow(1.0).scroll_y())
    }

    fn file_tree(&self) -> Element<Msg> {
        let tree: &[(usize, &'static str, bool, Option<&'static str>)] = &[
            (0, "nebula", true, None),
            (1, "assets", true, Some("nebula")),
            (1, "src", true, Some("nebula")),
            (2, "main.rs", false, Some("src")),
            (2, "app.rs", false, Some("src")),
            (2, "theme.rs", false, Some("src")),
            (2, "widgets", true, Some("src")),
            (3, "button.rs", false, Some("widgets")),
            (3, "split.rs", false, Some("widgets")),
            (3, "tabs.rs", false, Some("widgets")),
            (3, "tree.rs", false, Some("widgets")),
            (2, "layout.rs", false, Some("src")),
            (2, "render.rs", false, Some("src")),
            (1, "tests", true, Some("nebula")),
            (1, "Cargo.toml", false, Some("nebula")),
            (1, "README.md", false, Some("nebula")),
        ];
        let visible = |parent: Option<&str>| -> bool {
            let mut p = parent;
            while let Some(name) = p {
                if name != "nebula" && !self.expanded.contains(&name) {
                    return false;
                }
                p = tree.iter().find(|t| t.1 == name).and_then(|t| t.3);
            }
            true
        };
        let mut list = col().py(4.0).gap(1.0);
        for &(depth, name, folder, parent) in tree {
            if !visible(parent) {
                continue;
            }
            let icon_ = if folder {
                Icon::Folder
            } else if name.ends_with(".rs") {
                Icon::Code
            } else {
                Icon::File
            };
            let expanded = if folder { Some(name == "nebula" || self.expanded.contains(&name)) } else { None };
            let r = tree_row(depth, expanded, Some(icon_), name, !folder && self.selected_file == name).key(name);
            list = list.child(if folder { r.on_click(Msg::ToggleFolder(name)) } else { r.on_click(Msg::Open(name)) });
        }
        list
    }

    fn editor(&self) -> Element<Msg> {
        let th = theme();
        let c = &th.colors;
        if self.tabs.is_empty() {
            return col()
                .center()
                .gap(12.0)
                .color(c.text_faint)
                .child(icon(Icon::Layers).font_size(64.0).weight(Weight(300)))
                .child(text("No open editors").font_size(15.0))
                .child(row().gap(6.0).items_center().child(text("Show all commands")).child(kbd("Ctrl+Shift+P")));
        }
        let tabs = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, t)| {
                Tab::new(*t, i == self.active_tab, Msg::SelectTab(i))
                    .icon(Icon::Code)
                    .closable(Msg::CloseTab(i))
                    .modified(i == 1)
            })
            .collect();
        let breadcrumb = row()
            .h(26.0)
            .shrink(0.0)
            .items_center()
            .gap(4.0)
            .px(14.0)
            .color(c.text_faint)
            .font_size(12.0)
            .child(text("nebula"))
            .child(icon(Icon::ChevronRight).font_size(12.0))
            .child(text("src"))
            .child(icon(Icon::ChevronRight).font_size(12.0))
            .child(text(self.tabs[self.active_tab]).color(c.text_muted));
        col().bg(c.surface).child(tab_bar(tabs)).child(breadcrumb).child(
            row()
                .grow(1.0)
                .min_h(0.0)
                .child(code_view(self.font_size, self.tabs[self.active_tab]))
                .child_if(self.minimap, minimap),
        )
    }

    fn bottom_panel(&self) -> Element<Msg> {
        let th = theme();
        let c = &th.colors;
        let tab = |i: usize, label: &str, count: Option<u32>| {
            let active = self.panel_tab == i;
            row()
                .items_center()
                .gap(6.0)
                .h_full()
                .px(2.0)
                .mr(14.0)
                .font_size(11.5)
                .medium()
                .letter_spacing(0.4)
                .color(if active { c.text } else { c.text_faint })
                .hover(|s| s.color(c.text))
                .on_click(Msg::PanelTab(i))
                .child(text(label.to_uppercase()).nowrap())
                .when(count.is_some(), |r| r.child(badge(count.unwrap().to_string()).h(16.0).px(5.0).font_size(10.0)))
                .when(active, |r| r.border_b(1.0, c.accent))
        };
        let lines: Vec<(&str, Color)> = match self.panel_tab {
            0 => vec![
                ("$ cargo build --release", c.text),
                ("   Compiling taffy v0.14.0", c.success),
                ("   Compiling cosmic-text v0.19.0", c.success),
                ("   Compiling rust-ui v0.1.0 (/home/dev/rust-ui)", c.success),
                ("    Finished `release` profile [optimized] target(s) in 24.31s", c.success),
                ("$ cargo run --example showcase", c.text),
                ("     Running `target/release/examples/showcase`", c.text_muted),
                ("$ ", c.text),
            ],
            1 => vec![
                ("warning: unused variable `frame` — src/render.rs:42:9", c.warning),
                ("warning: field `cache` is never read — src/layout.rs:17:5", c.warning),
                ("error[E0308]: mismatched types — src/app.rs:88:21", c.danger),
            ],
            _ => vec![
                ("[info] Language server started", c.text_muted),
                ("[info] Indexed 214 files in 380ms", c.text_muted),
            ],
        };
        col()
            .bg(c.panel)
            .border_t(1.0, c.border)
            .child(
                row()
                    .h(34.0)
                    .shrink(0.0)
                    .px(14.0)
                    .items(Align::Stretch)
                    .child(tab(0, "Terminal", None))
                    .child(tab(1, "Problems", Some(3)))
                    .child(tab(2, "Output", None))
                    .child(spacer())
                    .child(
                        row()
                            .items_center()
                            .child(icon_button(Icon::Plus).tooltip("New Terminal"))
                            .child(icon_button(Icon::Close).on_click(Msg::TogglePanel).tooltip("Close Panel")),
                    ),
            )
            .child(
                col()
                    .grow(1.0)
                    .scroll_y()
                    .px(14.0)
                    .py(6.0)
                    .mono()
                    .font_size(12.5)
                    .children(lines.into_iter().map(|(l, col_)| text(l).color(col_).nowrap())),
            )
    }

    fn inspector(&self) -> Element<Msg> {
        let th = theme();
        let c = &th.colors;
        let field = |label: &str, control: Element<Msg>| {
            col().gap(6.0).child(text(label).font_size(12.0).medium().color(c.text_muted)).child(control)
        };
        let swatches = row().gap(8.0).children(ACCENTS.iter().enumerate().map(|(i, h)| {
            let sel = self.accent == i;
            div()
                .square(22.0)
                .pill()
                .bg(hex(h))
                .transition(0.12)
                .hover(|s| s.translate(0.0, -2.0))
                .on_click(Msg::Accent(i))
                .when(sel, |d| {
                    d.outline(2.0, 2.0, hex(h))
                        .center()
                        .child(icon(Icon::Check).font_size(12.0).color(Color::WHITE).bold())
                })
        }));
        col()
            .bg(c.panel)
            .border_l(1.0, c.border)
            .child(
                row()
                    .items_center()
                    .justify_between()
                    .h(36.0)
                    .pr(6.0)
                    .shrink(0.0)
                    .child(section_header("Inspector"))
                    .child(icon_button(Icon::Close).on_click(Msg::ToggleInspector).tooltip("Close")),
            )
            .child(
                col()
                    .grow(1.0)
                    .scroll_y()
                    .p(14.0)
                    .gap(16.0)
                    .child(
                        card()
                            .gap(14.0)
                            .child(
                                row()
                                    .items_center()
                                    .gap(10.0)
                                    .child(icon(Icon::Sun).font_size(16.0).color(c.accent))
                                    .child(text("Appearance").semibold().font_size(14.0)),
                            )
                            .child(field(
                                "Theme",
                                segmented(vec![
                                    ("Dark".into(), self.dark, if self.dark { Msg::Noop } else { Msg::ToggleTheme }),
                                    ("Light".into(), !self.dark, if self.dark { Msg::ToggleTheme } else { Msg::Noop }),
                                ]),
                            ))
                            .child(field("Accent color", swatches))
                            .child(field(
                                "Density",
                                segmented(vec![
                                    ("Compact".into(), self.density == 0, Msg::Density(0)),
                                    ("Default".into(), self.density == 1, Msg::Density(1)),
                                    ("Roomy".into(), self.density == 2, Msg::Density(2)),
                                ]),
                            ))
                            .child(field(
                                "Gray tint",
                                segmented(
                                    GRAYS
                                        .iter()
                                        .enumerate()
                                        .map(|(i, g)| (g.0.to_string(), self.gray == i, Msg::Gray(i)))
                                        .collect(),
                                ),
                            ))
                            .child(field(
                                "Corner radius",
                                row()
                                    .items_center()
                                    .gap(12.0)
                                    .child(slider(self.radius, 0.0, 12.0).step(1.0).on_change(Msg::Radius).grow(1.0))
                                    .child(
                                        text(format!("{}px", self.radius as i32))
                                            .nowrap()
                                            .mono()
                                            .font_size(12.0)
                                            .w(40.0)
                                            .text_align(TextAlign::Right),
                                    ),
                            ))
                            .child(field(
                                "Editor font size",
                                row()
                                    .items_center()
                                    .gap(12.0)
                                    .child(
                                        slider(self.font_size, 10.0, 20.0).step(1.0).on_change(Msg::FontSize).grow(1.0),
                                    )
                                    .child(
                                        text(format!("{}px", self.font_size as i32))
                                            .nowrap()
                                            .mono()
                                            .font_size(12.0)
                                            .w(40.0)
                                            .text_align(TextAlign::Right),
                                    ),
                            )),
                    )
                    .child(
                        card()
                            .gap(14.0)
                            .child(
                                row()
                                    .items_center()
                                    .gap(10.0)
                                    .child(icon(Icon::Settings).font_size(16.0).color(c.accent))
                                    .child(text("Workspace").semibold().font_size(14.0)),
                            )
                            .child(field(
                                "Display name",
                                text_input(self.name.clone(), Msg::Name).placeholder("Workspace name"),
                            ))
                            .child(
                                row()
                                    .justify_between()
                                    .items_center()
                                    .child(text("Auto save"))
                                    .child(switch(self.autosave).on_click(Msg::Autosave)),
                            )
                            .child(
                                row()
                                    .justify_between()
                                    .items_center()
                                    .child(text("Show minimap"))
                                    .child(switch(self.minimap).on_click(Msg::Minimap)),
                            )
                            .child(checkbox("Send anonymous usage data", self.telemetry).on_click(Msg::Telemetry))
                            .child(field(
                                "Indexing",
                                col()
                                    .gap(6.0)
                                    .child(progress(0.68))
                                    .child(text("146 of 214 files").font_size(11.5).color(c.text_faint)),
                            )),
                    )
                    .child(
                        row()
                            .gap(8.0)
                            .flex_wrap()
                            .child(primary_button("Save changes").with_icon(Icon::Check))
                            .child(button("Reset"))
                            .child(ghost_button("Delete…").on_click(Msg::ShowModal(true)).color(c.danger)),
                    )
                    .child(
                        row()
                            .gap(6.0)
                            .flex_wrap()
                            .child(tag("rust", hex("#f74c00")))
                            .child(tag("ui", c.accent))
                            .child(tag("desktop", c.success))
                            .child(tag("electron-like", hex("#a371f7"))),
                    ),
            )
    }

    fn status_bar(&self) -> Element<Msg> {
        let th = theme();
        status_bar()
            .child(
                row()
                    .items_center()
                    .h_full()
                    .ml(-8.0)
                    .mr(4.0)
                    .px(10.0)
                    .gap(6.0)
                    .bg(th.colors.accent)
                    .color(th.colors.accent_text)
                    .child(icon(Icon::GitBranch).font_size(13.0))
                    .child(text("main").nowrap()),
            )
            .child(status_item(Some(Icon::Refresh), "Synced"))
            .child(status_item(Some(Icon::Warning), "2"))
            .child(status_item(Some(Icon::Info), "1"))
            .child(spacer())
            .child(status_item(None, "Ln 12, Col 34"))
            .child(status_item(None, "Spaces: 4"))
            .child(status_item(None, "UTF-8"))
            .child(status_item(Some(Icon::Code), "Rust"))
            .child(status_item(Some(Icon::Bell), "").on_click(Msg::ClearNotifications))
    }
}

fn minimap() -> Element<Msg> {
    let th = theme();
    canvas(move |cv, r| {
        let c = &th.colors;
        let mut y = r.y + 6.0;
        let mut i = 0u32;
        while y < r.bottom() - 4.0 {
            let w = ((i * 37 + 11) % 60) as f32 + 12.0;
            let indent = ((i * 13) % 4) as f32 * 6.0;
            cv.fill_rounded(
                Rect::new(r.x + 8.0 + indent, y, w.min(r.w - 16.0 - indent), 2.0),
                1.0,
                c.text_faint.with_alpha(0.5),
            );
            y += 4.0;
            i += 1;
        }
    })
    .w(70.0)
    .shrink(0.0)
    .border_l(1.0, theme().colors.border)
}

const CODE: &str = r#"use rust_ui::prelude::*;

/// Application state.
struct Nebula {
    sidebar_open: bool,
    font_size: f32,
}

#[derive(Clone)]
enum Msg { ToggleSidebar, FontSize(f32) }

impl App for Nebula {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _cx: &mut Cx) {
        match msg {
            Msg::ToggleSidebar => self.sidebar_open = !self.sidebar_open,
            Msg::FontSize(v) => self.font_size = v,
        }
    }

    fn view(&self) -> Element<Msg> {
        hsplit("main", vec![
            Pane::fixed(260.0, sidebar()).collapsed(!self.sidebar_open),
            Pane::fill(editor(self.font_size)),
        ])
    }
}

fn main() {
    let opts = WindowOptions::new("Nebula").frameless(true);
    rust_ui::run(Nebula { sidebar_open: true, font_size: 13.0 }, opts).unwrap();
}"#;

fn highlight(line: &str) -> Vec<(String, Color)> {
    let th = theme();
    let dark = th.dark;
    let kw = if dark { hex("#c792ea") } else { hex("#8e44ad") };
    let ty = if dark { hex("#ffcb6b") } else { hex("#b7791f") };
    let st = if dark { hex("#c3e88d") } else { hex("#2f855a") };
    let num = if dark { hex("#f78c6c") } else { hex("#c05621") };
    let cm = th.colors.text_faint;
    let fnc = if dark { hex("#82aaff") } else { hex("#2b6cb0") };
    let plain = th.colors.text;
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") || trimmed.starts_with("#[") {
        return vec![(line.to_string(), cm)];
    }
    let keywords = [
        "use", "struct", "enum", "impl", "fn", "let", "match", "for", "type", "self", "mut", "pub", "true", "false",
        "vec!",
    ];
    let mut out: Vec<(String, Color)> = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '"' {
            let mut j = i + 1;
            while j < chars.len() && chars[j] != '"' {
                j += 1;
            }
            let s: String = chars[i..(j + 1).min(chars.len())].iter().collect();
            out.push((s, st));
            i = j + 1;
        } else if ch.is_alphabetic() || ch == '_' {
            let mut j = i;
            while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_' || chars[j] == '!') {
                j += 1;
            }
            let w: String = chars[i..j].iter().collect();
            let next_paren = chars.get(j) == Some(&'(');
            let color = if keywords.contains(&w.as_str()) {
                kw
            } else if w.chars().next().unwrap().is_uppercase() {
                ty
            } else if next_paren {
                fnc
            } else {
                plain
            };
            out.push((w, color));
            i = j;
        } else if ch.is_ascii_digit() {
            let mut j = i;
            while j < chars.len() && (chars[j].is_ascii_digit() || chars[j] == '.') {
                j += 1;
            }
            out.push((chars[i..j].iter().collect(), num));
            i = j;
        } else {
            let mut j = i;
            while j < chars.len() && !(chars[j].is_alphanumeric() || chars[j] == '_' || chars[j] == '"') {
                j += 1;
            }
            out.push((chars[i..j].iter().collect(), th.colors.text_muted));
            i = j;
        }
    }
    out
}

fn code_view(font_size: f32, file: &str) -> Element<Msg> {
    let th = theme();
    let c = &th.colors;
    let lh = 1.6;
    let mut gutter = col().pt(8.0).pr(14.0).pl(18.0).shrink(0.0).color(c.text_faint).text_align(TextAlign::Right);
    let mut lines = col().pt(8.0).grow(1.0).pr(24.0);
    for (n, line) in CODE.lines().enumerate() {
        let current = n == 11;
        gutter = gutter.child(text(format!("{}", n + 1)).nowrap().when(current, |t| t.color(c.text)).h(font_size * lh));
        let mut r = row().h(font_size * lh).items_center();
        if current {
            r = r.bg(c.hover).mx(-6.0).px(6.0).rounded(3.0);
        }
        if line.is_empty() {
            r = r.child(text(" ").nowrap());
        }
        for (s, col_) in highlight(line) {
            r = r.child(text(s.replace(' ', "\u{a0}")).nowrap().color(col_));
        }
        lines = lines.child(r);
    }
    row()
        .key(file)
        .grow(1.0)
        .scroll_both()
        .mono()
        .font_size(font_size)
        .line_height(lh)
        .items(Align::Start)
        .child(gutter)
        .child(lines)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--bench") {
        for scale in [1.0f32, 2.0] {
            let mut h = Headless::new(Nebula::new(), 1440.0, 900.0, scale);
            h.settle();
            let n = 30;
            let t = std::time::Instant::now();
            for i in 0..n {
                // Alternate hover targets so every frame rebuilds, lays out and repaints.
                h.rt.handle(rust_ui::Event::PointerMove(Point::new(100.0 + (i % 2) as f32 * 40.0, 200.0)));
                h.rt.invalidate();
                h.rt.render();
            }
            println!("scale {scale}: {:.2} ms/frame", t.elapsed().as_secs_f64() * 1000.0 / n as f64);
        }
        return;
    }
    if let Some(pos) = args.iter().position(|a| a == "--screenshot") {
        let out = args.get(pos + 1).cloned().unwrap_or_else(|| "showcase.png".into());
        let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
        let scale: f32 = flag("--scale").and_then(|s| s.parse().ok()).unwrap_or(1.0);
        let mut app = Nebula::new();
        app.dark = !args.iter().any(|a| a == "--light");
        match flag("--state").as_deref() {
            Some("menu") => app.open_menu = Some(2),
            Some("modal") => app.show_modal = true,
            Some("context") => app.context_menu = Some(Point::new(120.0, 300.0)),
            Some("collapsed") => {
                app.sidebar_open = false;
                app.inspector_open = false;
            }
            Some("knobs") => {
                app.density = 0;
                app.gray = 2;
                app.radius = 12.0;
                app.accent = 1;
            }
            Some("search") => {
                app.activity = Activity::Search;
                app.search = "split".into();
            }
            _ => {}
        }
        let mut h = Headless::new(app, 1440.0, 900.0, scale);
        h.settle();
        h.move_to(500.0, 300.0);
        h.save_png(&out).expect("save");
        println!("saved {out}");
        return;
    }
    rust_ui::run(
        Nebula::new(),
        WindowOptions::new("Nebula Studio").size(1440.0, 900.0).min_size(720.0, 460.0).frameless(true),
    )
    .expect("failed to run");
}
