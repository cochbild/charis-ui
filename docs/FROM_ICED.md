# Coming from iced

iced is where this framework's author started: the app worked, but it looked plain and was hard to
style. This page covers:
- why iced apps tend to look that way;
- how this framework addresses each cause;
- how iced concepts map to this framework;
- what iced does well that we must match.

## Why iced apps tend to look "off"

| iced (0.14) | Effect | This framework |
|---|---|---|
| Theme = `Palette` of a few base colors plus an auto-derived extended palette | Few tones to work with; everything looks flat and similar | 12-step OKLCH scales and semantic roles (surface, panel, elevated, border, text_muted…), plus knobs for accent, gray tint, radius, density and scaling |
| Styling via per-widget `style(|theme, status| Style { … })` closures | Every custom look is verbose, and hover/pressed states must be written by hand for each widget | CSS-like builders: `.bg()`, `.border()`, `.rounded()`, `.shadow()`, `.hover(|s| …)`, `.active(…)`, `.focus_style(…)`, with automatic transitions |
| Custom layout engine (rows, columns, containers) | Web-style layouts (wrap, grid, auto margins, absolute overlays) are awkward | Taffy flexbox and CSS grid, absolute and fixed positioning, z-index overlays that escape clipping |
| Limited transitions (a new Animation API in 0.14) | UIs feel static | Every visual style property transitions on hover, press, focus or change; `.animate_layout()` animates position and size |
| Default font and text rendering | Text looks thin or blurry next to browser apps | Bundled Inter font, DirectWrite-style contrast and gamma correction, pixel-snapped baselines |
| `pane_grid`: resizable, rearrangeable panes, no tabs | No IDE-style docking | Tabbed dock groups: drag tabs between groups or onto edges, reorder, maximize, save layouts; splits with animated collapse |
| Window decorations left to the OS; custom title bars take manual work | Can't get an Electron-style title bar | `titlebar()`, `window_controls()`, menu bars. On Windows these include native snap, Snap Layouts, shadow and rounded corners |
| No AccessKit upstream (the libcosmic fork has it) | Weak accessibility | Planned (roadmap M3) |

## Concept map

| iced | This framework |
|---|---|
| `iced::application(boot, update, view).run()` | `rust_ui::run(app, WindowOptions::new("Title"))` with `impl App for MyApp` |
| `fn update(&mut self, msg) -> Task<Message>` | `fn update(&mut self, msg, cx: &mut Cx)` (see "Gaps" below for async tasks) |
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
| `canvas` / `Program` | `canvas(|cv, rect| …)` |

## Gaps: what iced has that we must match

iced does some things well that this framework doesn't do yet. These are on the roadmap:

1. **Async tasks.** In iced, `update` returns a `Task` that runs futures (HTTP, file I/O) and
   sends a message back when done. We need `cx.spawn(future)` or `Task` equivalents.
2. **Subscriptions.** iced has timers, window and keyboard event streams, and external channels.
   We need `App::subscriptions()` (at least time ticks, window events and a channel for
   background threads).
3. **Multiple windows.** iced has a daemon mode with several windows.
4. **Wider widget set:** pick lists, combo boxes, radio buttons, images and SVGs, markdown,
   `table`, and `qr_code`.
5. **Tooling:** time-travel debugging, hot reloading, and the `iced_test` headless testing
   crate.

Items 1 and 2 matter most for real apps and are scheduled right after text input (roadmap M2).
