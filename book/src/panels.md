# Panels and docking

rust-ui has four layers of panel management. Use as many as the app needs:

| Type | What it does |
|---|---|
| `hsplit` / `vsplit` | Fixed arrangements of resizable panes (see [Layout](layout.md#splits)). |
| `Dock<T>` | Tab groups the user can drag, split, reorder, maximize and close. |
| `DockSpace<T>` | A `Dock` whose tabs can also be torn out into their own OS windows. |
| `ToolWindows<T>` | JetBrains-style side panels with icon stripes, pinned or auto-hide. |
| `Layouts<S>` | Named snapshots of any of the above ("Default", "Debug"…). |

All of them are plain values in your app state. `T` is your own panel type, usually an enum,
and each one comes with a message type you wrap in your app's `Msg`.

## A dock

```rust
use rust_ui::prelude::*;

#[derive(Clone, Debug, PartialEq)]
enum Panel {
    Explorer,
    Editor(&'static str),
    Terminal,
}

struct Ide {
    dock: Dock<Panel>,
}

#[derive(Clone)]
enum Msg {
    Dock(DockMsg),
}

impl App for Ide {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        match msg {
            Msg::Dock(m) => self.dock.update(m),
        }
    }

    fn view(&self) -> Element<Msg> {
        let title = |p: &Panel| match p {
            Panel::Explorer => "Explorer".to_string(),
            Panel::Editor(name) => name.to_string(),
            Panel::Terminal => "Terminal".to_string(),
        };
        let content = |p: &Panel| -> Element<Msg> { text(format!("{p:?}")).p(12.0) };
        self.dock.view("dock", title, content, Msg::Dock)
    }
}

let dock = Dock::new(DockNode::hsplit(vec![
    (1.0, DockNode::tabs(vec![Panel::Explorer])),
    (
        3.0,
        DockNode::vsplit(vec![
            (3.0, DockNode::tabs(vec![Panel::Editor("main.rs"), Panel::Editor("lib.rs")])),
            (1.0, DockNode::tabs(vec![Panel::Terminal])),
        ]),
    ),
]));
let mut h = Headless::new(Ide { dock }, 1000.0, 700.0, 1.0);
h.settle();
assert!(h.rt.rect_of_text("lib.rs").is_some());
```

What users can do with it:

- drag a tab onto another group to join it, or onto a group's edge to split it;
- aim with the **compass**, the cross of drop targets in the hovered group, or the guides at
  the dock's outer edges (turn it off with `dock.compass = false`);
- drag a tab along its strip to reorder it;
- double-click a tab (or use the header button) to maximize its group;
- resize every split; the proportions are written back into the tree;
- right-click a tab, or press Shift+F10, for Close, Close Others, Split Right / Down,
  Move to Edge, Open in New Window and Maximize.

Your code opens tabs with `dock.open(panel)`. To move keyboard focus into the n-th group (a
"Focus Panel 2" command, say), pass `dock.focus_id("dock", n)` to `cx.focus`.

## Tabs in their own windows

`DockSpace` wraps a `Dock` and adds floating windows. Hook it up in four places:

```rust
# use rust_ui::prelude::*;
use rust_ui::dock::{DockSpace, DockSpaceMsg};
# #[derive(Clone, Debug, PartialEq)] enum Panel { Editor, Terminal }
# fn title(p: &Panel) -> String { format!("{p:?}") }
# fn content(p: &Panel) -> Element<Msg> { text(title(p)) }

struct Ide {
    dock: DockSpace<Panel>,
}

#[derive(Clone)]
enum Msg {
    Dock(DockSpaceMsg),
}

impl App for Ide {
    type Msg = Msg;
    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        let Msg::Dock(m) = msg;
        self.dock.update(m);
    }
    fn view(&self) -> Element<Msg> {
        self.dock.view(None, title, content, Msg::Dock)
    }
    fn windows(&self) -> Vec<WindowSpec<Msg>> {
        self.dock.windows(title, Msg::Dock)
    }
    fn window_view(&self, key: &str) -> Element<Msg> {
        self.dock.view(Some(key), title, content, Msg::Dock)
    }
}
# let dock = DockSpace::new(Dock::new(DockNode::tabs(vec![Panel::Editor, Panel::Terminal])));
# let mut h = Headless::new(Ide { dock }, 800.0, 600.0, 1.0);
# h.settle();
```

Dragging a tab out of the window opens it in a new OS window where it was dropped; dragging it
onto a group in any window docks it there. The header buttons "Open in new window" and "Dock
back" do the same from the keyboard, and on Wayland, where windows can't learn their screen
position. Closing a floating window docks its tabs back.

## Tool windows

`ToolWindows` puts side panels around any content, often a dock. Each tool window lives on an
edge and is either *pinned* (it takes space, with a splitter) or *auto-hide* (it slides over the
content and hides on an outside click or Escape). One tool window per edge is open at a time.

```rust
use rust_ui::prelude::*;
use rust_ui::toolwin::{Side, ToolMode, ToolMsg, ToolWindows};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Tool {
    Files,
    Problems,
}

struct Shell {
    tools: ToolWindows<Tool>,
}

#[derive(Clone)]
enum Msg {
    Tool(ToolMsg<Tool>),
}

impl App for Shell {
    type Msg = Msg;
    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        let Msg::Tool(m) = msg;
        self.tools.update(m);
    }
    fn on_key(&self, k: &KeyEvent) -> Option<Msg> {
        self.tools.key(k).map(Msg::Tool) // Escape hides auto-hide panels
    }
    fn view(&self) -> Element<Msg> {
        let icon_of = |t: &Tool| match t {
            Tool::Files => Icon::Folder,
            Tool::Problems => Icon::Warning,
        };
        let center = text("Editor").p(12.0);
        self.tools.view(center, |t| format!("{t:?}"), icon_of, |t| text(format!("{t:?} panel")), Msg::Tool)
    }
}

let tools = ToolWindows::new()
    .add(Tool::Files, Side::Left, ToolMode::Pinned)
    .add(Tool::Problems, Side::Bottom, ToolMode::AutoHide);
let mut shell = Shell { tools };
shell.tools.show(&Tool::Files);
let mut h = Headless::new(shell, 900.0, 600.0, 1.0);
h.settle();
assert!(h.rt.rect_of_text("Files panel").is_some());
```

Right-clicking a stripe button or a tool window's header moves it to another edge, switches its
mode, or hides it. `tools.menu_items(&id, &map)` gives the same actions for your own menus.

## Workspaces

`Layouts<S>` stores named snapshots of any state `S`, usually the dock and tool windows together:

```rust,ignore
#[derive(Clone)]
struct Workspace {
    dock: DockSpace<Panel>,
    tools: ToolWindows<Tool>,
}

let layouts = Layouts::new().with("Default", default_ws()).with("Debug", debug_ws());

// update:
Msg::Layout(m) => {
    let now = Workspace { dock: self.dock.clone(), tools: self.tools.clone() };
    if let Some(ws) = self.layouts.update(m, &now) {
        self.dock = ws.dock;
        self.tools = ws.tools;
    }
}
// menu:  MenuItem::submenu("Layouts", self.layouts.menu_items(&Msg::Layout))
// view:  .children(self.layouts.dialog(Msg::Layout))   // the "Save Layout As" dialog
```

The menu lists the layouts with the current one checked, plus "Save Changes", "Save Layout
As…" and "Delete Layout". `examples/dock.rs` wires all of this up.

## Saving layouts

With the `serde` feature, `Dock`, `DockSpace`, `ToolWindows`, `Layouts` and `Keymap` implement
`Serialize` and `Deserialize`. Save them on exit and load them on start to restore the user's
arrangement, floating windows included. Serialized docks carry a `version` field so future
releases can migrate them.
