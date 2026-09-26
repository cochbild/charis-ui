# Changelog

All notable changes to rust-ui are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project follows
[Semantic Versioning](https://semver.org/) as described in [docs/SEMVER.md](docs/SEMVER.md).

## [Unreleased]

### Breaking

- `#[non_exhaustive]` on enums that will grow: `Event`, `WindowRequest`, `ClipboardContent`,
  `ChromeHit`, `MouseButton`, `Key`, `WindowControl`, `Role`, `Icon`, `Cursor`, `FontFamily`,
  `Fit`, `ImageSource`, `Backdrop`, `Damage`, `DialogKind`, `Easing`, `Accent`, `GrayTint`,
  `ButtonKind`, `TreeEvent` and `DropZone`. A `match` on these needs a `_` arm.
- The `edit` module (text-editing helpers used by the text widgets) is now private, and the
  `cpu` module is hidden from the docs. Neither had documented uses outside the crate.

### Added

- `WindowOptions::resizable`.
- A versioning policy (`docs/SEMVER.md`), `CONTRIBUTING.md`, this changelog and a code of
  conduct.
- The book (`book/`), whose examples run as doc tests, and docs on every public item
  (`#![warn(missing_docs)]`).
- A widget gallery (`cargo run --example gallery`) and `cargo generate` templates for an IDE
  shell and a settings app (`templates/`).
- `Runtime::accessibility_update` and `Runtime::reset_accessibility`: incremental accessibility
  updates. The window now sends screen readers only the nodes that changed.
- `HeadlessApp::drag_to_window`, for testing drags between windows without screen positions.

### Changed

- `text_area` handles large documents: text is laid out by paragraph and only what's visible,
  at the caret or hit-tested is shaped. A 100k-line file opens in about 20 ms and edits in a few
  milliseconds per keystroke (it used to run out of memory).
- Undo history stores edits instead of copies of the text.
- A multi-line input follows its caret only when the caret moves, so the mouse wheel can scroll
  away from it.

### Fixed

- Bold, italic and links inside tight Markdown list items (`- **bold** item`) were dropped.
- Dropping a dock tab onto another window didn't work on Wayland.

## [0.1.0] - unreleased snapshot

This is the first version, developed ahead of a public release. The list below groups what it
contains.

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
- Async tasks (`cx.spawn`, `cx.spawn_blocking`, optional tokio), streams and subscriptions.

### Rendering

- A GPU renderer on wgpu (instanced SDF quads, glyph and image atlases), with automatic fallback
  to a CPU renderer on tiny-skia.
- Text correction (contrast and gamma) that matches between the two renderers.
- Images (PNG, JPEG, SVG through resvg) with object-fit, rounding and tinting.

### Widgets

- About 30 widgets: buttons, text inputs, `text_area` (undo/redo), checkboxes, switches,
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
