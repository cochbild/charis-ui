//! Widget gallery: every built-in widget on its own page, live, with the
//! theme knobs on the Theme page.
//!
//! ```sh
//! cargo run --release --example gallery
//! ```

use std::rc::Rc;

use charis_ui::image::{Fit, Image, Svg};
use charis_ui::prelude::*;
use charis_ui::tree::{TreeModel, TreeMsg, TreeState};
use charis_ui::ClipboardContent;

/// The gallery's pages, in sidebar order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Buttons,
    Inputs,
    Selection,
    Data,
    Feedback,
    Overlays,
    Media,
    Text,
    Theme,
}

impl Page {
    pub const ALL: [Page; 9] = [
        Page::Buttons,
        Page::Inputs,
        Page::Selection,
        Page::Data,
        Page::Feedback,
        Page::Overlays,
        Page::Media,
        Page::Text,
        Page::Theme,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Page::Buttons => "Buttons",
            Page::Inputs => "Text input",
            Page::Selection => "Selection",
            Page::Data => "Lists, tables, trees",
            Page::Feedback => "Status and feedback",
            Page::Overlays => "Menus and dialogs",
            Page::Media => "Images and icons",
            Page::Text => "Typography",
            Page::Theme => "Theme",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Page::Buttons => Icon::Play,
            Page::Inputs => Icon::Code,
            Page::Selection => Icon::Check,
            Page::Data => Icon::Columns,
            Page::Feedback => Icon::Bell,
            Page::Overlays => Icon::Layers,
            Page::Media => Icon::Star,
            Page::Text => Icon::File,
            Page::Theme => Icon::Settings,
        }
    }
}

/// A small file tree for the tree demo.
pub struct Files;

impl TreeModel for Files {
    type Id = String;
    fn children(&self, parent: Option<&String>) -> Vec<String> {
        match parent.map(String::as_str) {
            None => vec!["src".into(), "examples".into(), "Cargo.toml".into()],
            Some("src") => vec!["src/lib.rs".into(), "src/widgets.rs".into(), "src/runtime".into()],
            Some("src/runtime") => vec!["src/runtime/mod.rs".into(), "src/runtime/a11y.rs".into()],
            Some("examples") => vec!["examples/gallery.rs".into(), "examples/dock.rs".into()],
            Some(_) => Vec::new(),
        }
    }
    fn has_children(&self, id: &String) -> bool {
        matches!(id.as_str(), "src" | "src/runtime" | "examples")
    }
    fn label(&self, id: &String) -> String {
        id.rsplit('/').next().unwrap_or(id).to_string()
    }
    fn icon(&self, id: &String, _expanded: bool) -> Option<Icon> {
        Some(if self.has_children(id) { Icon::Folder } else { Icon::File })
    }
}

pub struct Gallery {
    pub page: Page,
    pub dark: bool,
    pub accent: usize,
    pub radius: f32,
    pub density: usize,
    pub high_contrast: bool,

    pub clicks: u32,
    pub name: String,
    pub notes: String,
    pub query: String,
    pub password: String,
    pub quantity: f64,

    pub checked: bool,
    pub switched: bool,
    pub radio: Option<usize>,
    pub picked: Option<usize>,
    pub combo: Option<usize>,
    pub segment: usize,
    pub tab: usize,
    pub volume: f32,

    pub selected_row: Option<usize>,
    pub sort: (usize, SortDir),
    pub files: Rc<Files>,
    pub tree: TreeState<String>,
    pub list_selected: Option<usize>,

    pub menu: Option<usize>,
    pub context: Option<Point>,
    pub dialog: bool,
    pub last_action: String,

    pub photo: Image,
    pub pasted: Option<Image>,
    pub logo: Option<Svg>,
    pub mark: Option<Svg>,
}

#[derive(Clone, Debug)]
pub enum Msg {
    Page(Page),
    Dark(bool),
    Accent(usize),
    Radius(f32),
    Density(usize),
    HighContrast(bool),

    Click,
    Name(String),
    Notes(String),
    Query(String),
    Password(String),
    Quantity(f64),

