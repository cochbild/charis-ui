//! Drag-and-drop docking: drag any tab onto another panel's center to join it,
//! or onto an edge to split it. Drag a tab out of the window to open it in its
//! own window, and drag it back (or press "Dock back") to re-dock it. Every
//! splitter is resizable (also with the arrow keys once focused). Right-click
//! a tab for its commands. Window ▸ Layouts switches between saved layouts.
//! Every command has a rebindable key: Ctrl+Shift+P (⇧⌘P) searches them,
//! Ctrl+K Ctrl+S (⌘K ⌘S) edits the bindings, Ctrl+1…6 focus panel 1…6.
//! Alt+1 opens the Project tree (a virtualized tree with a 100,000-file
//! folder). F11 (⌃⌘F on macOS) toggles full screen.
//!
//! Options: `--mica` (Windows 11 Mica backdrop), `--system-font` (the
//! platform's UI font instead of Inter).
//!
//! Run:        cargo run --release --example dock
//! Screenshot: cargo run --release --example dock -- --screenshot dock.png

use rust_ui::commands::{pending_chord, CommandPalette, KeymapEditor, KeymapMsg, PaletteMsg};
use rust_ui::dock::{DockSpace, DockSpaceMsg};
use rust_ui::layouts::{LayoutMsg, Layouts};
use rust_ui::prelude::*;
use rust_ui::toolwin::{Side, ToolMode, ToolMsg, ToolWindows};
use rust_ui::tree::{TreeEvent, TreeModel, TreeMsg, TreeState};

#[derive(Clone, Copy, PartialEq, Debug)]
enum Panel {
    Explorer,
    Outline,
    Editor(&'static str),
    Terminal,
    Problems,
    Preview,
    Chat,
}

impl Panel {
    fn title(&self) -> String {
        match self {
            Panel::Explorer => "Explorer".into(),
            Panel::Outline => "Outline".into(),
            Panel::Editor(f) => f.to_string(),
            Panel::Terminal => "Terminal".into(),
            Panel::Problems => "Problems".into(),
            Panel::Preview => "Preview".into(),
            Panel::Chat => "Assistant".into(),
        }
    }
    fn icon(&self) -> Icon {
        match self {
            Panel::Explorer => Icon::Files,
            Panel::Outline => Icon::Layers,
            Panel::Editor(_) => Icon::Code,
            Panel::Terminal => Icon::Terminal,
            Panel::Problems => Icon::Warning,
            Panel::Preview => Icon::Play,
            Panel::Chat => Icon::User,
        }
    }
}

/// Tool windows in the edge stripes (JetBrains style), around the dock.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Tool {
    Project,
    Bookmarks,
    Git,
    Build,
    Todo,
}

impl Tool {
    fn icon(&self) -> Icon {
        match self {
            Tool::Project => Icon::Folder,
            Tool::Bookmarks => Icon::Star,
            Tool::Git => Icon::GitBranch,
            Tool::Build => Icon::Play,
            Tool::Todo => Icon::Check,
        }
    }
}

fn tools() -> ToolWindows<Tool> {
    ToolWindows::new()
        .add(Tool::Project, Side::Left, ToolMode::AutoHide)
        .add(Tool::Bookmarks, Side::Left, ToolMode::AutoHide)
        .add(Tool::Git, Side::Right, ToolMode::AutoHide)
        .add(Tool::Todo, Side::Right, ToolMode::Pinned)
        .add(Tool::Build, Side::Bottom, ToolMode::AutoHide)
}

fn tool_content(t: &Tool) -> Element<Msg> {
    let th = theme();
    let c = &th.colors;
    match t {
        Tool::Project => div(), // drawn by DockDemo::tool_view (it needs the tree state)
        Tool::Bookmarks => col().p(10.0).gap(2.0).children(
            ["main.rs:12  fn main", "dock.rs:88  fn update", "theme.rs:210  Palette"]
                .map(|b| tree_row(0, None, Some(Icon::Star), b, false)),
        ),
        Tool::Git => col().p(12.0).gap(8.0).child(text("Changes").semibold()).children(
            ["M  src/dock.rs", "M  src/toolwin.rs", "A  examples/dock.rs"].map(|f| text(f).mono().font_size(12.0)),
        ),
        Tool::Build => col()
            .p(12.0)
            .mono()
            .font_size(12.0)
            .gap(2.0)
            .child(text("   Compiling rust-ui v0.1.0").color(c.success))
            .child(text("    Finished `release` profile in 12.4s").color(c.text_muted)),
        Tool::Todo => col().p(12.0).gap(6.0).children(
            ["Keyboard resizing of splitters", "Workspaces", "Compass drop targets"]
                .map(|t| row().gap(8.0).items_center().child(checkbox(t, false))),
        ),
    }
}

