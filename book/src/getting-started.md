# Getting started

## Add the dependency

```toml
[dependencies]
charis-ui = { git = "https://github.com/cochbild/charis-ui" }
```

On Linux, winit needs the usual X11 and Wayland libraries at build time:

```sh
sudo apt install libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxi-dev libxrandr-dev
```

## A first app

An app is a struct that implements `App`. It has two required methods:

- `view` describes the UI for the current state;
- `update` changes the state when a message arrives.

```rust
use charis_ui::prelude::*;

struct Counter {
    n: i32,
}

#[derive(Clone)]
enum Msg {
    Inc,
    Dec,
}

impl App for Counter {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        match msg {
            Msg::Inc => self.n += 1,
            Msg::Dec => self.n -= 1,
        }
    }

    fn view(&self) -> Element<Msg> {
        col()
            .size_full()
            .center()
            .gap(12.0)
            .child(text(self.n.to_string()).font_size(48.0).bold())
            .child(
                row()
                    .gap(8.0)
                    .child(button("−").on_click(Msg::Dec))
                    .child(primary_button("+").on_click(Msg::Inc)),
            )
    }
}

fn main() {
#   if false {
    charis_ui::run(Counter { n: 0 }, WindowOptions::new("Counter")).unwrap();
#   }
}
```

Run it with `cargo run`. Clicking `+` sends `Msg::Inc`; the runtime calls `update`, then `view`
again, and redraws what changed.

`WindowOptions` sets the title, size, minimum size, icon, fonts and chrome. For example:

```rust
# use charis_ui::prelude::*;
let opts = WindowOptions::new("Editor").size(1280.0, 800.0).min_size(640.0, 400.0).frameless(true);
```

## Cargo features

Everything is on by default except `serde` and `tokio`:

| Feature | What it adds |
|---|---|
| `window` | Opening real windows (winit, softbuffer). Without it you still have the headless runtime. |
| `gpu` | The wgpu renderer. Without it, windows use the CPU renderer. |
| `bundled-fonts` | Inter, bundled, so text looks the same everywhere. |
| `clipboard` | The system clipboard (arboard). |
| `markdown` | The `markdown()` element. |
| `accessibility` | Screen reader support through AccessKit. |
| `dialogs` | Native file dialogs (rfd). |
| `native-menu` | Native menu bars on macOS and (opt-in) Windows. |
| `svg`, `jpeg` | SVG images (resvg) and JPEG decoding. PNG is always supported. |
| `serde` | Serialize docks, tool windows, workspaces and keymaps. |
| `tokio` | Run `cx.spawn` futures on tokio, for tokio-based crates such as reqwest. |

## Starting from a template

Two [cargo-generate](https://github.com/cargo-generate/cargo-generate) templates give you a
working app to change:

```sh
cargo generate --git https://github.com/cochbild/charis-ui templates/ide-shell --name my-ide
cargo generate --git https://github.com/cochbild/charis-ui templates/settings-app --name my-settings
```

- **ide-shell:** a frameless window with menus, a dock of editor tabs, tool windows, a file
  tree, a command palette, rebindable shortcuts, named layouts and a status bar.
- **settings-app:** a searchable sidebar of sections, forms, a live theme, and settings saved to
  the user's config directory.

## The examples

The repository has examples that show most features:

```sh
cargo run --release --example gallery    # every widget, one page each
cargo run --release --example showcase   # an IDE shell with everything wired up
cargo run --release --example dock       # docking, tool windows, commands, workspaces
cargo run --release --example counter    # the app above
```

## Debugging aids

- **Inspector.** In debug builds, press F12 (or Ctrl+Shift+I, or Cmd+Alt+I on macOS) to outline
  elements under the pointer. Click one to pin a panel with its id, classes, box and style.
- **`CHARIS_PROFILE=1`** prints the GPU adapter and per-frame timings.
- **`CHARIS_RENDERER=cpu`** forces the CPU renderer.