    Check,
    Switch,
    Radio(usize),
    Pick(usize),
    Combo(usize),
    Segment(usize),
    Tab(usize),
    Volume(f32),

    Row(usize),
    Sort(usize, SortDir),
    Tree(TreeMsg<String>),
    ListSelect(usize),

    Menu(Option<usize>),
    Context(Option<Point>),
    Dialog(bool),
    Action(&'static str),
    CopyPhoto,
    Pasted(Image),
    ReadClipboard,
    Clipboard(ClipboardContent),
}

const PEOPLE: [(&str, &str, u32, &str); 8] = [
    ("Ada Lovelace", "Analyst", 36, "London"),
    ("Alan Turing", "Mathematician", 41, "Manchester"),
    ("Grace Hopper", "Admiral", 85, "Arlington"),
    ("Edsger Dijkstra", "Professor", 72, "Nuenen"),
    ("Barbara Liskov", "Professor", 84, "Boston"),
    ("Ken Thompson", "Engineer", 81, "Palo Alto"),
    ("Margaret Hamilton", "Director", 88, "Cambridge"),
    ("Donald Knuth", "Professor", 86, "Stanford"),
];

const LOGO: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
  <defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1">
    <stop offset="0" stop-color="#5b8cff"/><stop offset="1" stop-color="#a371f7"/></linearGradient></defs>
  <rect x="4" y="4" width="56" height="56" rx="14" fill="url(#g)"/>
  <path d="M20 42 L32 18 L44 42 Z" fill="none" stroke="white" stroke-width="4" stroke-linejoin="round"/>
</svg>"##;

/// A one-color mark, for tinting.
const MARK: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
  <path d="M14 50 L32 12 L50 50 Z" fill="none" stroke="black" stroke-width="6" stroke-linejoin="round"/>
</svg>"##;

/// A generated "photo": a soft gradient with circles, so the gallery needs no image files.
fn make_photo() -> Image {
    let (w, h) = (320u32, 200u32);
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
            let d = ((fx - 0.7).powi(2) + (fy - 0.35).powi(2)).sqrt();
            let sun = (1.0 - d * 4.0).clamp(0.0, 1.0);
            let r = (40.0 + 180.0 * fy + 200.0 * sun).min(255.0);
            let g = (60.0 + 90.0 * fy + 170.0 * sun).min(255.0);
            let b = (140.0 - 60.0 * fy + 60.0 * sun).min(255.0);
            let hill = fy > 0.72 + 0.08 * (fx * 9.0).sin();
            let (r, g, b) = if hill { (30.0, 70.0 + 30.0 * fy, 60.0) } else { (r, g, b) };
            px.extend_from_slice(&[r as u8, g as u8, b as u8, 255]);
        }
    }
    Image::from_rgba(w, h, &px).expect("the buffer matches the size")
}

impl Default for Gallery {
    fn default() -> Self {
        let mut tree = TreeState::new("gallery-tree");
        tree.expand("src".to_string());
        Gallery {
            page: Page::Buttons,
            dark: true,
            accent: 0,
            radius: 6.0,
            density: 1,
            high_contrast: false,
            clicks: 0,
            name: "Ada".into(),
            notes: "Multi-line text.\nUndo with Ctrl+Z.".into(),
            query: String::new(),
            password: "hunter2".into(),
            quantity: 3.0,
            checked: true,
            switched: false,
            radio: Some(1),
            picked: Some(0),
            combo: None,
            segment: 0,
            tab: 0,
            volume: 40.0,
            selected_row: None,
            sort: (0, SortDir::Asc),
            files: Rc::new(Files),
            tree,
            list_selected: Some(2),
            menu: None,
            context: None,
            dialog: false,
            last_action: "None yet".into(),
            photo: make_photo(),
            pasted: None,
            logo: Svg::parse(LOGO),
            mark: Svg::parse(MARK),
        }
    }
}

impl App for Gallery {
    type Msg = Msg;