struct DockDemo {
    dock: DockSpace<Panel>,
    tools: ToolWindows<Tool>,
    layouts: Layouts<Workspace>,
    dark: bool,
    /// The user's key binding overrides.
    keymap: Keymap,
    palette: CommandPalette,
    keys: KeymapEditor,
    show_keys: bool,
    files: std::rc::Rc<Repo>,
    project: TreeState<String>,
}

/// A made-up repository for the Project tree, with one very large folder.
struct Repo;

const BIG: usize = 100_000;

impl TreeModel for Repo {
    type Id = String;
    fn children(&self, parent: Option<&String>) -> Vec<String> {
        let names: &[&str] = match parent.map(String::as_str) {
            None => &["rust-ui"],
            Some("rust-ui") => {
                &["rust-ui/src", "rust-ui/examples", "rust-ui/generated", "rust-ui/Cargo.toml", "rust-ui/README.md"]
            }
            Some("rust-ui/src") => {
                &["rust-ui/src/dock.rs", "rust-ui/src/tree.rs", "rust-ui/src/widgets.rs", "rust-ui/src/runtime"]
            }
            Some("rust-ui/src/runtime") => &["rust-ui/src/runtime/mod.rs", "rust-ui/src/runtime/memo.rs"],
            Some("rust-ui/examples") => &["rust-ui/examples/dock.rs", "rust-ui/examples/showcase.rs"],
            Some("rust-ui/generated") => {
                return (0..BIG).map(|i| format!("rust-ui/generated/file_{i:06}.rs")).collect()
            }
            _ => &[],
        };
        names.iter().map(|s| s.to_string()).collect()
    }
    fn has_children(&self, id: &String) -> bool {
        !id.contains('.')
    }
    fn label(&self, id: &String) -> String {
        match id.rsplit('/').next() {
            Some("generated") => format!("generated ({BIG} files)"),
            Some(n) => n.to_string(),
            None => id.clone(),
        }
    }
    fn icon(&self, id: &String, _: bool) -> Option<Icon> {
        Some(if !id.contains('.') {
            Icon::Folder
        } else if id.ends_with(".rs") {
            Icon::Code
        } else {
            Icon::File
        })
    }
}

/// What a saved layout holds: the dock (including floating windows) and
/// the tool windows.
#[derive(Clone)]
struct Workspace {
    dock: DockSpace<Panel>,
    tools: ToolWindows<Tool>,
}

impl DockDemo {
    fn new() -> Self {
        let layouts = Layouts::new()
            .with("Default", Workspace { dock: initial(), tools: tools() })
            .with("Focus", focus())
            .with("Review", review());
        DockDemo {
            dock: initial(),
            tools: tools(),
            layouts,
            dark: true,
            keymap: Keymap::new(),
            palette: CommandPalette::new(),
            keys: KeymapEditor::new(),
            show_keys: false,
            files: std::rc::Rc::new(Repo),
            project: {
                let mut t = TreeState::new("project-tree");
                t.expand("rust-ui".into());
                t.expand("rust-ui/src".into());
                t
            },
        }
    }
}

#[derive(Clone)]
enum Msg {
    Dock(DockSpaceMsg),
    Theme,
    ResetLayout,
    DockAllBack,
    Open(Panel),
    Tool(ToolMsg<Tool>),
    Layout(LayoutMsg),
    Palette(PaletteMsg),
    Keys(KeymapMsg),
    ShowKeys(bool),
    /// Focus the active tab of dock group N (from 0).
    FocusPanel(usize),
    ToggleTool(Tool),
    Project(TreeMsg<String>),
    ToggleFullscreen,
}

