# Charis

> A desktop UI framework for Rust that looks like a modern web app, with no browser inside.

![The showcase app in the dark theme](docs/showcase-dark.png)

## About

Charis is for building desktop apps in Rust that look as polished as Electron or web apps, while
staying native, fast and small. It borrows the good parts of the web (CSS-like styling, flexbox
and grid layout, themes, transitions) and adds what IDE-class apps need: docking panels that
tear out into their own windows, tool windows, command palettes and rebindable shortcuts.

It's for developers writing tools, editors, dashboards and other desktop apps who want a
professional look without shipping a web engine.

## Features

- **Looks good out of the box.** Rounded corners, soft shadows, gradients and smooth
  transitions, with the Inter font bundled so text looks the same on every machine.
- **Themes from a few settings.** Pick an accent color, a gray tint, corner roundness and
  density, and every color and size follows, in light, dark and high-contrast versions. Users
  can change them while the app runs.
- **Fully customizable.** Every widget can be restyled, globally or one at a time, and
  stylesheets reload live while you design.
- **IDE-style panels.** Split panes, drag-and-drop docking with tabs that tear out into their
  own windows, side tool windows, and saved layouts.
- **Menus and commands.** Menu bars (native on macOS), context menus, a command palette, and
  keyboard shortcuts users can rebind, including two-key chords.
- **Custom window chrome.** Frameless windows with your own title bar that still snap, resize
  and maximize like native windows, with Mica on Windows 11.
- **About 30 widgets.** Buttons, text inputs, a large-document editor, checkboxes, sliders,
  tabs, dialogs, tooltips, images, and lists, trees and tables that stay fast with 100,000
  rows.
- **Accessible.** Screen readers (Narrator, NVDA, VoiceOver, Orca), full keyboard navigation,
  input methods for Chinese, Japanese and Korean, high contrast, and reduced motion.
- **Fast.** GPU rendering by default, with a CPU renderer as a fallback. Typical frames take a
  few milliseconds.
- **Testable.** Apps run headless in tests: click, type and drag, then check the result or save
  a screenshot. Every screenshot on this page was made that way.

| Light theme | Drag-and-drop docking |
|---|---|
| ![The showcase app in the light theme](docs/showcase-light.png) | ![Dragging a tab to a new position](docs/dock-drag.png) |

| Tabs in their own windows | Tool windows |
|---|---|
| ![A tab dragged out into its own window](docs/dock-tear-out.png) | ![Pinned and auto-hide tool windows](docs/tool-windows.png) |

| Command palette | High contrast |
|---|---|
| ![The command palette](docs/command-palette.png) | ![Normal, high contrast dark, and high contrast light](docs/high-contrast.png) |

## Getting started

### Prerequisites

- Rust 1.89 or newer (`rustup update`).
- **Windows:** the MSVC toolchain and the Visual Studio Build Tools ("Desktop development with
  C++").
- **Linux:** the X11 and Wayland development libraries, for example on Ubuntu:
  `sudo apt install libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxi-dev libxrandr-dev`.
- **macOS:** the Xcode command-line tools.

### Try the examples

```sh
git clone https://github.com/cochbild/charis-ui.git
cd charis-ui
cargo run --release --example gallery    # every widget, one page each
cargo run --release --example showcase   # the IDE-style app in the screenshots
cargo run --release --example dock       # drag-and-drop docking
```

### Use it in your app

Charis isn't on crates.io yet, so add it from GitHub:

```toml
[dependencies]
charis-ui = { git = "https://github.com/cochbild/charis-ui" }
```

A complete app is a struct, a message type, and two methods:

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
        col().size_full().center().gap(12.0)
            .child(text(self.n.to_string()).font_size(48.0).bold())
            .child(row().gap(8.0)
                .child(button("−").on_click(Msg::Dec))
                .child(primary_button("+").on_click(Msg::Inc)))
    }
}

fn main() {
    charis_ui::run(Counter { n: 0 }, WindowOptions::new("Counter")).unwrap();
}
```

Or start from a template, an IDE-style shell or a settings app (see
[templates](templates/README.md)):

```sh
cargo install cargo-generate
cargo generate --git https://github.com/cochbild/charis-ui templates/ide-shell --name my-app
```

## Documentation

- **[The Charis Book](book/src/introduction.md)** is the guide: the app model, layout, styling,
  theming, widgets, docking, menus, windows, accessibility, testing and performance. To read it
  as a website, run `mdbook serve book`.
- **The API reference:** run `cargo doc --open`. Every public item is documented.
- **More:** [architecture](docs/ARCHITECTURE.md), [customizing](docs/CUSTOMIZING.md),
  [performance](docs/PERFORMANCE.md), [coming from iced](docs/FROM_ICED.md), the
  [roadmap](docs/ROADMAP.md), the [versioning policy](docs/SEMVER.md) and the
  [changelog](CHANGELOG.md).

## Status

Charis is a working 0.x framework heading for 1.0. It's tested on Windows 11 and on Linux, and
the API may still change between minor releases; the [changelog](CHANGELOG.md) lists every
change. The [roadmap](docs/ROADMAP.md) has the plan to 1.0.

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for how to build, test and
submit changes, and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for the community standards. For
questions, see [SUPPORT.md](SUPPORT.md). To report a security issue, see
[SECURITY.md](SECURITY.md).

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE), at your option. The
bundled Inter font is under the SIL Open Font License
([assets/fonts/Inter-LICENSE.txt](assets/fonts/Inter-LICENSE.txt)).

## Contact

**Dale Cochran** — [@cochbild](https://github.com/cochbild)