    fn theme(&self) -> Theme {
        let base = if self.dark { ThemeConfig::dark() } else { ThemeConfig::light() };
        Theme::from_config(ThemeConfig {
            accent: Accent::ALL[self.accent].color(self.dark),
            radius: self.radius,
            density: [Density::Compact, Density::Default, Density::Comfortable][self.density],
            contrast: if self.high_contrast { Contrast::High } else { Contrast::Normal },
            ..base
        })
    }

    fn update(&mut self, msg: Msg, cx: &mut Cx<Msg>) {
        match msg {
            Msg::Page(p) => self.page = p,
            Msg::Dark(d) => self.dark = d,
            Msg::Accent(i) => self.accent = i,
            Msg::Radius(r) => self.radius = r.round(),
            Msg::Density(d) => self.density = d,
            Msg::HighContrast(h) => self.high_contrast = h,
            Msg::Click => self.clicks += 1,
            Msg::Name(s) => self.name = s,
            Msg::Notes(s) => self.notes = s,
            Msg::Query(s) => self.query = s,
            Msg::Password(s) => self.password = s,
            Msg::Quantity(q) => self.quantity = q,
            Msg::Check => self.checked = !self.checked,
            Msg::Switch => self.switched = !self.switched,
            Msg::Radio(i) => self.radio = Some(i),
            Msg::Pick(i) => self.picked = Some(i),
            Msg::Combo(i) => self.combo = Some(i),
            Msg::Segment(i) => self.segment = i,
            Msg::Tab(i) => self.tab = i,
            Msg::Volume(v) => self.volume = v,
            Msg::Row(i) => self.selected_row = Some(i),
            Msg::Sort(c, d) => self.sort = (c, d),
            Msg::Tree(m) => {
                let files = self.files.clone();
                if let Some(e) = self.tree.update(m, &*files, cx) {
                    self.last_action = format!("{e:?}");
                }
            }
            Msg::ListSelect(i) => self.list_selected = Some(i),
            Msg::Menu(m) => self.menu = m,
            Msg::Context(p) => self.context = p,
            Msg::Dialog(open) => self.dialog = open,
            Msg::CopyPhoto => {
                cx.copy_image(self.photo.clone());
                self.last_action = "Copied the image".into();
            }
            Msg::Pasted(img) => {
                self.last_action = format!("Pasted a {}×{} image", img.width(), img.height());
                self.pasted = Some(img);
            }
            Msg::ReadClipboard => cx.read_clipboard(Msg::Clipboard),
            Msg::Clipboard(c) => {
                self.last_action = match c {
                    ClipboardContent::Text(t) => {
                        format!("Clipboard text: {:?}", t.chars().take(40).collect::<String>())
                    }
                    ClipboardContent::Image(img) => {
                        let s = format!("Clipboard image: {}×{}", img.width(), img.height());
                        self.pasted = Some(img);
                        s
                    }
                    _ => "The clipboard is empty".into(),
                };
            }
            Msg::Action(a) => {
                self.last_action = a.to_string();
                self.menu = None;
                self.context = None;
                self.dialog = false;
            }
        }
    }