impl App for DockDemo {
    type Msg = Msg;
    fn theme(&self) -> Theme {
        if self.dark {
            Theme::dark()
        } else {
            Theme::light()
        }
    }
    fn update(&mut self, msg: Msg, cx: &mut Cx<Msg>) {
        match msg {
            Msg::Palette(m) => {
                if let Some(run) = self.palette.update(m, &self.commands()) {
                    self.update(run, cx);
                }
            }
            Msg::Keys(m) => self.keys.update(m, &mut self.keymap),
            Msg::ShowKeys(on) => self.show_keys = on,
            Msg::FocusPanel(n) => {
                if let Some(id) = self.dock.focus_id(n) {
                    cx.focus(&id);
                }
            }
            Msg::ToggleTool(t) => self.tools.update(ToolMsg::Toggle(t)),
            Msg::ToggleFullscreen => cx.toggle_fullscreen(),
            Msg::Project(m) => {
                // Opening a file opens it in the editor group.
                if let Some(TreeEvent::Activated(path)) = self.project.update(m, &*self.files, cx) {
                    let name: &'static str =
                        Box::leak(path.rsplit('/').next().unwrap_or("file").to_string().into_boxed_str());
                    self.dock.open(Panel::Editor(name));
                }
            }
            Msg::Dock(m) => self.dock.update(m),
            Msg::Theme => self.dark = !self.dark,
            Msg::ResetLayout => {
                self.dock = initial();
                self.tools = tools();
            }
            Msg::DockAllBack => self.dock.dock_all_back(),
            Msg::Open(p) => self.dock.open(p),
            Msg::Tool(m) => self.tools.update(m),
            Msg::Layout(m) => {
                let now = Workspace { dock: self.dock.clone(), tools: self.tools.clone() };
                if let Some(w) = self.layouts.update(m, &now) {
                    self.dock = w.dock;
                    self.tools = w.tools;
                }
            }
        }
    }
    fn on_key(&self, k: &KeyEvent) -> Option<Msg> {
        self.tools.key(k).map(Msg::Tool)
    }
    /// Every action, with its default keys; the user's keymap applies on top.
    fn commands(&self) -> Commands<Msg> {
        let mut list = vec![
            Command::new("file.newEditor", "New Editor", Msg::Open(Panel::Editor("untitled.rs")))
                .category("File")
                .key("Mod+N"),
            Command::new("view.commandPalette", "Command Palette…", Msg::Palette(PaletteMsg::Open))
                .category("View")
                .key("Mod+Shift+P")
                .key("F1"),
            Command::new("view.toggleTheme", "Dark Theme", Msg::Theme)
                .category("View")
                .key("Mod+Shift+T")
                .checked(self.dark),
            Command::new("view.resetLayout", "Reset Layout", Msg::ResetLayout).category("View").key("Mod+Shift+R"),
            Command::new("view.fullscreen", "Full Screen", Msg::ToggleFullscreen)
                .category("View")
                .key(if cfg!(target_os = "macos") { "Ctrl+Cmd+F" } else { "F11" })
                .checked(window_info().fullscreen),
            Command::new("view.hideToolWindows", "Hide All Tool Windows", Msg::Tool(ToolMsg::HideAll))
                .category("View")
                .key("Mod+Shift+F12"),
            Command::new("window.dockAllBack", "Dock All Windows Back", Msg::DockAllBack)
                .category("Window")
                .key("Mod+Shift+W")
                .enabled(!self.dock.floating.is_empty()),
            Command::new("layout.saveAs", "Save Layout As…", Msg::Layout(LayoutMsg::SaveAs)).category("Layout"),
            Command::new("preferences.keyboardShortcuts", "Keyboard Shortcuts", Msg::ShowKeys(true))
                .category("Preferences")
                .key("Mod+K Mod+S"),
        ];
        for n in 1..=6 {
            list.push(
                Command::new(format!("view.focusPanel{n}"), format!("Focus Panel {n}"), Msg::FocusPanel(n - 1))
                    .category("View")
                    .key(&format!("Mod+{n}")),
            );
        }
        // JetBrains-style Alt+number for tool windows.
        for (t, key) in [
            (Tool::Project, "Alt+1"),
            (Tool::Bookmarks, "Alt+2"),
            (Tool::Build, "Alt+4"),
            (Tool::Todo, "Alt+6"),
            (Tool::Git, "Alt+9"),
        ] {
            list.push(
                Command::new(format!("view.tool.{t:?}"), format!("{t:?}"), Msg::ToggleTool(t))
                    .category("Tool Window")
                    .key(key)
                    .checked(self.tools.is_open(&t)),
            );
        }
        for name in self.layouts.names() {
            list.push(
                Command::new(format!("layout.apply.{name}"), name, Msg::Layout(LayoutMsg::Apply(name.to_string())))
                    .category("Layout"),
            );
        }
        Commands::new(list).with_keymap(&self.keymap)
    }
    fn menu(&self) -> Vec<Menu<Msg>> {
        let open = |p: Panel| MenuItem::action(p.title(), Msg::Open(p)).icon(p.icon());
        let c = self.commands();
        vec![
            Menu::new(
                "File",
                vec![c.menu_item("file.newEditor"), MenuItem::Separator, c.menu_item("preferences.keyboardShortcuts")],
            ),
            Menu::new(
                "View",
                vec![
                    c.menu_item("view.commandPalette"),
                    MenuItem::Separator,
                    c.menu_item("view.toggleTheme"),
                    c.menu_item("view.resetLayout"),
                    c.menu_item("view.fullscreen"),
                    MenuItem::submenu(
                        "Tool Windows",
                        ["Project", "Bookmarks", "Build", "Todo", "Git"]
                            .iter()
                            .map(|t| c.menu_item(&format!("view.tool.{t}")))
                            .collect(),
                    ),
                    c.menu_item("view.hideToolWindows"),
                    MenuItem::submenu(
                        "Focus Panel",
                        (1..=6).map(|n| c.menu_item(&format!("view.focusPanel{n}"))).collect(),
                    ),
                ],
            ),
            Menu::new(
                "Window",
                vec![
                    MenuItem::submenu(
                        "Open Panel",
                        vec![
                            open(Panel::Explorer),
                            open(Panel::Outline),
                            open(Panel::Terminal),
                            open(Panel::Problems),
                            open(Panel::Preview),
                            open(Panel::Chat),
                        ],
                    ),
                    MenuItem::submenu("Layouts", self.layouts.menu_items(&Msg::Layout)),
                    MenuItem::Separator,
                    c.menu_item("window.dockAllBack"),
                ],
            ),
        ]
    }
    fn view(&self) -> Element<Msg> {
        let th = theme();
        let bar = titlebar(
            "Docking demo — drag tabs onto panel edges",
            row()
                .pl(12.0)
                .gap(8.0)
                .items_center()
                .child(icon(Icon::Grid).font_size(16.0).color(th.colors.accent))
                .child(menubar(self.menu())),
            row()
                .pr(6.0)
                .gap(6.0)
                .items_center()
                .children(
                    self.layouts
                        .current()
                        .map(|l| badge(format!("Layout: {l}")).tooltip("Window ▸ Layouts to switch or save")),
                )
                .child(
                    icon_button(if self.dark { Icon::Sun } else { Icon::Moon })
                        .on_click(Msg::Theme)
                        .tooltip("Toggle theme"),
                ),
            window_info().maximized,
        );
        let dock = self.dock.view_with_icons(None, Panel::title, |p| Some(p.icon()), panel_content, Msg::Dock);
        let body = self.tools.view(dock, |t| format!("{t:?}"), Tool::icon, |t| self.tool_view(t), Msg::Tool);
        let cmds = self.commands();
        // A chord's first stroke, like VS Code's status bar hint.
        let status = status_bar().child(text(match pending_chord() {
            Some(k) => format!("({k}) was pressed. Waiting for the second key of the chord…"),
            None => format!(
                "{} commands · {} keyboard shortcuts",
                cmds.key_label("view.commandPalette").unwrap_or_default(),
                cmds.key_label("preferences.keyboardShortcuts").unwrap_or_default()
            ),
        }));
        col()
            .size_full()
            .child(bar)
            .child(body)
            .child(status)
            .children(self.layouts.dialog(Msg::Layout))
            .children(self.show_keys.then(|| self.keys_dialog(&cmds)))
            .children(self.palette.view(&cmds, Msg::Palette))
    }
    fn windows(&self) -> Vec<WindowSpec<Msg>> {
        self.dock.windows(Panel::title, Msg::Dock)
    }
    fn window_view(&self, key: &str) -> Element<Msg> {
        col().size_full().child(self.dock.view_with_icons(
            Some(key),
            Panel::title,
            |p| Some(p.icon()),
            panel_content,
            Msg::Dock,
        ))
    }
}

