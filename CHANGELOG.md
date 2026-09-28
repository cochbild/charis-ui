# Changelog

All notable changes to Charis are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project follows
[Semantic Versioning](https://semver.org/) as described in [docs/SEMVER.md](docs/SEMVER.md).

## [Unreleased]

## [0.1.0] - 2026-09-28

The first public release. It started as an internal prototype called `rust-ui`. The list below groups what it contains.

### Core

- Declarative `Element` tree with CSS-like styling: flexbox and grid layout (taffy), per-corner
  radii, layered blurred shadows, gradients, opacity, outlines and focus rings.
- Elm-style `App` trait: `update`, `view`, `Cx` for effects, and subscriptions.
- CSS-style transitions for hover, press and focus, and smooth scrolling with overlay
  scrollbars.
- Themes built on OKLCH scales and semantic tokens, with presets (accent, gray tint, density,
  radius, scaling), high contrast, reduced motion, and OS preference detection.
- Style classes (`Theme::style_class`) for restyling any element or built-in widget.
- A hot-reloadable stylesheet layer (`stylesheet` module, `WindowOptions::stylesheet`).
- Components with their own state (`Component`, `stateful`) and memoized subtrees (`lazy`).
- Incremental layout, a text shaping cache, and CPU damage tracking.
- `#[non_exhaustive]` on enums that will grow; see `docs/SEMVER.md`.
- Async tasks (`cx.spawn`, `cx.spawn_blocking`, optional tokio), streams and subscriptions.

### Rendering

- A GPU renderer on wgpu (instanced SDF quads, glyph and image atlases), with automatic fallback
  to a CPU renderer on tiny-skia.
- Text correction (contrast and gamma) that matches between the two renderers.
- Images (PNG, JPEG, SVG through resvg) with object-fit, rounding and tinting.

### Widgets

- About 30 widgets: buttons, text inputs, `text_area` (undo/redo, fast on documents of
  100,000 lines or more), checkboxes, switches,
  sliders, radio groups, number inputs, `pick_list`, `combo_box`, progress bars, tabs, tooltips,
  modals, menus and context menus, markdown, rich selectable text, images.
- Virtualized `virtual_list`, `table` (sortable, resizable columns, selection) and `tree`.

### Windows and panels

- Frameless windows with a custom title bar and window controls; native Windows chrome (snap,
  Snap Layouts, shadow, rounded corners), Mica/Acrylic backdrops, macOS traffic lights and full
  screen, Linux client-side decorations.
- Multiple windows (`App::windows`).
- Resizable, collapsible splits with priorities and remembered sizes.
- Docking (`Dock`, `DockSpace`): tab reordering, splits, maximize, a drop compass, tear-out into
  OS windows and back, panel commands, serde-persisted layouts, and named workspaces
  (`Layouts`).
- JetBrains-style tool windows with pinned and auto-hide modes.
- App menus declared once, shown in-window or as native macOS/Windows menu bars.
- Commands with a rebindable keymap, chords, a command palette and a shortcuts editor.
- Native file dialogs, and a system clipboard for text and images.

### Accessibility and input

- AccessKit integration: roles, names, states and actions for every built-in widget.
- IME composition with inline pre-edit and candidate window placement.
- Keyboard access to every built-in widget.

### Tooling

- `Headless` test harness with PNG screenshots, golden screenshot tests and a GPU parity test.
- An element inspector (F12 in debug builds).
- Frame budget benchmarks (`cargo bench --bench frames`).
- `CHARIS_PROFILE=1` prints the renderer, GPU and backend, and per-frame timings;
  `WGPU_ADAPTER_NAME` picks the GPU adapter.

### Documentation

- The Charis Book (`book/`), whose examples run as doc tests, and docs on every public item.
- Examples, including a widget gallery, and `cargo generate` templates for an IDE shell and a
  settings app (`templates/`).
- A versioning policy, contributing guide, code of conduct, and manual QA checklists
  (`docs/qa/`).