    fn view(&self) -> Element<Msg> {
        let th = theme();
        let c = &th.colors;
        let sidebar = col()
            .w(230.0)
            .h_full()
            .shrink(0.0)
            .bg(c.panel)
            .border_r(1.0, c.border)
            .child(
                row()
                    .h(52.0)
                    .px(16.0)
                    .gap(10.0)
                    .items_center()
                    .child(icon(Icon::Blocks).color(c.accent))
                    .child(text("Widget gallery").semibold()),
            )
            .child(col().px(8.0).gap(2.0).children(Page::ALL.iter().map(|&p| {
                list_item(Some(p.icon()), p.title(), p == self.page)
                    .id(&format!("page-{p:?}").to_lowercase())
                    .on_click(Msg::Page(p))
            })))
            .child(spacer())
            .child(
                row()
                    .p(12.0)
                    .gap(8.0)
                    .items_center()
                    .child(icon(if self.dark { Icon::Moon } else { Icon::Sun }).color(c.text_muted))
                    .child(text("Dark mode").color(c.text_muted))
                    .child(spacer())
                    .child(switch(self.dark).id("dark-switch").on_click(Msg::Dark(!self.dark))),
            );

        let page = col().grow(1.0).h_full().scroll_y().id("page").child(
            col()
                .max_w(880.0)
                .w_full()
                .px(32.0)
                .py(28.0)
                .gap(20.0)
                .child(text(self.page.title()).font_size(24.0).bold())
                .children(self.page_sections()),
        );

        let mut root = row().size_full().bg(c.background).child(sidebar).child(page);
        if let Some(at) = self.context {
            root = root.child(context_menu(
                at,
                vec![
                    MenuItem::action("Cut", Msg::Action("Cut")).shortcut("Mod+X"),
                    MenuItem::action("Copy", Msg::Action("Copy")).shortcut("Mod+C"),
                    MenuItem::Separator,
                    MenuItem::submenu("More", vec![MenuItem::action("Rename…", Msg::Action("Rename"))]),
                ],
                Msg::Context(None),
            ));
        }
        if self.dialog {
            root = root.child(modal(
                "Discard changes?",
                text("Your edits to settings.toml will be lost.").color(c.text_muted),
                vec![
                    button("Cancel").on_click(Msg::Dialog(false)),
                    danger_button("Discard").id("discard").on_click(Msg::Action("Discard")),
                ],
                Msg::Dialog(false),
            ));
        }
        root
    }
}

/// A titled demo card.
fn section(title: &str, note: &str, body: Element<Msg>) -> Element<Msg> {
    let th = theme();
    col()
        .gap(10.0)
        .child(
            col()
                .gap(2.0)
                .child(text(title).semibold().font_size(15.0))
                .child(text(note).color(th.colors.text_muted).font_size(th.font_size_sm)),
        )
        .child(card().p(20.0).gap(12.0).child(body))
}

fn labeled(label: &str, e: impl Into<Element<Msg>>) -> Element<Msg> {
    row()
        .gap(16.0)
        .items_center()
        .child(text(label).w(120.0).shrink(0.0).color(theme().colors.text_muted))
        .child(e.into().grow(1.0))
}

impl Gallery {
    fn page_sections(&self) -> Vec<Element<Msg>> {
        match self.page {
            Page::Buttons => self.buttons(),
            Page::Inputs => self.inputs(),
            Page::Selection => self.selection(),
            Page::Data => self.data(),
            Page::Feedback => self.feedback(),
            Page::Overlays => self.overlays(),
            Page::Media => self.media(),
            Page::Text => self.typography(),
            Page::Theme => self.theme_page(),
        }
    }

    fn buttons(&self) -> Vec<Element<Msg>> {
        vec![
            section(
                "Variants",
                "button, primary_button, ghost_button, danger_button. Tab to focus, Enter or Space to press.",
                row()
                    .gap(8.0)
                    .flex_wrap()
                    .child(button("Secondary").on_click(Msg::Click))
                    .child(primary_button("Primary").id("primary").on_click(Msg::Click))
                    .child(ghost_button("Ghost").on_click(Msg::Click))
                    .child(danger_button("Danger").on_click(Msg::Click))
                    .child(primary_button("Disabled").disabled(true)),
            ),
            section(
                "Icon buttons",
                "icon_button with a tooltip names the button for screen readers too.",
                row().gap(4.0).children([Icon::Play, Icon::Refresh, Icon::Search, Icon::Settings, Icon::More].map(
                    |i| {
                        let tip = format!("{i:?}");
                        icon_button(i).tooltip(tip).on_click(Msg::Click)
                    },
                )),
            ),
            section(
                "Counter",
                "Every press sends a message; update changes the state and the view follows.",
                row()
                    .gap(12.0)
                    .items_center()
                    .child(primary_button("Press me").id("counter").on_click(Msg::Click))
                    .child(text(format!("Pressed {} times", self.clicks)).id("clicks")),
            ),
        ]
    }

