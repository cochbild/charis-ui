# rust-ui

A web-inspired, highly customizable desktop UI framework for Rust. It aims to look as good as
Electron / modern web apps while staying pure Rust, with no browser or webview.

![Showcase, dark theme](docs/showcase-dark.png)

**What you get**

- **Web-like looks out of the box.** CSS-style flexbox and grid layout (via [taffy]), per-corner
  radii, layered *blurred* box shadows, linear gradients, opacity, outlines/focus rings, and the
  [Inter] font bundled so text looks the same everywhere.
- **Customizability first.** Every widget is an ordinary `Element` you can restyle with chainable
  builder methods. All built-in widgets read their colors, radii and sizes from a `Theme`. You can swap
  or tweak the theme at runtime (dark, light, any accent).
- **IDE-grade panels.** `hsplit`/`vsplit` panes with draggable splitters, min/max sizes, fixed or
  proportional sizing, and animated slide-to-collapse, nested in any direction. Drag a splitter
  past half the minimum to collapse a pane (VS Code style). Double-click a splitter to reset it.
- **Drag-and-drop docking.** `Dock` lets users drag tabs between panel groups or onto an edge to
  split it left/right/top/bottom, with live drop previews.
- **Electron-style chrome.** Frameless windows with custom title bars, window controls,
  edge-resizing, menu bars with dropdowns, context menus, modals and tooltips.
- **Smooth.** CSS-like `transition`s for hover, press, focus and any style change. Smooth wheel
  scrolling with auto-hiding overlay scrollbars.
- **Testable.** A headless runtime drives apps without a window: simulate clicks, drags and typing,
  then assert on layout or save PNG screenshots. Every screenshot here was rendered that way.

| Light theme | Drag-and-drop docking |
|---|---|
| ![](docs/showcase-light.png) | ![](docs/dock-drag.png) |

![Menus and context menus](docs/menus.png)

## Quick start

```rust
use rust_ui::prelude::*;

struct Counter { n: i32 }

#[derive(Clone)]
enum Msg { Inc, Dec }

impl App for Counter {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _cx: &mut Cx) {
        match msg { Msg::Inc => self.n += 1, Msg::Dec => self.n -= 1 }
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
    rust_ui::run(Counter { n: 0 }, WindowOptions::new("Counter")).unwrap();
}
```

```sh
cargo run --release --example showcase   # the IDE shell in the screenshots
cargo run --release --example dock       # drag-and-drop docking
cargo run --release --example counter    # the minimal app above
```

On Linux you need the usual X11/Wayland runtime libraries (e.g. `libxkbcommon-x11`).

## Concepts

### The app model

Apps follow the Elm/React pattern: state lives in your struct, `view` describes the UI for the
current state, and user interaction produces messages handled by `update`. The runtime only
rebuilds when something changed. It keeps UI-only state (scroll offsets, splitter positions, text
cursors, animations) keyed by element identity, so your model stays clean.

Give elements stable identity with `.id("name")` or `.key(value)` when they can move around.

### Styling

Builder methods mirror CSS:

```rust
row()
    .items_center().gap(8.0).px(12.0).h(32.0)
    .bg(hex("#1e1f24"))
    .border(1.0, hex("#2f3138"))
    .rounded(8.0)
    .shadow(Shadow::new(0.0, 4.0, 12.0, 0.0, Color::BLACK.with_alpha(0.3)))
    .gradient(135.0, [(0.0, hex("#5b8cff")), (1.0, hex("#a371f7"))])
    .transition(0.15)                                  // animate state changes
    .hover(|s| s.bg(hex("#26282e")).translate(0.0, -1.0))
    .active(|s| s.bg(hex("#2c2e35")))
    .focus_style(|s| s.outline(2.0, 2.0, hex("#5b8cff")))
```

Supported: flex (direction, wrap, grow/shrink/basis, justify, align, gap), grid (fr/px/%/auto
tracks, spans), width/height/min/max, padding, margin (including `auto`), absolute and fixed
positioning with z-index (overlays escape clipping), overflow hidden and scroll, per-side borders,
per-corner radii, multiple blurred shadows, gradients, opacity, outlines, translate, cursors,
fonts (family, size, weight, italic, line height, letter spacing), text alignment, wrapping and
`…` ellipsis. Text properties inherit like CSS.

Use `.apply(fn)` and `.when(cond, fn)` for reusable style mixins.

### Themes

```rust
fn theme(&self) -> Theme {
    let base = if self.dark { Theme::dark() } else { Theme::light() };
    let mut t = base.with_accent(hex("#a371f7"));
    t.radius = 8.0;
    t.font_size = 14.0;
    t.colors.panel = hex("#101114");
    t
}
```

