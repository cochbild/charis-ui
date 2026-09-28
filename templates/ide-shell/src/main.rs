//! An IDE-style shell built on Charis: a custom title bar with menus, a
//! dock of editor tabs that can be torn out into their own windows, tool
//! windows on the sides, a command palette, rebindable shortcuts, named
//! layouts and a status bar.
//!
//! Start here:
//! - `Panel` lists what can be docked; `panel_view` draws each one.
//! - `Tool` lists the side tool windows; `tool_view` draws each one.
//! - `commands()` holds every action with its default keys. Menus, the
//!   palette and the shortcuts editor are built from it.

use std::collections::BTreeMap;
use std::rc::Rc;

use charis_ui::commands::{pending_chord, CommandPalette, KeymapEditor, KeymapMsg, PaletteMsg};
use charis_ui::dock::{DockSpace, DockSpaceMsg};
use charis_ui::layouts::{LayoutMsg, Layouts};
use charis_ui::prelude::*;
use charis_ui::toolwin::{Side, ToolMode, ToolMsg, ToolWindows};
use charis_ui::tree::{TreeEvent, TreeModel, TreeMsg, TreeState};

/// What can be docked.
#[derive(Clone, Debug, PartialEq)]
pub enum Panel {
    Editor(String),
    Terminal,
    Problems,
}

impl Panel {
    fn title(&self) -> String {
        match self {
            Panel::Editor(path) => path.rsplit('/').next().unwrap_or(path).to_string(),
            Panel::Terminal => "Terminal".into(),
            Panel::Problems => "Problems".into(),
        }
    }

    fn icon(&self) -> Option<Icon> {
        Some(match self {
            Panel::Editor(_) => Icon::File,
            Panel::Terminal => Icon::Terminal,
            Panel::Problems => Icon::Warning,
        })
    }
}

/// The side tool windows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tool {
    Project,
    Outline,
}

impl Tool {
    fn title(&self) -> String {
        format!("{self:?}")
    }

    fn icon(&self) -> Icon {
        match self {
            Tool::Project => Icon::Folder,
            Tool::Outline => Icon::Layers,
        }
    }
}

/// The arrangement saved by named layouts.
#[derive(Clone)]
pub struct Workspace {
    pub dock: DockSpace<Panel>,
    pub tools: ToolWindows<Tool>,
}

fn default_workspace() -> Workspace {
    let dock = DockSpace::new(Dock::new(DockNode::vsplit(vec![
        (3.0, DockNode::tabs(vec![Panel::Editor("src/main.rs".into())])),
        (1.0, DockNode::tabs(vec![Panel::Terminal, Panel::Problems])),
    ])));
    let mut tools = ToolWindows::new().add(Tool::Project, Side::Left, ToolMode::Pinned).add(
        Tool::Outline,
        Side::Right,
        ToolMode::AutoHide,
    );
    tools.show(&Tool::Project);
    Workspace { dock, tools }
}

/// The project's files. Replace with a real file system walk.
#[derive(Clone)]
pub struct Project {
    pub files: BTreeMap<String, String>,
}

impl TreeModel for Project {
    type Id = String;

    fn children(&self, parent: Option<&String>) -> Vec<String> {
        let prefix = parent.map(|p| format!("{p}/")).unwrap_or_default();
        let mut out: Vec<String> = Vec::new();
        for path in self.files.keys() {
            if let Some(rest) = path.strip_prefix(&prefix) {
                let first = rest.split('/').next().unwrap_or(rest);
                let child = format!("{prefix}{first}");
                if !out.contains(&child) {
                    out.push(child);
                }
            }
        }
        // Folders first.
        out.sort_by_key(|c| (!self.has_children(c), c.clone()));
        out
    }

    fn has_children(&self, id: &String) -> bool {
        let prefix = format!("{id}/");
        self.files.keys().any(|p| p.starts_with(&prefix))
    }

    fn label(&self, id: &String) -> String {
        id.rsplit('/').next().unwrap_or(id).to_string()
    }

    fn icon(&self, id: &String, _expanded: bool) -> Option<Icon> {
        Some(if self.has_children(id) { Icon::Folder } else { Icon::File })
    }
}

fn sample_project() -> Project {
    let files = [
        ("Cargo.toml", "[package]\nname = \"hello\"\nversion = \"0.1.0\"\n"),
        ("README.md", "# Hello\n\nAn example project.\n"),
        ("src/main.rs", "mod app;\n\nfn main() {\n    app::run();\n}\n"),
        ("src/app.rs", "pub fn run() {\n    println!(\"Hello from the app\");\n}\n"),
        ("src/ui/mod.rs", "pub mod theme;\n"),
        ("src/ui/theme.rs", "pub const ACCENT: &str = \"#5b8cff\";\n"),
    ];
    Project { files: files.iter().map(|(p, c)| (p.to_string(), c.to_string())).collect() }
}