    fn inputs(&self) -> Vec<Element<Msg>> {
        vec![
            section(
                "Text fields",
                "Selection, word navigation, clipboard, undo and redo, and IME composition.",
                col()
                    .gap(10.0)
                    .child(labeled("Name", text_input(self.name.clone(), Msg::Name).id("name")))
                    .child(labeled("Search", search_input(self.query.clone(), Msg::Query)))
                    .child(labeled("Password", text_input(self.password.clone(), Msg::Password).password())),
            ),
            section(
                "Text area",
                "Multi-line editing with word wrap and scrolling.",
                text_area(self.notes.clone(), Msg::Notes).h(120.0),
            ),
            section(
                "Number input",
                "Range, step and decimals; arrow keys step, and typing is validated on Enter or blur.",
                labeled(
                    "Quantity",
                    Element::from(number_input("qty", self.quantity, Msg::Quantity).range(0.0, 99.0).step(1.0))
                        .w(160.0),
                ),
            ),
        ]
    }

    fn selection(&self) -> Vec<Element<Msg>> {
        vec![
            section(
                "Toggles",
                "checkbox and switch_row; both are keyboard reachable and expose their state.",
                col()
                    .gap(10.0)
                    .child(checkbox("Remember me", self.checked).id("check").on_click(Msg::Check))
                    .child(switch_row("Wi-Fi", self.switched).on_click(Msg::Switch)),
            ),
            section(
                "Radio group",
                "Arrow keys move the selection, as in WAI-ARIA.",
                radio_group(["Small", "Medium", "Large"], self.radio, Msg::Radio),
            ),
            section(
                "Dropdowns",
                "pick_list chooses from a list; combo_box filters it as you type.",
                col()
                    .gap(10.0)
                    .child(labeled(
                        "Language",
                        pick_list(["Rust", "Zig", "Go", "Swift"], self.picked, Msg::Pick).w(220.0),
                    ))
                    .child(labeled(
                        "Font",
                        combo_box(["Inter", "Geist", "JetBrains Mono", "SF Pro", "Segoe UI"], self.combo, Msg::Combo)
                            .w(220.0),
                    )),
            ),
            section(
                "Segmented control and tabs",
                "segmented and tab_bar.",
                col()
                    .gap(12.0)
                    .child(segmented(
                        ["Day", "Week", "Month"]
                            .iter()
                            .enumerate()
                            .map(|(i, s)| (s.to_string(), i == self.segment, Msg::Segment(i)))
                            .collect(),
                    ))
                    .child(tab_bar(
                        ["Overview", "Activity", "Settings"]
                            .iter()
                            .enumerate()
                            .map(|(i, s)| Tab::new(*s, i == self.tab, Msg::Tab(i)))
                            .collect(),
                    )),
            ),
            section(
                "Slider",
                "Drag, click the track, or use the arrow keys.",
                labeled(
                    "Volume",
                    row()
                        .gap(12.0)
                        .items_center()
                        .child(slider(self.volume, 0.0, 100.0).id("volume").on_change(Msg::Volume).grow(1.0))
                        .child(text(format!("{:.0}", self.volume)).w(32.0)),
                ),
            ),
        ]
    }

