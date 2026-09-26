# Project templates

Starters for [cargo-generate](https://github.com/cargo-generate/cargo-generate):

| Template | What you get |
|---|---|
| `ide-shell` | A frameless IDE-style window: menus, a dock of editor tabs, tool windows, a file tree, a command palette, rebindable shortcuts, named layouts, a status bar. |
| `settings-app` | A settings window: a searchable sidebar, forms, a live theme, settings saved to the user's config directory. |

```sh
cargo install cargo-generate
cargo generate --git https://github.com/cochbild/rust-ui templates/ide-shell --name my-ide
cargo generate --git https://github.com/cochbild/rust-ui templates/settings-app --name my-settings
```

Both templates are compiled and tested with the crate (`tests/templates.rs`), so they stay
in step with the API.