pub struct Shell {
    pub project: Rc<Project>,
    pub tree: TreeState<String>,
    pub dock: DockSpace<Panel>,
    pub tools: ToolWindows<Tool>,
    pub layouts: Layouts<Workspace>,
    pub palette: CommandPalette,
    pub keymap: Keymap,
    pub keys: KeymapEditor,
    pub show_keys: bool,
    pub dark: bool,
    pub status: String,
}

#[derive(Clone, Debug)]
pub enum Msg {
    Dock(DockSpaceMsg),
    Tool(ToolMsg<Tool>),
    ToggleTool(Tool),
    Tree(TreeMsg<String>),
    Edit(String, String),
    Open(Panel),
    Palette(PaletteMsg),
    Keys(KeymapMsg),
    ShowKeys(bool),
    Layout(LayoutMsg),
    FocusPanel(usize),
    Save,
    ToggleTheme,
    ResetLayout,
}

impl Default for Shell {
    fn default() -> Self {
        let ws = default_workspace();
        let mut tree = TreeState::new("project-tree");
        tree.expand("src".to_string());
        Shell {
            project: Rc::new(sample_project()),
            tree,
            dock: ws.dock.clone(),
            tools: ws.tools.clone(),
            layouts: Layouts::new().with("Default", ws),
            palette: CommandPalette::new(),
            keymap: Keymap::new(),
            keys: KeymapEditor::new(),
            show_keys: false,
            dark: true,
            status: "Ready".into(),
        }
    }
}

impl App for Shell {
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
            Msg::Dock(m) => self.dock.update(m),
            Msg::Tool(m) => self.tools.update(m),
            Msg::ToggleTool(t) => self.tools.update(ToolMsg::Toggle(t)),
            Msg::Tree(m) => {
                let project = self.project.clone();
                if let Some(TreeEvent::Activated(path)) = self.tree.update(m, &*project, cx) {
                    if !project.has_children(&path) {
                        self.dock.open(Panel::Editor(path));
                    }
                }
            }
            Msg::Edit(path, text) => {
                Rc::make_mut(&mut self.project).files.insert(path, text);
                self.status = "Modified".into();
            }
            Msg::Open(p) => self.dock.open(p),
            Msg::Palette(m) => {
                if let Some(run) = self.palette.update(m, &self.commands()) {
                    self.update(run, cx);
                }
            }
            Msg::Keys(m) => self.keys.update(m, &mut self.keymap),
            Msg::ShowKeys(on) => self.show_keys = on,
            Msg::Layout(m) => {
                let now = Workspace { dock: self.dock.clone(), tools: self.tools.clone() };
                if let Some(ws) = self.layouts.update(m, &now) {
                    self.dock = ws.dock;
                    self.tools = ws.tools;
                }
            }
            Msg::FocusPanel(n) => {
                if let Some(id) = self.dock.focus_id(n) {
                    cx.focus(&id);
                }
            }
            Msg::Save => self.status = "Saved".into(),
            Msg::ToggleTheme => self.dark = !self.dark,
            Msg::ResetLayout => {
                let ws = default_workspace();
                self.dock = ws.dock;
                self.tools = ws.tools;
            }
        }
    }

    fn on_key(&self, k: &KeyEvent) -> Option<Msg> {
        // Escape hides auto-hide tool windows.
        self.tools.key(k).map(Msg::Tool)
    }

    fn commands(&self) -> Commands<Msg> {
        let mut list = vec![
            Command::new("file.save", "Save", Msg::Save).category("File").key("Mod+S"),
            Command::new("view.palette", "Command Palette…", Msg::Palette(PaletteMsg::Open))
                .category("View")
                .key("Mod+Shift+P")
                .key("F1"),
            Command::new("view.terminal", "Terminal", Msg::Open(Panel::Terminal)).category("View").key("Ctrl+`"),
            Command::new("view.project", "Project", Msg::ToggleTool(Tool::Project))
                .category("View")
                .key("Alt+1")
                .checked(self.tools.is_open(&Tool::Project)),
            Command::new("view.outline", "Outline", Msg::ToggleTool(Tool::Outline))
                .category("View")
                .key("Alt+7")
                .checked(self.tools.is_open(&Tool::Outline)),
            Command::new("view.darkTheme", "Dark Theme", Msg::ToggleTheme).category("View").checked(self.dark),
            Command::new("view.resetLayout", "Reset Layout", Msg::ResetLayout).category("View"),
            Command::new("prefs.keys", "Keyboard Shortcuts", Msg::ShowKeys(true))
                .category("Preferences")
                .key("Mod+K Mod+S"),
        ];
        for n in 1..=4 {
            list.push(
                Command::new(format!("view.focusPanel{n}"), format!("Focus Panel {n}"), Msg::FocusPanel(n - 1))
                    .category("View")
                    .key(&format!("Mod+{n}")),
            );
        }
        Commands::new(list).with_keymap(&self.keymap)
    }

    fn menu(&self) -> Vec<Menu<Msg>> {
        let c = self.commands();
        vec![
            Menu::new("File", vec![c.menu_item("file.save"), MenuItem::Separator, c.menu_item("prefs.keys")]),
            Menu::new(
                "View",
                vec![
                    c.menu_item("view.palette"),
                    MenuItem::Separator,
                    c.menu_item("view.project"),
                    c.menu_item("view.outline"),
                    c.menu_item("view.terminal"),
                    MenuItem::Separator,
                    c.menu_item("view.darkTheme"),
                ],
            ),
            Menu::new(
                "Window",
                vec![
                    MenuItem::submenu("Layouts", self.layouts.menu_items(&Msg::Layout)),
                    c.menu_item("view.resetLayout"),
                ],
            ),
        ]
    }

    fn view(&self) -> Element<Msg> {
        let th = theme();
        let bar = titlebar(
            env!("CARGO_PKG_NAME"),
            row()
                .pl(12.0)
                .gap(8.0)
                .items_center()
                .child(icon(Icon::Code).color(th.colors.accent))
                .child(menubar(self.menu())),
            row().pr(6.0).child(
                icon_button(if self.dark { Icon::Sun } else { Icon::Moon })
                    .tooltip("Toggle theme")
                    .on_click(Msg::ToggleTheme),
            ),
            window_info().maximized,
        );
        let dock = self.dock.view_with_icons(None, Panel::title, Panel::icon, |p| self.panel_view(p), Msg::Dock);
        let body = self.tools.view(dock, Tool::title, Tool::icon, |t| self.tool_view(t), Msg::Tool);
        let cmds = self.commands();
        let hint = match pending_chord() {
            Some(k) => format!("({k}) was pressed. Waiting for the second key…"),
            None => format!("{} for commands", cmds.key_label("view.palette").unwrap_or_default()),
        };
        let status = status_bar()
            .child(status_item(Some(Icon::GitBranch), "main"))
            .child(status_item(None, self.status.clone()))
            .child(spacer())
            .child(status_item(None, hint));
        col()
            .size_full()
            .child(bar)
            .child(body.grow(1.0))
            .child(status)
            .children(self.layouts.dialog(Msg::Layout))
            .children(self.show_keys.then(|| self.keys_dialog(&cmds)))
            .children(self.palette.view(&cmds, Msg::Palette))
    }

    fn windows(&self) -> Vec<WindowSpec<Msg>> {
        self.dock.windows(Panel::title, Msg::Dock)
    }

    fn window_view(&self, key: &str) -> Element<Msg> {
        self.dock.view_with_icons(Some(key), Panel::title, Panel::icon, |p| self.panel_view(p), Msg::Dock)
    }
}