    fn data(&self) -> Vec<Element<Msg>> {
        let th = theme();
        let (col_i, dir) = self.sort;
        let mut order: Vec<usize> = (0..PEOPLE.len()).collect();
        order.sort_by(|&a, &b| {
            let (pa, pb) = (PEOPLE[a], PEOPLE[b]);
            let o = match col_i {
                0 => pa.0.cmp(pb.0),
                1 => pa.1.cmp(pb.1),
                2 => pa.2.cmp(&pb.2),
                _ => pa.3.cmp(pb.3),
            };
            if dir == SortDir::Desc {
                o.reverse()
            } else {
                o
            }
        });
        let people = table(
            "people",
            vec![
                Column::new("Name").sortable(),
                Column::new("Role").sortable(),
                Column::new("Age").fixed(70.0).align_end().sortable(),
                Column::new("City").sortable(),
            ],
            order.len(),
            move |r, c| {
                let p = PEOPLE[order[r]];
                match c {
                    0 => cell_text(p.0),
                    1 => cell_text(p.1),
                    2 => cell_text(p.2.to_string()),
                    _ => cell_text(p.3),
                }
            },
        )
        .sort(col_i, dir, Msg::Sort)
        .on_row_click(Msg::Row)
        .selected(self.selected_row)
        .striped(true);

        vec![
            section(
                "Table",
                "Sortable headers, resizable columns (drag the dividers), selection, a virtualized body.",
                Element::from(people).h(260.0),
            ),
            section(
                "Tree",
                "Lazy children, keyboard navigation like VS Code's explorer, type-ahead.",
                col()
                    .gap(8.0)
                    .child(self.tree.view(&self.files, Msg::Tree).h(200.0))
                    .child(text(format!("Last event: {}", self.last_action)).color(th.colors.text_muted)),
            ),
            section(
                "Virtual list",
                "100,000 rows; only those near the viewport exist.",
                virtual_list(100_000, move |i| {
                    list_item(None, format!("Row {i}"), false).when(i % 2 == 1, |e| e.bg(theme().colors.hover))
                })
                .id("rows")
                .h(200.0),
            ),
            section(
                "List items",
                "list_item and tree_row are the building blocks of sidebars.",
                col().children(["Inbox", "Drafts", "Sent", "Archive"].iter().enumerate().map(|(i, s)| {
                    list_item(Some(Icon::File), *s, self.list_selected == Some(i)).on_click(Msg::ListSelect(i))
                })),
            ),
        ]
    }

    fn feedback(&self) -> Vec<Element<Msg>> {
        let th = theme();
        let c = &th.colors;
        vec![
            section(
                "Progress",
                "progress takes a value from 0 to 1.",
                col().gap(10.0).child(progress(0.25)).child(progress(0.6)).child(progress(1.0)),
            ),
            section(
                "Badges, tags and keys",
                "badge, tag and kbd.",
                row()
                    .gap(8.0)
                    .items_center()
                    .flex_wrap()
                    .child(badge("New"))
                    .child(badge("12"))
                    .child(tag("bug", c.danger))
                    .child(tag("feature", c.success))
                    .child(tag("docs", c.accent))
                    .child(kbd("Ctrl+Shift+P")),
            ),
            section(
                "Avatars",
                "avatar with initials and a color.",
                row().gap(8.0).children(
                    [("AL", Accent::Violet), ("GH", Accent::Teal), ("AT", Accent::Orange), ("BL", Accent::Pink)]
                        .map(|(s, a)| avatar(s, a.color(self.dark))),
                ),
            ),
            section(
                "Status bar",
                "status_bar and status_item, as at the bottom of an editor.",
                status_bar()
                    .child(status_item(Some(Icon::GitBranch), "main"))
                    .child(status_item(Some(Icon::Warning), "2"))
                    .child(spacer())
                    .child(status_item(None, "Ln 12, Col 4"))
                    .child(status_item(None, "UTF-8")),
            ),
        ]
    }

    fn overlays(&self) -> Vec<Element<Msg>> {
        let menus = vec![
            Menu::new(
                "File",
                vec![
                    MenuItem::action("New", Msg::Action("New")).shortcut("Mod+N"),
                    MenuItem::action("Open…", Msg::Action("Open")).shortcut("Mod+O"),
                    MenuItem::Separator,
                    MenuItem::submenu(
                        "Export",
                        vec![
                            MenuItem::action("PDF", Msg::Action("PDF")),
                            MenuItem::action("HTML", Msg::Action("HTML")),
                        ],
                    ),
                ],
            ),
            Menu::new("View", vec![MenuItem::check("Word wrap", self.checked, Msg::Check)]),
        ];
        vec![
            section(
                "Menu bar",
                "menu_bar with submenus, check items and shortcuts. Arrow keys move between items.",
                row().h(32.0).child(menu_bar(menus, self.menu, Msg::Menu)),
            ),
            section(
                "Context menu",
                "Right-click the area below (or press Shift+F10 when it has focus).",
                div()
                    .id("context-area")
                    .h(90.0)
                    .center()
                    .rounded(8.0)
                    .border(1.0, theme().colors.border)
                    .focusable()
                    .on_context_menu(|p| Msg::Context(Some(p)))
                    .child(text("Right-click here").color(theme().colors.text_muted)),
            ),
            section(
                "Dialog and tooltip",
                "modal closes on Escape or a backdrop click. Hover the button for its tooltip.",
                row()
                    .gap(12.0)
                    .items_center()
                    .child(
                        danger_button("Discard changes…")
                            .id("open-dialog")
                            .tooltip("Opens a confirmation dialog")
                            .on_click(Msg::Dialog(true)),
                    )
                    .child(text(format!("Last action: {}", self.last_action)).id("last-action")),
            ),
        ]
    }

