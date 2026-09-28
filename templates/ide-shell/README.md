# {{project-name}}

An IDE-style desktop app built on [Charis](https://github.com/cochbild/charis-ui):

- a custom title bar with File / View / Window menus (the native menu bar on macOS);
- editor tabs in a dock: drag them to split, reorder, or tear them out into their own windows;
- a Project tool window with a file tree (Alt+1) and an Outline tool window (Alt+7);
- a command palette (Ctrl+Shift+P or F1), rebindable shortcuts (Ctrl+K Ctrl+S) and named layouts
  (Window ▸ Layouts);
- a status bar.

```sh
cargo run --release
```

Where to start in `src/main.rs`:

- `Panel` lists what can be docked, and `Shell::panel_view` draws each panel.
- `Tool` lists the side tool windows, and `Shell::tool_view` draws them.
- `Shell::commands` holds every action with its default keys. The menus, the palette and the
  shortcuts editor are built from it.
- `sample_project` is an in-memory project. Replace it with a walk of a real folder.

To keep the layout and key bindings between runs, enable Charis's `serde` feature and save
`dock`, `tools`, `layouts` and `keymap` on exit.
