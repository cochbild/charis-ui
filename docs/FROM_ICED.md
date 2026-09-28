# Coming from iced

iced is where Charis's author started: the app worked, but it looked plain and was hard to
style. This page covers:
- why iced apps tend to look that way;
- how Charis addresses each cause;
- how iced concepts map to Charis;
- what iced still has that Charis doesn't.

## Why iced apps tend to look "off"

| iced (0.14) | Effect | Charis |
|---|---|---|
| Theme = `Palette` of a few base colors plus an auto-derived extended palette | Few tones to work with; everything looks flat and similar | 12-step OKLCH scales and semantic roles (surface, panel, elevated, border, text_muted…), plus knobs for accent, gray tint, radius, density and scaling |
| Styling via per-widget `style(|theme, status| Style { … })` closures | Every custom look is verbose, and hover/pressed states must be written by hand for each widget | CSS-like builders: `.bg()`, `.border()`, `.rounded()`, `.shadow()`, `.hover(|s| …)`, `.active(…)`, `.focus_style(…)`, with automatic transitions |
| Custom layout engine (rows, columns, containers) | Web-style layouts (wrap, grid, auto margins, absolute overlays) are awkward | Taffy flexbox and CSS grid, absolute and fixed positioning, z-index overlays that escape clipping |
| Limited transitions (a new Animation API in 0.14) | UIs feel static | Every visual style property transitions on hover, press, focus or change; `.animate_layout()` animates position and size |
| Default font and text rendering | Text looks thin or blurry next to browser apps | Bundled Inter font, DirectWrite-style contrast and gamma correction, pixel-snapped baselines |
| `pane_grid`: resizable, rearrangeable panes, no tabs | No IDE-style docking | Tabbed dock groups: drag tabs between groups or onto edges, reorder, maximize, save layouts; splits with animated collapse |
| Window decorations left to the OS; custom title bars take manual work | Can't get an Electron-style title bar | `titlebar()`, `window_controls()`, menu bars. On Windows these include native snap, Snap Layouts, shadow and rounded corners |
| No AccessKit upstream (the libcosmic fork has it) | Weak accessibility | AccessKit built in: roles, names, states and actions for every widget; ARIA-style methods for custom ones |

## Concept map

| iced | Charis |
|---|---|
| `iced::application(boot, update, view).run()` | `charis_ui::run(app, WindowOptions::new("Title"))` with `impl App for MyApp` |
| `fn update(&mut self, msg) -> Task<Message>` | `fn update(&mut self, msg, cx: &mut Cx)`; effects go through `cx` |
| `Task::perform(future, Msg::Done)` | `cx.spawn(future, Msg::Done)` (or `cx.spawn_blocking(\|\| …)` for blocking work); returns a `TaskHandle` you can `abort()` |
| `Task::run(stream, Msg::Item)` | `cx.run(stream, Msg::Item)` |
| `fn subscription(&self) -> Subscription<Message>` | `fn subscriptions(&self) -> Subscriptions<Msg>`: `.every(period, msg)`, `.every_if(…)`, `.on_resize(…)`, `.on_focus_change(…)`, `.on_close_request(…)` |
| `Subscription::run` with a channel from another thread | `cx.proxy()`: a `Proxy` any thread can send messages through |
| keyboard subscriptions (`keyboard::on_key_press`) | `App::on_key`, or commands with key bindings (`App::commands`) |
| `iced::daemon` and `window::open` | `App::windows()` returns `WindowSpec`s and `App::window_view(key)` draws each one; windows open and close as the list changes |
| `fn view(&self) -> Element<'_, Message>` | `fn view(&self) -> Element<Msg>` (owned; no lifetime) |
| `column![a, b].spacing(8).padding(12)` | `col().gap(8.0).p(12.0).child(a).child(b)` |
| `row![…].align_y(Center)` | `row().items_center().child(…)` |
| `container(x).center(Fill)` | `div().size_full().center().child(x)` |
| `Length::Fill` / `FillPortion(n)` | `.grow(1.0)` / `.flex1()` / `.grow(n)` |
| `button("Save").on_press(Msg::Save)` | `primary_button("Save").on_click(Msg::Save)` (also `button`, `ghost_button`, `danger_button`) |
| `text_input("placeholder", &v).on_input(Msg::Changed)` | `text_input(v.clone(), Msg::Changed).placeholder("placeholder")` |
| `checkbox("Label", checked).on_toggle(Msg::T)` | `checkbox("Label", checked).on_click(Msg::Toggle)` |
| `toggler(on)` | `switch(on).on_click(Msg::Toggle)` |
| `slider(0.0..=1.0, v, Msg::V)` | `slider(v, 0.0, 1.0).on_change(Msg::V)` |
| `scrollable(content)` | `content.scroll_y()` (also `scroll_x`, `scroll_both`) |
| `pane_grid` | `hsplit`/`vsplit` with `Pane`s, or `Dock` for tabbed docking |
| `tooltip(x, "tip", Position::Bottom)` | `x.tooltip("tip")` |
| `.style(|theme, status| …)` | `.bg(…).hover(|s| …).active(|s| …)`, or read tokens with `theme()` |
| `Theme::custom(name, palette)` | `Theme::from_config(ThemeConfig { accent, gray, radius, density, … })` |
| `Element::map` | `Element::map` |
| `Component` (deprecated in 0.13) or lifting all state into the app | `Component` trait with local `State`, `Event` and `Output`, or `stateful(key, view, update)`; state kept by the runtime |
| `lazy(deps, \|deps\| view)` (the closure can't borrow the app) | `lazy(key, deps, \|\| view)`: the closure can borrow `&self`, and layout is reused too |
| `canvas` / `Program` | `canvas(|cv, rect| …)` |

## What iced has that Charis doesn't

Async tasks, subscriptions, multiple windows, pick lists, combo boxes, radio buttons, images and
SVGs, markdown, tables and headless testing have all landed (see the concept map above and the
book). What's left:

1. **`qr_code`.** There's no QR code widget. `canvas` can draw one from a QR encoding crate's
   modules in a few lines.
2. **Time-travel debugging.** iced's developer tools can replay messages. Here, the element
   inspector (F12 in debug builds) shows the live tree, boxes and styles, but there's no
   message history.
3. **Hot reloading of code.** iced can hot-patch `view` code while the app runs. Here, styles
   reload live from a stylesheet (`WindowOptions::stylesheet`), but code changes need a rebuild.