    fn media(&self) -> Vec<Element<Msg>> {
        let th = theme();
        let fits = [("Cover", Fit::Cover), ("Contain", Fit::Contain), ("Fill", Fit::Fill)];
        let mut svg_row = row().gap(16.0).items_center();
        if let Some(logo) = &self.logo {
            svg_row = svg_row.child(svg(logo).size(64.0, 64.0)).child(svg(logo).size(32.0, 32.0));
        }
        if let Some(mark) = &self.mark {
            svg_row = svg_row
                .child(vseparator().h(40.0))
                .child(svg(mark).size(32.0, 32.0).tint(th.colors.accent))
                .child(svg(mark).size(32.0, 32.0).tint(th.colors.text_muted))
                .child(svg(mark).size(32.0, 32.0).tint(th.colors.danger));
        }
        vec![
            section(
                "Images",
                "image with object-fit and rounded corners; cached per device size.",
                row().gap(16.0).children(fits.map(|(name, fit)| {
                    col()
                        .gap(6.0)
                        .items_center()
                        .child(image(self.photo.clone()).fit(fit).size(150.0, 110.0).rounded(10.0))
                        .child(text(name).color(th.colors.text_muted).font_size(th.font_size_sm))
                })),
            ),
            section(
                "SVG",
                "svg renders vector images at any size; tint recolors one-color art such as icons and logos.",
                svg_row,
            ),
            section(
                "Clipboard images",
                "Copy the photo, then click the box and paste it (or a screenshot) with Ctrl+V.",
                row()
                    .gap(16.0)
                    .items_center()
                    .child(
                        col()
                            .gap(8.0)
                            .child(button("Copy image").id("copy-image").on_click(Msg::CopyPhoto))
                            .child(button("Read clipboard").on_click(Msg::ReadClipboard)),
                    )
                    .child(
                        div()
                            .id("paste-target")
                            .size(220.0, 130.0)
                            .center()
                            .rounded(10.0)
                            .border(1.0, th.colors.border_strong)
                            .focusable()
                            .on_paste_image(Msg::Pasted)
                            .child(match &self.pasted {
                                Some(img) => image(img.clone()).fit(Fit::Contain).size(200.0, 110.0),
                                None => text("Click, then Ctrl+V").color(th.colors.text_muted),
                            }),
                    )
                    .child(text(self.last_action.clone()).color(th.colors.text_muted)),
            ),
            section(
                "Icons",
                "The built-in set; Icon::svg takes any 24×24 path.",
                div().grid((0..8).map(|_| Track::Fr(1.0)).collect()).gap(14.0).children(
                    [
                        Icon::File,
                        Icon::Files,
                        Icon::Folder,
                        Icon::Search,
                        Icon::Settings,
                        Icon::Terminal,
                        Icon::GitBranch,
                        Icon::Bell,
                        Icon::User,
                        Icon::Play,
                        Icon::Code,
                        Icon::Home,
                        Icon::Star,
                        Icon::Info,
                        Icon::Warning,
                        Icon::Layers,
                        Icon::Sun,
                        Icon::Moon,
                        Icon::Blocks,
                        Icon::Refresh,
                        Icon::Columns,
                        Icon::Pin,
                        Icon::PopOut,
                        Icon::svg("M12 2 L15 9 L22 9 L16.5 13.5 L18.5 21 L12 16.5 L5.5 21 L7.5 13.5 L2 9 L9 9 Z"),
                    ]
                    .map(|i| {
                        col().gap(4.0).items_center().child(icon(i.clone()).font_size(20.0)).tooltip(format!("{i:?}"))
                    }),
                ),
            ),
        ]
    }

