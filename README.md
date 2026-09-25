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

Themes are organized like modern web design systems (Radix Colors / shadcn/ui):

1. **Primitive scales**: 12-step OKLCH color scales generated from a seed color.
   Steps 1–2 are app backgrounds, 3–5 component states, 6–8 borders, 9–10 solid fills,
   and 11–12 text.
2. **Semantic roles**: `theme().colors.surface`, `.border`, `.text_muted`, `.accent`,
   `.focus_ring` and so on, derived from the scales. Widgets only use these roles.
3. **A few global knobs** that regenerate everything consistently:

```rust
fn theme(&self) -> Theme {
    Theme::from_config(ThemeConfig {
        accent: hex("#a371f7"),
        gray: GrayTint::Mauve,        // Zinc, Slate, Sand, Sage, Accent, Custom { hue, chroma }…
        radius: 8.0,                  // sm = ×0.6, lg = ×1.6
        scaling: 1.0,                 // text and control size multiplier
        density: Density::Compact,    // control, row and tab heights
        ..ThemeConfig::dark()
    })
}
```

Every token on the resulting `Theme` is public, so you can still override single values.
The raw scales are there for your own components: `theme().scales.accent.step(3)`.

Defaults follow web conventions:
- 13px UI text and a 4px spacing rhythm.
- Transitions of 150ms using the standard curve `cubic-bezier(.4,0,.2,1)`. Use
  `.easing(..)` for any CSS curve.
- Two-layer soft shadows.
- Translucent borders in dark mode.
- Focus rings shown only for keyboard focus (3px ring, like `:focus-visible`).
- Splitters highlight after a 300ms hover delay, as in VS Code.

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
| Display list | `scene`: each frame is recorded once, then handed to a backend |
| **GPU backend** (default) | `gpu`: wgpu. The whole frame is one instanced draw call, with SDF rounded rects and borders, analytic Gaussian shadows, and glyph/path atlases |
| CPU backend (fallback, tests) | `cpu`: tiny-skia with bounded scratch buffers for clipping and layers |
| Runtime: retained state, events, hit-testing, focus, drag and drop, animations | `runtime` |
| Windowing | winit, with wgpu surfaces or softbuffer; clipboard via arboard |

**Text quality.** Glyph coverage gets DirectWrite-style contrast enhancement and gamma correction,
the same approach Windows Terminal and Zed use to match browser and native text. Baselines are
snapped to the pixel grid. Both backends apply identical correction, and a test keeps the GPU
output matching the CPU reference.

**Choosing a renderer.** The GPU backend is used when an adapter is available (Vulkan, Metal, DX12
or GL). Otherwise the app falls back to the CPU backend. Set `RUI_RENDERER=cpu` to force the CPU,
or build without the `gpu` feature. Set `RUI_PROFILE=1` to print the chosen adapter and per-frame
timings.

**Performance.** On the CPU backend, a full 1440×900 IDE frame (about 600 elements) takes about
10 ms at 1× and about 20 ms at 2× on one core. On the GPU backend, the CPU side of a frame is layout
(about 5 ms) plus recording (about 0.4 ms), and the GPU draws everything in one call.

## Status and roadmap

This is an early but working foundation. Known gaps and planned work:

- Incremental layout (reuse taffy's cache between frames) and damage-region repainting on the
  CPU backend
- Multi-line text editing, IME pre-edit display, rich text spans
- Virtualized lists for very large data sets
- Accessibility (AccessKit) and screen-reader support
- Multiple windows, native menus, file dialogs
- Serializable dock layouts, tab reordering within a group, maximize/auto-hide panels,
  floating tabs in OS windows
- Hot-reloadable stylesheet layer

See [`docs/LANDSCAPE.md`](docs/LANDSCAPE.md) for the survey of existing Rust UI frameworks and
the design research behind these choices.

## License

MIT OR Apache-2.0. The bundled Inter font is under the SIL Open Font License
(`assets/fonts/Inter-LICENSE.txt`).

[taffy]: https://github.com/DioxusLabs/taffy
[cosmic-text]: https://github.com/pop-os/cosmic-text
[tiny-skia]: https://github.com/RazrFalcon/tiny-skia
[Inter]: https://rsms.me/inter/
