# Menus and commands

## Menus

`App::menu` declares the app's menus from state. Shortcuts work in every window, and the same
menus show as the macOS menu bar or in the window with `menubar()`.

```rust
use rust_ui::prelude::*;

struct Editor {
    wrap: bool,
}

#[derive(Clone)]
enum Msg {
    Open,
    ExportHtml,
    ToggleWrap,
}

impl App for Editor {
    type Msg = Msg;
    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        if let Msg::ToggleWrap = msg {
            self.wrap = !self.wrap;
        }
    }
    fn menu(&self) -> Vec<Menu<Msg>> {
        vec![
            Menu::new(
                "File",
                vec![
                    MenuItem::action("Open…", Msg::Open).shortcut("Mod+O"), // Cmd on macOS, Ctrl elsewhere
                    MenuItem::submenu("Export", vec![MenuItem::action("HTML", Msg::ExportHtml)]),
                ],
            ),
            Menu::new("View", vec![MenuItem::check("Word wrap", self.wrap, Msg::ToggleWrap).shortcut("Alt+Z")]),
        ]
    }
    fn view(&self) -> Element<Msg> {
        col().size_full().child(menubar(self.menu())).child(text("…"))
    }
}
```

- A focused text input keeps its own editing keys, so Ctrl+C in a text field copies instead of
  running the menu's Copy.
- On macOS the menus become the global menu bar with the standard app menu, and `menubar()`
  renders nothing. On Windows, `WindowOptions::native_menu(true)` opts into a native bar.
- `context_menu` and `.on_context_menu(…)` give any element a right-click menu.

## Commands

An app with many actions declares them as **commands**: an id, a title, a message and default
keys. Commands feed the menus, the command palette and the shortcuts editor, and users can
rebind them.

```rust
use rust_ui::prelude::*;

struct Editor {
    wrap: bool,
    keymap: Keymap,
}

#[derive(Clone)]
enum Msg {
    Save,
    ShowKeys,
    ToggleWrap,
}

impl App for Editor {
    type Msg = Msg;
    fn update(&mut self, _msg: Msg, _cx: &mut Cx<Msg>) {}
    fn view(&self) -> Element<Msg> {
        div()
    }
    fn commands(&self) -> Commands<Msg> {
        Commands::new(vec![
            Command::new("file.save", "Save", Msg::Save).category("File").key("Mod+S"),
            Command::new("prefs.keys", "Keyboard Shortcuts", Msg::ShowKeys).key("Mod+K Mod+S"), // a chord
            Command::new("view.wrap", "Word Wrap", Msg::ToggleWrap).key("Alt+Z").checked(self.wrap),
        ])
        .with_keymap(&self.keymap) // the user's own bindings win
    }
    fn menu(&self) -> Vec<Menu<Msg>> {
        let c = self.commands();
        // Title, key and checked state come from the command.
        vec![Menu::new("File", vec![c.menu_item("file.save"), c.menu_item("view.wrap")])]
    }
}
```

- **Keys** use `Mod` for Cmd on macOS and Ctrl elsewhere, and `Ctrl`, `Shift`, `Alt`, `Meta`
  for specific keys. Two strokes separated by a space form a chord; while the first is waiting,
  `commands::pending_chord()` returns it for a status-bar hint.
- **`CommandPalette`** is VS Code's Ctrl+Shift+P: it lists every command with its keys, filters
  as you type, and runs the one you pick.
- **`KeymapEditor`** is a Keyboard Shortcuts screen. It searches commands, records new bindings
  (chords too), resets or removes them, and warns about conflicts. It edits a `Keymap`, which
  you save with the `serde` feature.
- `App::on_key` sees any key nothing else handled, for app-wide keys that aren't commands.

`examples/dock.rs` has a palette and a shortcuts editor wired up.
