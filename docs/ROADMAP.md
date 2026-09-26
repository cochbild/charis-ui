# Road to a production-grade 1.0

The goal is a framework you can use in all of your own Rust projects, and one the public can adopt
for production desktop apps. It should offer windowing and panel features that other Rust UI
frameworks don't provide.

"Production grade" here means five concrete things:
1. It runs correctly on Windows, macOS and Linux (X11 and Wayland), on real GPUs and without one.
2. It is accessible (screen readers, keyboard navigation) and handles international text input
   (IME).
3. It stays fast with large UIs (thousands of rows, 4K displays, 120Hz or more).
4. It has a stable, documented API with semver guarantees.
5. Every release is verified automatically on every platform.

## Where we are (v0.1, September 2026)

**Done:**
- Declarative element tree with CSS-like styling and transitions.
- Taffy flexbox and grid layout.
- OKLCH design tokens.
- GPU renderer (wgpu), with a CPU fallback.
- Text correction (contrast and gamma).
- Resizable, collapsible and proportional splits.
- Docking: move tabs, split groups, reorder tabs, maximize, and save/restore layouts (serde).
- Frameless window chrome.
- Menus, context menus, modals, tooltips.
- About 20 widgets.
- Headless test harness, and 41 tests including a GPU-vs-CPU parity test.

**Missing for production:**
- Platforms other than Linux have never been tested.
- Accessibility has not been tried with a real screen reader yet.
- IME composition has only been tested by simulating the events (no real IME on Linux CI).
- Layout is fully rebuilt every frame.
- 34 `unwrap`/`expect` calls in library code.
- The crate name is taken.
- No CI and no API docs site.

## Decisions (September 2026)

- **Name:** to be decided before the first public release; *Prism* (`prism-ui`) and *Lumen*
  (`lumen-ui`) are the leading candidates.
- **License:** MIT OR Apache-2.0.
- **Platform priority:** Windows is hardened first.
- **CI comes last** because CI minutes are limited. Until then, the full test suite (including
  golden screenshots and GPU parity) runs locally with `cargo test`.

## Milestones

Each milestone ends with a tagged release, and each has exit criteria that can be verified.

### M0: Release engineering

- [ ] Final crate name, reserved on crates.io. `rust-ui`, `rui` and `rustui` are taken.
- [x] `LICENSE-MIT` and `LICENSE-APACHE` files. Still to do: `CONTRIBUTING.md`, `CHANGELOG.md`, code of conduct.
- [ ] CI with GitHub Actions (**deferred to the end**):
  - fmt, clippy (`-D warnings`), and tests on Linux, Windows and macOS;
  - a feature matrix: `--no-default-features`, `gpu`, `serde`;
  - a minimum supported Rust version (MSRV) job;
  - `cargo doc` with `-D warnings`.
- [ ] GPU parity test in CI using lavapipe (Linux) and WARP (Windows) (deferred along with CI).
- [x] Visual regression tests: golden PNG screenshots per theme, with a tolerance.
- [x] No panics in library code on user input: remove or justify every `unwrap`/`expect`, and
      handle GPU errors (device lost, surface lost) by recovering or falling back to the CPU.

**Exit:** green CI on 3 operating systems × the feature matrix, and a published 0.2 under the
final name.

### M1: Platform correctness

- [x] Windows (implemented; awaiting QA on real hardware, see `docs/WINDOWS_QA.md`):
  - DWM frameless window with snap layouts and Aero shake;
  - hit testing for the custom title bar (maximize-button hover shows the Snap Layouts flyout);
  - rounded corners and shadow on Windows 11;
  - dark or light system menus that follow the theme.
- [x] Windows: Mica/Acrylic/Tabbed backdrop (`WindowOptions::backdrop`) through a
      DirectComposition swapchain with premultiplied alpha, with a fallback to an opaque window
      without a GPU.
- [x] macOS: a transparent title bar with traffic lights placed by the app
      (`traffic_lights`) and re-placed after resizes; native full screen
      (`Cx::toggle_fullscreen`); Cmd editing keys and standard Window menu items; Retina
      scaling.