    fn typography(&self) -> Vec<Element<Msg>> {
        let th = theme();
        let c = &th.colors;
        vec![
            section(
                "Sizes and weights",
                "font_size, weight, italic and mono. Text properties inherit like CSS.",
                col()
                    .gap(6.0)
                    .child(text("Display heading").font_size(28.0).bold())
                    .child(text("Section heading").font_size(18.0).semibold())
                    .child(text("Body text at the theme's size, which is 13 px by default."))
                    .child(text("Muted secondary text").color(c.text_muted))
                    .child(text("Italic for emphasis").italic())
                    .child(text("fn main() { println!(\"monospace\"); }").mono()),
            ),
            section(
                "Rich and selectable text",
                "rich_text mixes spans; selectable text can be copied.",
                col()
                    .gap(8.0)
                    .child(rich_text(vec![
                        span("Build "),
                        span("failed").bold().color(c.danger),
                        span(" in "),
                        span("2.1s").mono(),
                        span(". See the "),
                        span("log").link("log"),
                        span("."),
                    ]))
                    .child(text("Select this sentence and copy it with Ctrl+C.").selectable()),
            ),
            section(
                "Truncation",
                "ellipsis cuts long single lines.",
                text("A very long file name that does not fit in its column: src/runtime/components/inspector.rs")
                    .nowrap()
                    .ellipsis()
                    .w(320.0),
            ),
            section(
                "Markdown",
                "markdown renders headings, lists, emphasis, code and links.",
                markdown(
                    "### Release notes\n\n- **Faster** layout with *incremental* updates\n- New `number_input` widget\n\n```rust\nlet x = 42;\n```",
                ),
            ),
        ]
    }

    fn theme_page(&self) -> Vec<Element<Msg>> {
        let th = theme();
        vec![
            section(
                "Accent",
                "Any color works; these are the 16 presets. The whole palette is derived from it.",
                row().gap(8.0).flex_wrap().children(Accent::ALL.iter().enumerate().map(|(i, a)| {
                    color_swatch(a.color(self.dark), i == self.accent).tooltip(a.name()).on_click(Msg::Accent(i))
                })),
            ),
            section(
                "Mode and contrast",
                "Light or dark, and a high-contrast version of either.",
                col().gap(10.0).child(switch_row("Dark mode", self.dark).on_click(Msg::Dark(!self.dark))).child(
                    switch_row("High contrast", self.high_contrast)
                        .id("high-contrast")
                        .on_click(Msg::HighContrast(!self.high_contrast)),
                ),
            ),
            section(
                "Shape and density",
                "Radius scales every corner; density sets control and row heights.",
                col()
                    .gap(12.0)
                    .child(labeled(
                        "Radius",
                        row()
                            .gap(12.0)
                            .items_center()
                            .child(slider(self.radius, 0.0, 16.0).on_change(Msg::Radius).grow(1.0))
                            .child(text(format!("{:.0} px", self.radius)).w(48.0)),
                    ))
                    .child(labeled(
                        "Density",
                        row().child(segmented(
                            ["Compact", "Default", "Comfortable"]
                                .iter()
                                .enumerate()
                                .map(|(i, s)| (s.to_string(), i == self.density, Msg::Density(i)))
                                .collect(),
                        )),
                    )),
            ),
            section(
                "Preview",
                "Widgets pick the knobs up immediately.",
                row()
                    .gap(8.0)
                    .items_center()
                    .child(primary_button("Primary"))
                    .child(button("Secondary"))
                    .child(text_input("Text", |_| Msg::Click).w(160.0))
                    .child(checkbox("Checked", true))
                    .child(badge("Badge"))
                    .child(text(format!("accent {}", Accent::ALL[self.accent].name())).color(th.colors.text_muted)),
            ),
        ]
    }
}

#[allow(dead_code)]
fn main() {
    #[cfg(feature = "window")]
    charis_ui::run(Gallery::default(), WindowOptions::new("Widget gallery").size(1180.0, 820.0).min_size(760.0, 480.0))
        .expect("failed to open the window");
}