Widgets call `theme()` to pick up tokens, and so can your own components.

### Split panes

```rust
hsplit("main", vec![
    Pane::fixed(260.0, sidebar).min(170.0).max(520.0)
        .collapsible(true).collapsed(!self.sidebar_open),   // animated slide
    Pane::fill(vsplit("center", vec![
        Pane::fill(editor),
        Pane::fixed(220.0, terminal).collapsible(true).collapsed(!self.panel_open),
    ])),
    Pane::flex(0.5, inspector),                            // proportional share
])
.on_collapse(|pane, collapsed| Msg::Collapsed(pane, collapsed))  // drag-to-collapse
.on_resize(Msg::SaveLayout)                                      // persist sizes
```

### Docking

```rust
let dock = Dock::new(DockNode::hsplit(vec![
    (1.0, DockNode::tabs(vec![Panel::Explorer])),
    (3.0, DockNode::vsplit(vec![
        (3.0, DockNode::tabs(vec![Panel::Editor("main.rs"), Panel::Editor("lib.rs")])),
        (1.0, DockNode::tabs(vec![Panel::Terminal])),
    ])),
]));

// update: Msg::Dock(m) => self.dock.update(m)
// view:   self.dock.view("dock", |p| p.title(), |p| p.content(), Msg::Dock)
```

### Widgets

`button`, `primary_button`, `ghost_button`, `danger_button`, `icon_button`, `text_input`
(selection, word navigation, clipboard, password mode), `search_input`, `checkbox`, `switch`,
`slider`, `progress`, `segmented`, `tab_bar`, `tree_row`/`list_item`, `menu_bar`, `menu_panel`,
`context_menu`, `modal`, `backdrop`, `titlebar`, `window_controls`, `status_bar`/`status_item`,
`card`, `badge`, `tag`, `kbd`, `avatar`, `section_header`, `separator`, and `.tooltip(..)` on any
element.

Icons are crisp vectors. There's a built-in set, and any 24×24 SVG path works via
`Icon::svg("M…")`, so you can paste in Lucide or Feather icons. For fully custom drawing, use
`canvas(|cv, rect| …)`.

### Custom window chrome

```rust
rust_ui::run(app, WindowOptions::new("My App").frameless(true))
// in view:
titlebar(title, left_content, right_content, window_info().maximized)
```

Any element can become a drag area (`.window_drag_area()`) or a window button
(`.window_control(WindowControl::Close)`). Frameless windows stay resizable from their edges.

### Headless testing and screenshots

```rust
let mut h = Headless::new(MyApp::default(), 1280.0, 800.0, 2.0);
h.click(100.0, 40.0);
h.drag((260.0, 300.0), (400.0, 300.0), 8);
h.type_text("hello");
assert_eq!(h.rt.app.query, "hello");
h.save_png("shot.png")?;
```

## Architecture

| Layer | Crate / module |
|---|---|
| Declarative tree + builders | `element`, `widgets`, `dock` |
| Layout (flexbox/grid) | [taffy] |
| Text shaping and rasterization | [cosmic-text] + swash, Inter bundled |
| 2D rendering (anti-aliased paths, gradients, blurred shadows, clipping, layers) | [tiny-skia] (CPU) |
| Runtime: diffing retained state, events, hit-testing, focus, drag and drop, animations | `runtime` |
| Windowing | winit + softbuffer, clipboard via arboard |

The renderer is CPU-based and backend-agnostic. It draws only when something changed. A full
1440×900 IDE frame (about 570 elements) takes about 8 ms at 1× and about 20 ms at 2× (Retina) on
one CPU core (`cargo run --release --example showcase -- --bench`).

Set `RUI_PROFILE=1` to print per-frame build and paint timings.

## Status and roadmap

This is an early but working foundation. Known gaps and planned work:

- GPU backend (wgpu/vello) for 4K and high refresh rates, plus damage-region repainting
- Multi-line text editing, IME pre-edit display, rich text spans
- Virtualized lists for very large data sets
- Accessibility (AccessKit) and screen-reader support
- Multiple windows, native menus, file dialogs
- Serializable dock layouts and tab reordering within a group

## License

MIT OR Apache-2.0. The bundled Inter font is under the SIL Open Font License
(`assets/fonts/Inter-LICENSE.txt`).

[taffy]: https://github.com/DioxusLabs/taffy
[cosmic-text]: https://github.com/pop-os/cosmic-text
[tiny-skia]: https://github.com/RazrFalcon/tiny-skia
[Inter]: https://rsms.me/inter/
