# Windows and platforms

## Window options

```rust,no_run
use charis_ui::prelude::*;
# struct MyApp;
# impl App for MyApp {
#     type Msg = ();
#     fn update(&mut self, _: (), _: &mut Cx<()>) {}
#     fn view(&self) -> Element<()> { div() }
# }

let opts = WindowOptions::new("My App")
    .size(1280.0, 800.0)
    .min_size(640.0, 400.0)
    .frameless(true) // draw your own title bar
    .backdrop(Backdrop::Mica) // Windows 11 material behind the window
    .system_font(true); // the platform UI font instead of Inter
charis_ui::run(MyApp, opts).unwrap();
```

`run` opens the window and blocks until the app exits. It picks the GPU renderer when an
adapter is available (Vulkan, Metal, DX12 or GL) and falls back to the CPU renderer otherwise,
or when `CHARIS_RENDERER=cpu` is set.

## Custom title bars

With `frameless(true)` the app draws its own title bar. `titlebar(title, left, right, maximized)`
is a ready-made one, with a drag area and window buttons:

```rust
# use charis_ui::prelude::*;
# #[derive(Clone)] enum Msg {}
fn chrome(body: Element<Msg>) -> Element<Msg> {
    col()
        .size_full()
        .child(titlebar("My App", text("left"), text("right"), window_info().maximized))
        .child(body.grow(1.0))
}
# let _ = chrome(div());
```

To build your own, make any element a drag area with `.window_drag_area()`, or a window button
with `.window_control(WindowControl::Close)`. Frameless windows still resize from their edges.

`window_info()` tells the view about its window: `maximized`, `focused`, `fullscreen`,
`native_buttons` and `buttons_inset` (macOS traffic lights), `backdrop` and `scale`.

## Per platform

- **Windows.** Frameless windows keep Snap Layouts (hover the maximize button), the shadow and
  the rounded corners. `Backdrop::Mica`, `Acrylic` and `Tabbed` show the Windows 11 material
  through transparent parts of the UI; they need the GPU renderer.
- **macOS.** A frameless window keeps its traffic lights over the app's title bar; move them
  with `.traffic_lights(x, y)`. `cx.toggle_fullscreen()` enters a native full-screen Space.
  Text fields use the macOS editing keys, and the menu bar gets the standard Window items.
- **Linux.** winit negotiates server-side decorations and draws client-side ones on GNOME
  Wayland, themed to match the app. Frameless windows resize from their edges on X11 and
  Wayland. On Wayland, apps can't position windows or learn where they are, so a torn-out dock
  tab opens where the compositor puts it; dropping a tab onto another window still works.
- **Scaling.** Moving to a monitor with another scale re-lays out and re-rasterizes for it. At
  fractional scales, fills and borders snap to device pixels so 1 px lines stay sharp.

## Accessibility

With the `accessibility` feature (on by default), screen readers see the UI through AccessKit:
UI Automation on Windows, NSAccessibility on macOS and AT-SPI on Linux. Built-in widgets expose
their role, name, state and actions. For custom widgets, use the ARIA-style methods: `.role()`,
`.aria_label()`, `.aria_checked()`, `.aria_expanded()`, `.aria_value()` and so on.

The runtime also follows the OS preferences for reduced motion and high contrast. See
[Accessibility](accessibility.md) for all of it.

## IME

Text inputs support input methods for Chinese, Japanese, Korean and other languages: the text
being composed is shown inline and underlined, and the candidate window follows the caret.
The IME is on only while a text input has focus.

## Clipboard and file dialogs

- `cx.copy_to_clipboard(text)`, `cx.copy_image(image)` and `cx.read_clipboard(f)`; pasting an
  image into an element with `.on_paste_image(f)` hands you the image.
- `cx.open_file`, `cx.open_files`, `cx.pick_folder` and `cx.save_file` show native dialogs and
  answer with a message, so nothing blocks. The dialog is modal to the window that asked: the
  common item dialog on Windows, the system panels on macOS, and the XDG desktop portal on
  Linux, falling back to zenity:

```rust
# use charis_ui::prelude::*;
# use std::path::PathBuf;
# #[derive(Clone)] enum Msg { Open, Opened(Option<PathBuf>) }
# struct A;
# impl App for A {
#     type Msg = Msg;
#     fn view(&self) -> Element<Msg> { div() }
fn update(&mut self, msg: Msg, cx: &mut Cx<Msg>) {
    match msg {
        Msg::Open => cx.open_file(FileDialog::new().filter("Models", &["gguf"]), Msg::Opened),
        Msg::Opened(Some(_path)) => { /* load it */ }
        Msg::Opened(None) => {} // cancelled
    }
}
# }
```