- [x] Linux: winit's xdg-decoration negotiation with Adwaita CSD fallback, themed from the app;
      fractional scaling with device-pixel snapping; frameless edge resizing on X11 and Wayland.
- [x] Per-monitor DPI changes: re-layout and re-raster, identical to a window opened at that
      scale (tested at 1×, 1.25×, 1.5×, 1.75× and 2×).
- [x] A system font per platform (Segoe UI Variable, SF Pro, the desktop's font on Linux) as an
      option (`WindowOptions::system_font`).

**Exit:** a manual QA checklist signed off on Windows 11, macOS 15 and Ubuntu (GNOME Wayland plus
X11). Screenshots go in the docs.

### M1.5: App plumbing (parity with iced)

Real apps need these before they need more widgets. iced has them; see `docs/FROM_ICED.md`.

- [x] Async tasks: `cx.spawn(future)` and `cx.spawn_blocking(fn)` that deliver a message when
      done, on a small runtime.
- [x] Subscriptions: `App::subscriptions()` for timers and intervals, window events, and a
      `Sender<Msg>` handle that background threads can use to push messages.
- [x] Widgets: pick list/dropdown, combo box, radio group (arrow keys), image (PNG, JPEG; object-fit,
      rounded, cached per device size, GPU atlas with a linear sampler), SVG (resvg, tinting),
      number input (range, step, decimals, steppers, keyboard, draft text). `on_focus_change`.
- [x] Native file dialogs: `cx.open_file / open_files / pick_folder / save_file`, answered as
      messages; rfd underneath (Windows common item dialog, macOS panels, XDG portal or zenity on
      Linux), modal to the asking window; scripted in tests with `set_dialog_responder`.
- [x] App menus (`App::menu`): declared once; shortcuts in every window (after focused inputs),
      `menubar()` in-window with submenus, native menu bar via muda on macOS (default, with the
      standard app menu) and Windows (opt-in). Linux has no native menu (GTK-free build).

**Exit:** one of your own apps (currently built with iced) ported with no loss of function.

### M2: Text and input

- [~] Multi-line text editor widget (`text_area`, with undo/redo; still to do: 100k-line
  documents):
  - selection, undo/redo, word wrap, scrolling;
  - large documents (100k lines) that only shape what is visible.
- [x] IME: show pre-edit (the text being composed) with its underline, and position the candidate
      window. IME is enabled only while a text input has focus; editing keys go to the IME while
      composing.
- [x] Rich text spans (bold, color, links) and selectable read-only text.
- [x] System clipboard on all platforms, including images: `cx.copy_image`, `cx.read_clipboard`
      (text or image), `on_paste_image` for Ctrl/Cmd+V with an image; arboard `image-data`.
- [x] Keyboard shortcut and command system: `App::commands()` with default keys and chords,
      a rebindable `Keymap` (serde), menus built from commands, a command palette, a Keyboard
      Shortcuts editor with a recorder and conflict warnings, `on_key_capture`.

**Exit:** type Chinese, Japanese and Korean through the system IME in the editor on all 3 operating
systems; a 100k-line file scrolls at 120fps.

### M3: Accessibility

- [x] AccessKit integration (`accessibility` feature, on by default):
  - roles, names, states and actions for every built-in widget, and ARIA-style `.role()`,
    `.aria_label()`, `.aria_checked()`… for custom ones;
  - the tree is rebuilt each frame while a screen reader is active (incremental updates later);
  - focus follows keyboard focus; click, focus, set value, increment, decrement, scroll and
    expand/collapse actions are supported.
- [x] High-contrast theme generated from the token system (`Contrast::High`, WCAG ratios tested
  for every accent, gray and mode); the OS reduced-motion setting is respected (movement snaps,
  fades stay); OS preferences detected on Windows, macOS and GNOME (`system_prefs()`).
- [x] Headless tests assert on the accessibility tree (`tests/accessibility.rs`).

**Exit:** the showcase is usable with NVDA (Windows), VoiceOver (macOS) and Orca (Linux).

### M4: Scale and performance

- [x] Incremental layout: the taffy tree persists between frames; only changed styles, text and
  child lists are marked dirty. An unchanged 700-element frame dropped from 12.6ms to 0.7ms
  (`examples/stress.rs`).
- [x] Text cache: text is shaped once per string and style and only re-wrapped for other widths;
  lookups don't allocate.
- [x] Components with their own state (`Component`, `stateful`): local events update local
  state; only chosen outputs reach the app; nesting; works inside `lazy`.
- [x] Memoized subtrees: `lazy(key, deps, || …)` skips view, tree-building and layout sync for
  unchanged parts; rebuilds automatically on theme, interaction or animation changes.
- [x] Virtualized list (`virtual_list`: variable heights, anchored scrolling, follow-end,
  scroll-to-item).
- [x] Virtualized table (`table`: sortable headers, resizable columns, selection).
- [x] Virtualized tree (`tree::TreeState` over a lazy `TreeModel`): only on-screen rows are
      built, and keyboard navigation scrolls the selection into view.
- [ ] Damage tracking on the CPU backend (repaint only dirty regions).
- [ ] Frame budget targets:
  - under 4 ms of CPU per frame for the showcase;
  - a 100k-row table at 120fps;
  - benchmarks tracked in CI.

**Exit:** benchmark suite published; no frame over 8 ms in the showcase on mid-range hardware.

### M5: Windowing and panels beyond other frameworks

- [x] Multiple windows: declared by `App::windows()` / `window_view`, one runtime per window
  around a shared app, `HeadlessApp` for tests. The system font scan happens once; later
  windows copy the database (about 1ms to create a window's runtime).
- [x] Drag a dock tab out into a floating OS window, and dock it back (`DockSpace`): tear-out at
  the drop point, cross-window drop targets with previews, pop-out / dock-back buttons, closing
  a floating window re-docks its tabs. Verified end to end under Xvfb with real OS windows.
- [x] Auto-hide (unpinned) panels that slide over the content, plus side "stripes" (JetBrains
      style): `ToolWindows` with pinned/auto-hide modes, one per edge, resizable, Escape and
      outside click hide, reduced-motion aware.
- [x] Panel commands: tab context menu (right-click / Shift+F10) with close others / all, split
      right / down, move to the dock's left / right / top / bottom edge, new window / dock back,
      maximize; tool window menu (move to an edge, pinned / auto-hide, hide); hide all auto-hide
      tool windows (Escape); keyboard resizing of splitters (arrows, Shift, Home / End).
      Menus are keyboard accessible (autofocus, arrows, Enter, Escape).
- [x] Focus panel N (a keymap command): `Dock::focus_id` / `DockSpace::focus_id` with `cx.focus`.
- [x] Split resize priorities (`Pane::priority`, Low/Normal/High) and remembered sizes for panes
      that come and go (`Pane::key`); squeezed panes resize from their size on screen.
- [x] Drop targets: edge zones plus a Visual Studio-style compass in the hovered group and
      guides at the dock's outer edges (`Dock::compass`, on by default).
- [x] Workspaces: named, saved layouts (`Layouts<S>`) with menu commands, a Save As dialog, and
      serde support.

**Exit:** a demo IDE with floating panels across two monitors that restores its exact layout after
a restart.

### M6: Developer experience and 1.0

- [ ] API review:
  - consistent naming;
  - `#[non_exhaustive]` where needed;
  - private internals;
  - a documented semver policy.
- [ ] A docs site with a book (guide, theming, layout, panels, testing) plus rustdoc with examples
      on every public item.
- [ ] Optional hot-reloadable stylesheet layer and an element inspector overlay.
- [ ] Widget gallery app and templates: a `cargo generate` starter for "IDE shell" and
      "settings app".
- [ ] Two real applications built on it: your projects are the proving ground.

**Exit:** 1.0 with a semver guarantee.

## Working agreements

- **Every feature ships with:** a headless test, an entry in the showcase or gallery, and docs.
- **Fidelity:** CPU and GPU output must stay within the parity tolerance, and golden screenshots
  change only on purpose.
- **Platform bugs:** found through QA on real hardware; this repo's container only covers Linux
  (with a software GPU driver).