fn panel_content(p: &Panel) -> Element<Msg> {
    let th = theme();
    let c = &th.colors;
    match p {
        Panel::Explorer => col().py(6.0).scroll_y().children(
            ["src", "main.rs", "app.rs", "dock.rs", "theme.rs", "Cargo.toml", "README.md"].iter().enumerate().map(
                |(i, f)| {
                    tree_row(
                        if i == 0 { 0 } else { 1 },
                        if i == 0 { Some(true) } else { None },
                        Some(if i == 0 { Icon::Folder } else { Icon::File }),
                        *f,
                        i == 1,
                    )
                },
            ),
        ),
        Panel::Outline => col().p(12.0).gap(6.0).color(c.text_muted).children(
            ["struct DockDemo", "enum Msg", "impl App", "fn panel_content", "fn main"]
                .map(|s| text(s).mono().font_size(12.0)),
        ),
        Panel::Editor(f) => col()
            .p(18.0)
            .gap(4.0)
            .mono()
            .font_size(13.0)
            .child(text(format!("// {f}")).color(c.text_faint))
            .child(text("fn main() {").color(c.text))
            .child(text("    println!(\"hello, docking\");").color(c.success))
            .child(text("}").color(c.text)),
        Panel::Terminal => col()
            .p(12.0)
            .mono()
            .font_size(12.5)
            .gap(2.0)
            .child(text("$ cargo run --example dock").color(c.text))
            .child(text("   Running `target/release/examples/dock`").color(c.text_muted)),
        Panel::Problems => col().p(12.0).gap(6.0).child(
            row()
                .gap(8.0)
                .items_center()
                .child(icon(Icon::Warning).color(c.warning))
                .child(text("unused import `Rect`").color(c.text_muted)),
        ),
        Panel::Preview => col()
            .center()
            .gap(10.0)
            .child(
                div()
                    .w(160.0)
                    .h(100.0)
                    .rounded(12.0)
                    .gradient(135.0, [(0.0, c.accent), (1.0, hex("#a371f7"))])
                    .shadows(th.shadow_popover.clone()),
            )
            .child(text("Live preview").color(c.text_muted)),
        Panel::Chat => col()
            .p(12.0)
            .gap(10.0)
            .child(
                card().p(10.0).child(
                    text("Try dragging the “Terminal” tab onto the right edge of the editor, or out of the window.")
                        .color(c.text_muted),
                ),
            )
            .child(spacer())
            .child(text_input("", |_| Msg::Theme).placeholder("Ask something…")),
    }
}