impl Shell {
    fn panel_view(&self, p: &Panel) -> Element<Msg> {
        let th = theme();
        match p {
            Panel::Editor(path) => {
                let text = self.project.files.get(path).cloned().unwrap_or_default();
                let path = path.clone();
                text_area(text, move |t| Msg::Edit(path.clone(), t))
                    .mono()
                    .size_full()
                    .rounded(0.0)
                    .border(0.0, th.colors.border)
            }
            Panel::Terminal => col()
                .p(10.0)
                .mono()
                .font_size(12.0)
                .gap(2.0)
                .child(text("$ cargo run"))
                .child(text("   Compiling hello v0.1.0").color(th.colors.text_muted))
                .child(text("Hello from the app")),
            Panel::Problems => col().p(12.0).child(text("No problems").color(th.colors.text_muted)),
        }
    }

    fn tool_view(&self, t: &Tool) -> Element<Msg> {
        match t {
            Tool::Project => self.tree.view(&self.project, Msg::Tree).size_full(),
            Tool::Outline => col().p(12.0).gap(6.0).children(
                ["struct Shell", "enum Msg", "impl App for Shell", "fn main"].map(|s| text(s).mono().font_size(12.0)),
            ),
        }
    }

    fn keys_dialog(&self, cmds: &Commands<Msg>) -> Element<Msg> {
        modal(
            "Keyboard Shortcuts",
            self.keys.view(cmds, &self.keymap, Msg::Keys).w(640.0).h(420.0),
            vec![primary_button("Done").on_click(Msg::ShowKeys(false))],
            Msg::ShowKeys(false),
        )
    }
}

fn main() {
    let opts = WindowOptions::new(env!("CARGO_PKG_NAME")).size(1280.0, 800.0).min_size(720.0, 480.0).frameless(true);
    if let Err(e) = charis_ui::run(Shell::default(), opts) {
        eprintln!("error: {e}");
    }
}