fn initial() -> DockSpace<Panel> {
    DockSpace::new(Dock::new(DockNode::hsplit(vec![
        (
            1.0,
            DockNode::vsplit(vec![
                (2.0, DockNode::tabs(vec![Panel::Explorer])),
                (1.0, DockNode::tabs(vec![Panel::Outline])),
            ]),
        ),
        (
            3.2,
            DockNode::vsplit(vec![
                (
                    2.4,
                    DockNode::tabs(vec![Panel::Editor("main.rs"), Panel::Editor("app.rs"), Panel::Editor("dock.rs")]),
                ),
                (1.0, DockNode::tabs(vec![Panel::Terminal, Panel::Problems])),
            ]),
        ),
        (
            1.3,
            DockNode::vsplit(vec![
                (1.0, DockNode::tabs(vec![Panel::Preview])),
                (1.0, DockNode::tabs(vec![Panel::Chat])),
            ]),
        ),
    ])))
}

impl DockDemo {
    /// A tool window's content (the Project tree needs the app's state).
    fn tool_view(&self, t: &Tool) -> Element<Msg> {
        match t {
            Tool::Project => self.project.view(&self.files, Msg::Project),
            other => tool_content(other),
        }
    }

    /// The keyboard shortcuts editor, in a large dialog.
    fn keys_dialog(&self, cmds: &Commands<Msg>) -> Element<Msg> {
        let th = theme();
        let c = &th.colors;
        let close = Msg::ShowKeys(false);
        let esc = close.clone();
        div().child(backdrop(close.clone(), true)).child(
            div().fixed().top(0.0).left(0.0).right(0.0).bottom(0.0).z_index(95).center().pointer_events(false).child(
                col()
                    .pointer_events(true)
                    .aria_modal()
                    .aria_label("Keyboard Shortcuts")
                    .on_key(move |k| (k.key == Key::Escape).then(|| esc.clone()))
                    .w(820.0)
                    .h(560.0)
                    .max_w(pct(94.0))
                    .max_h(pct(90.0))
                    .p(16.0)
                    .gap(10.0)
                    .bg(c.elevated)
                    .border(1.0, c.border_strong)
                    .rounded(th.radius_lg + 2.0)
                    .shadows(th.shadow_popover.clone())
                    .child(
                        row()
                            .items_center()
                            .child(text("Keyboard Shortcuts").font_size(th.font_size_lg).semibold().grow(1.0))
                            .child(icon_button(Icon::Close).aria_label("Close").on_click(close)),
                    )
                    .child(self.keys.view(cmds, &self.keymap, Msg::Keys)),
            ),
        )
    }
}

/// Just the editors and the terminal; everything else in auto-hide tool windows.
fn focus() -> Workspace {
    let dock = DockSpace::new(Dock::new(DockNode::vsplit(vec![
        (4.0, DockNode::tabs(vec![Panel::Editor("main.rs"), Panel::Editor("app.rs"), Panel::Editor("dock.rs")])),
        (1.0, DockNode::tabs(vec![Panel::Terminal])),
    ])));
    let mut tools = tools();
    for w in &mut tools.windows {
        w.mode = ToolMode::AutoHide;
    }
    Workspace { dock, tools }
}

/// Code and preview side by side, with Git changes pinned open.
fn review() -> Workspace {
    let dock = DockSpace::new(Dock::new(DockNode::hsplit(vec![
        (1.0, DockNode::tabs(vec![Panel::Explorer])),
        (2.4, DockNode::tabs(vec![Panel::Editor("dock.rs"), Panel::Editor("main.rs")])),
        (
            2.0,
            DockNode::vsplit(vec![
                (2.0, DockNode::tabs(vec![Panel::Preview])),
                (1.0, DockNode::tabs(vec![Panel::Problems])),
            ]),
        ),
    ])));
    let mut tools = tools();
    tools.update(ToolMsg::SetMode(Tool::Git, ToolMode::Pinned));
    tools.update(ToolMsg::Toggle(Tool::Git));
    Workspace { dock, tools }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // For scripted UI tests: print where a text is (center, window coordinates).
    if let Some(pos) = args.iter().position(|a| a == "--where") {
        let mut h = Headless::new(DockDemo::new(), 1280.0, 800.0, 1.0);
        h.settle();
        let t = args.get(pos + 1).expect("--where TEXT");
        let c = h.rt.rect_of_text(t).expect("text not found").center();
        println!("{} {}", c.x.round(), c.y.round());
        return;
    }
    if let Some(pos) = args.iter().position(|a| a == "--screenshot") {
        let out = args.get(pos + 1).cloned().unwrap_or_else(|| "dock.png".into());
        let mut h = Headless::new(DockDemo::new(), 1280.0, 800.0, 1.0);
        h.settle();
        let state = args.iter().position(|a| a == "--state").and_then(|i| args.get(i + 1)).cloned();
        if state.as_deref() == Some("reorder") {
            // Tab strip of the editor group (the Explorer also lists these names).
            let from = Point::new(288.0, 54.0);
            let to = Rect::new(486.0, 40.0, 60.0, 28.0);
            h.move_to(from.x, from.y);
            h.event(rust_ui::Event::PointerDown(from, rust_ui::MouseButton::Left));
            for i in 1..=8 {
                let t = i as f32 / 8.0;
                h.move_to(from.x + (to.x + 6.0 - from.x) * t, from.y);
            }
            h.settle();
            h.save_png(&out).expect("save");
            println!("saved {out}");
            return;
        }
        if state.as_deref() == Some("tools") {
            // Todo pinned on the right, Build sliding up over the editor.
            h.rt.send(Msg::Tool(ToolMsg::Toggle(Tool::Todo)));
            h.rt.send(Msg::Tool(ToolMsg::Toggle(Tool::Build)));
            h.settle();
            h.save_png(&out).expect("save");
            println!("saved {out}");
            return;
        }
        if state.as_deref() == Some("menu") {
            // Window → Open Panel ▸ submenu.
            let w = h.rt.rect_of_text("Window").unwrap().center();
            h.click(w.x, w.y);
            let sub = h.rt.rect_of_text("Open Panel").unwrap().center();
            h.move_to(sub.x, sub.y);
            h.settle();
            h.save_png(&out).expect("save");
            println!("saved {out}");
            return;
        }
        if state.as_deref() == Some("tabmenu") {
            // Right-click the "app.rs" tab (the editor group's second), hover "Move to Edge".
            let t = Point::new(421.0, 56.0);
            h.event(rust_ui::Event::PointerDown(t, rust_ui::MouseButton::Right));
            h.event(rust_ui::Event::PointerUp(t, rust_ui::MouseButton::Right));
            h.settle();
            let sub = h.rt.rect_of_text("Move to Edge").unwrap().center();
            h.move_to(sub.x, sub.y);
            h.settle();
            h.save_png(&out).expect("save");
            println!("saved {out}");
            return;
        }
        if state.as_deref() == Some("layouts") {
            // Window ▸ Layouts, with "Review" applied.
            h.rt.send(Msg::Layout(LayoutMsg::Apply("Review".into())));
            h.settle();
            let w = h.rt.rect_of_text("Window").unwrap().center();
            h.click(w.x, w.y);
            let sub = h.rt.rect_of_text("Layouts").unwrap().center();
            h.move_to(sub.x, sub.y);
            h.settle();
            h.save_png(&out).expect("save");
            println!("saved {out}");
            return;
        }
        if state.as_deref() == Some("saveas") {
            h.rt.send(Msg::Layout(LayoutMsg::SaveAs));
            h.settle();
            h.type_text("Debugging");
            h.settle();
            h.save_png(&out).expect("save");
            println!("saved {out}");
            return;
        }
        if state.as_deref() == Some("palette") {
            h.rt.send(Msg::Palette(PaletteMsg::Open));
            h.settle();
            h.type_text("focus");
            h.settle();
            h.save_png(&out).expect("save");
            println!("saved {out}");
            return;
        }
        if state.as_deref() == Some("keys") {
            h.rt.send(Msg::ShowKeys(true));
            h.rt.send(Msg::Keys(KeymapMsg::Record("view.resetLayout".into())));
            h.settle();
            for key in [Key::Char('k'), Key::Char('r')] {
                let mods = Modifiers { ctrl: true, ..Default::default() };
                h.event(rust_ui::Event::Key(KeyEvent { key, mods, repeat: false }));
            }
            h.settle();
            h.save_png(&out).expect("save");
            println!("saved {out}");
            return;
        }
        if state.as_deref() == Some("tree") {
            // The Project tree with the 100k-file folder open, near its end.
            h.rt.send(Msg::ToggleTool(Tool::Project));
            h.rt.app.project.expand("rust-ui/generated".into());
            h.rt.app.project.select(Some("rust-ui/generated/file_099990.rs".into()));
            h.rt.invalidate();
            h.settle();
            let i = h.rt.app.project.index_of(&*h.rt.app.files, &"rust-ui/generated/file_099990.rs".into()).unwrap();
            h.rt.scroll_item_into_view("project-tree", i + 8);
            h.settle();
            h.save_png(&out).expect("save");
            println!("saved {out}");
            return;
        }
        if state.as_deref() == Some("maximize") {
            let t = h.rt.rect_of_text("Terminal").unwrap().center();
            h.click(t.x, t.y);
            h.click(t.x, t.y);
            h.settle();
            h.save_png(&out).expect("save");
            println!("saved {out}");
            return;
        }
        // Drag "Terminal" (bottom-center group) toward the editor's right edge, and
        // capture the frame mid-drag to show the drop preview.
        let from =
            (h.rt.rect_of_text("Terminal").unwrap().center().x, h.rt.rect_of_text("Terminal").unwrap().center().y);
        h.move_to(from.0, from.1);
        h.event(rust_ui::Event::PointerDown(Point::new(from.0, from.1), rust_ui::MouseButton::Left));
        for i in 1..=10 {
            let t = i as f32 / 10.0;
            h.move_to(from.0 + (900.0 - from.0) * t, from.1 + (260.0 - from.1) * t);
        }
        h.settle();
        h.save_png(&out).expect("save");
        h.event(rust_ui::Event::PointerUp(Point::new(900.0, 260.0), rust_ui::MouseButton::Left));
        h.settle();
        let after = out.replace(".png", "-after.png");
        h.save_png(&after).expect("save");
        println!("saved {out} and {after}");
        return;
    }
    let mut opts = WindowOptions::new("Dock demo").size(1280.0, 800.0).frameless(true);
    if args.iter().any(|a| a == "--mica") {
        opts = opts.backdrop(Backdrop::Mica);
    }
    if args.iter().any(|a| a == "--system-font") {
        opts = opts.system_font(true);
    }
    rust_ui::run(DockDemo::new(), opts).expect("run");
}
