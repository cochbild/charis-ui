# Home test plan: Windows 11 and WSL

Everything that needs a real machine, in one list. Some of it has never run outside the Linux
build container, so expect some failures; those are what this plan is for.

- **Part 1: Windows 11** (native): the platform code that was only type-checked so far.
- **Part 2: WSL** (Ubuntu under WSLg): Linux on real Wayland and X11 (XWayland) sessions, with
  a GPU.

Every item has an ID (`W-…` or `L-…`). To report a failure, give the ID, what happened, a
screenshot if it's visual, and the lines `CHARIS_PROFILE=1` prints at startup (renderer and GPU).
Items marked **(new)** cover code that has never run on a real machine.

---

## Part 1: Windows 11

### Setup

1. Install Rust with the MSVC toolchain (`rustup-init.exe`, default host
   `x86_64-pc-windows-msvc`) and Visual Studio Build Tools with "Desktop development with C++".
2. Get the branch:
   ```powershell
   git clone https://github.com/cochbild/charis-ui
   cd charis-ui
   git checkout claude/rust-ui-framework-u88b01
   ```
3. Optional: `cargo install cargo-generate mdbook` (cargo-generate 0.23 or newer; its latest
   release needs Rust 1.96, otherwise add `--version 0.23.5`), and NVDA (free) for the screen
   reader checks.

### W-A. Build and automated checks

- [ ] **W-A1** `cargo build --release --all-features --examples` succeeds (the first real MSVC
      build: linking, `windows` crate features, muda, rfd).
- [ ] **W-A2** `cargo test --all-features` passes, including:
  - `golden` (screenshots; the bundled font is used, so they should match);
  - `gpu` (GPU-vs-CPU parity on your real GPU);
  - the book's examples (doc tests);
  - `templates`.

  Paste the output of any failure.
- [ ] **W-A3** `$env:WGPU_ADAPTER_NAME="Microsoft Basic Render Driver"; cargo test --all-features --test gpu`
      runs the parity test on WARP, the software GPU. Remove the variable afterwards.
- [ ] **W-A4** `cargo test --no-default-features --lib` passes.
- [ ] **W-A5** `cargo clippy --all-targets --all-features -- -D warnings` is clean.
- [ ] **W-A6** `cargo doc --no-deps --all-features --open` builds, and the docs open.
- [ ] **W-A7** `cargo bench --bench frames` runs. Paste the table; it's the first run on a
      desktop CPU.

### W-B. Every example starts

Run each with `$env:CHARIS_PROFILE=1` and check that it opens, draws and closes cleanly.

- [ ] **W-B1** `cargo run --release --example counter`: + and − work.
- [ ] **W-B2** `cargo run --release --example gallery`: every page in the sidebar opens.
- [ ] **W-B3** `cargo run --release --example showcase`.
- [ ] **W-B4** `cargo run --release --example dock`.
- [ ] **W-B5** `cargo run --release --example multiwindow`.
- [ ] **W-B6** `cargo run --release --example lmfast_chat`.
- [ ] **W-B7** `cargo run --release --example editor` **(new)**.
- [ ] **W-B8** `cargo run --release --example stress` prints frame times.
- [ ] **W-B9** The renderer line says it uses your GPU through DX12 or Vulkan, not a fallback.

### W-C. Frameless window chrome (`dock` example)

- [ ] **W-C1** Drop shadow and rounded corners; no white border or title strip.
- [ ] **W-C2** Dragging empty title-bar space moves the window.
- [ ] **W-C3** Dragging to a screen edge or corner snaps it (Aero Snap), and shaking it minimizes
      the others.
- [ ] **W-C4** Double-clicking the title bar maximizes; again restores.
- [ ] **W-C5** Hovering the maximize button shows the Snap Layouts flyout, and picking a layout
      works.
- [ ] **W-C6** Minimize, maximize and close buttons have hover states and work.
- [ ] **W-C7** Right-clicking the title bar, or pressing Alt+Space, opens the system menu, and its
      items work.
- [ ] **W-C8** The menus in the title bar and the theme button are clickable, not draggable.
- [ ] **W-C9** All edges and corners resize the window, with the right cursors.
- [ ] **W-C10** Maximized, nothing is cut off, and the title bar is at the very top.
- [ ] **W-C11** Toggling the theme (the sun/moon button) switches the system menu between light
      and dark.
- [ ] **W-C12** Windows+Up, Windows+Down, Windows+Left and Windows+Right behave like any other
      window.

### W-D. Rendering, scaling and the CPU renderer

- [ ] **W-D1** Text is crisp at 100 %, 125 %, 150 % and 200 % display scaling (Settings ▸
      Display ▸ Scale). Change it while the app runs.
- [ ] **W-D2** With two monitors at different scales, moving the window across keeps it sharp,
      at the same size.
- [ ] **W-D3** `$env:CHARIS_RENDERER="cpu"`: the dock and gallery look the same as on the GPU.
- [ ] **W-D4 (new)** With the CPU renderer, hover over lists, type, scroll, and open and close
      menus. No stale pixels, trails or smears are left behind (damage tracking with
      `present_with_damage`).
- [ ] **W-D5** Same as W-D4 with `$env:CHARIS_NO_DAMAGE=1` (full redraws). Both look identical.
- [ ] **W-D6** Resizing the window quickly doesn't flicker or show garbage, on both renderers.
- [ ] **W-D7** Showcase at a high refresh rate (120 Hz or more, if you have it): smooth scrolling
      and hover transitions look smooth. `CHARIS_PROFILE=1` frame times stay under 8 ms.

### W-E. Keyboard, text and IME

- [ ] **W-E1** In a text field: Ctrl+C, Ctrl+V, Ctrl+X, Ctrl+A, Ctrl+Z, Ctrl+Y, Ctrl+←/→,
      Ctrl+Backspace, Shift+arrows, Home and End.
- [ ] **W-E2** Microsoft Pinyin (or the Japanese IME): the text being composed shows inside the
      field, underlined. The candidate window opens next to the caret, and Enter or Space commits
      the text.
- [ ] **W-E3** IME in a multi-line text area (gallery ▸ Text input): same as W-E2, and the
      candidates follow the caret on later lines.
- [ ] **W-E4** With focus on a button (not a text field), single-letter app shortcuts aren't
      swallowed by the IME.
- [ ] **W-E5** Wheel and touchpad scrolling are smooth. Two-finger horizontal scrolling works in
      the table.
- [ ] **W-E6** Tab and Shift+Tab move focus through the gallery's widgets, with a visible focus
      ring. Space and Enter activate them.

### W-F. Large documents (new)

- [ ] **W-F1** `cargo run --release --example editor -- --lines 100000` opens in well under a
      second.
- [ ] **W-F2** Wheel-scroll from top to bottom: smooth, with no jumps or blank areas. The
      scrollbar thumb settles as you go.
- [ ] **W-F3** Ctrl+End (or Ctrl+A then →) jumps to the end; type there. The caret stays visible
      and typing feels instant.
- [ ] **W-F4** Hold ↓ (key repeat) for a few seconds: the view follows smoothly.
- [ ] **W-F5** Select a few hundred lines with Shift+↓ and delete them. Ctrl+Z brings them back
      exactly; Ctrl+Y deletes them again.
- [ ] **W-F6** Scroll far away from the caret with the wheel: the view stays there (it doesn't
      snap back) until you type or move the caret.
- [ ] **W-F7** Click in the middle of a line after scrolling: the caret lands where you clicked.
- [ ] **W-F8** Open a real large file: `cargo run --release --example editor -- <path>` (a big
      log or source file). Long wrapped lines scroll and edit correctly.
- [ ] **W-F9** Paste a large block (a few MB) from Notepad: it pastes in about a second or less.
      Undo removes it in one step.

### W-G. Gallery walkthrough (`gallery` example)

- [ ] **W-G1** Buttons: every variant, the icon buttons' tooltips, and the counter.
- [ ] **W-G2** Text input: name, search, password (masked), text area and number input. Number
      input: the − and + buttons, ↑/↓ keys, typing a value, and a value outside 0–99 is clamped
      on Enter.
- [ ] **W-G3** Selection:
  - the checkbox and switch;
  - the radio group (arrow keys move the selection);
  - pick list and combo box (typing filters);
  - segmented control, tabs, and the slider (drag and arrow keys).
- [ ] **W-G4** Lists, tables, trees:
  - the table: sort by each column, resize a column by dragging its divider (double-click
    resets it), select a row;
  - the tree: keyboard navigation and type-ahead;
  - the 100k-row virtual list scrolls smoothly.
- [ ] **W-G5** Status and feedback: progress bars, badges, tags, keys, avatars and the status bar
      draw correctly.
- [ ] **W-G6** Menus and dialogs:
  - the menu bar with a submenu;
  - the right-click context menu (and Shift+F10);
  - the dialog: Escape and clicking the backdrop close it;
  - the tooltip on the Discard button.
- [ ] **W-G7** Images and icons: Cover, Contain and Fill look right, with rounded corners, and
      the SVGs are sharp and tinted.
- [ ] **W-G8 (new)** Clipboard images: "Copy image", then paste into Paint (Ctrl+V): the photo
      appears. Then take a screenshot (Windows+Shift+S), click the paste box and press Ctrl+V:
      it shows the screenshot. "Read clipboard" reports text or image correctly.
- [ ] **W-G9** Typography: the sizes, weights, italic and mono text, rich text with a link, text
      you can select and copy, the ellipsis, and Markdown.
- [ ] **W-G10** Theme: accent swatches, dark mode, high contrast, radius and density all apply
      live.

### W-H. Customization and developer tools

- [ ] **W-H1 (new)** `cargo run --release --example dock -- --stylesheet examples/dock.css`, then
      edit `examples/dock.css` and save (for example, change `.button { radius: 999; }` to `0`).
      The app restyles within a second. A typo prints an error with its line number, and the
      last good style stays.
- [ ] **W-H2 (new)** Inspector (debug build: `cargo run --example dock`, without `--release`):
  - F12 (or Ctrl+Shift+I) shows outlines under the pointer;
  - clicking pins the details panel (id, classes, box, style);
  - F12 again closes it.
- [ ] **W-H3** `-- --system-font`: text uses Segoe UI Variable.

### W-I. Accessibility (Narrator: Ctrl+Windows+Enter; NVDA if installed)

- [ ] **W-I1** Narrator reads the window. Tab moves between controls and announces each one's name
      and type ("Save, button"; "Remember me, check box, not checked").
- [ ] **W-I2** Space or Enter (or Narrator's Caps Lock+Enter) activates the control, and the new
      state is announced.
- [ ] **W-I3 (new: incremental updates)** In the gallery, toggle the checkbox and switch, move the
      slider, and type in a field. Narrator announces each change promptly: the updates now send
      only what changed.
- [ ] **W-I4 (new)** Open and close the gallery's dialog. Narrator announces the dialog and its
      buttons, and focus returns afterwards. Nothing stale remains (scan mode can't reach the
      closed dialog).
- [ ] **W-I5 (new)** Switch gallery pages several times. Scan mode (Caps Lock+Space) reads the
      new page, not the old one.
- [ ] **W-I6** In the table, tree and virtual list, arrow keys announce the rows.
- [ ] **W-I7** With the 100k-line editor open and Narrator on, typing doesn't lag noticeably.
      Report it if it does: the text area's whole value is sent on each change.
- [ ] **W-I8** Optional: Accessibility Insights for Windows shows the tree with no unnamed
      buttons.

### W-J. High contrast and reduced motion

- [ ] **W-J1** Settings ▸ Accessibility ▸ Contrast themes ▸ Aquatic, then refocus the gallery.
      It switches to high contrast: near-black surfaces, opaque borders, no shadows.
- [ ] **W-J2** Settings ▸ Accessibility ▸ Visual effects ▸ Animation effects off, then refocus.
      Scrolling jumps instead of gliding and the sidebar collapses instantly; color fades still
      fade.
- [ ] **W-J3** Settings ▸ Personalization ▸ Colors ▸ Light or Dark: the settings-app template in
      "System" mode follows it (see W-O).

### W-K. Multiple windows and tab tear-out (`dock` example)

- [ ] **W-K1** Drag the Terminal tab out of the window: a new window opens where you release it,
      with a shadow, and it snaps like a normal window.
- [ ] **W-K2** Drag that tab back onto a panel in the main window: the edge or center preview
      shows while hovering, and it docks there on release. The empty window closes.
- [ ] **W-K3** "Open in new window" (↗ in a panel header) and "Dock back" work. Closing a
      floating window with its ✕ puts its tabs back.
- [ ] **W-K4** On a second monitor at a different scale: tear a tab out onto it. It's sharp and
      the right size. Drag it back.
- [ ] **W-K5** The multiwindow example: inspector windows open, share the count, and close
      independently.
- [ ] **W-K6 (new)** `cargo run --release --example dock -- --native-menu`: a native Win32 menu
      bar appears. Its items and shortcuts work, and it follows the dark or light theme.

### W-L. Menus, commands, tool windows and layouts (`dock` example)

- [ ] **W-L1** File, View and Window open on click. The "Open Panel" submenu opens to the side.
      Clicking outside closes the menu.
- [ ] **W-L2** Ctrl+Shift+T toggles the theme and Ctrl+N opens an editor, even with focus in a
      text box.
- [ ] **W-L3** Ctrl+Shift+P (or F1) opens the palette: it filters, ↑/↓ and Enter work, and
      Escape closes it.
- [ ] **W-L4** Ctrl+K then Ctrl+S: the status bar shows the waiting chord, then Keyboard
      Shortcuts opens. Rebind a command; the new key works and the menu shows it.
- [ ] **W-L5** Ctrl+1 … Ctrl+6 focus panels. Alt+1/2/4/6/9 toggle tool windows.
- [ ] **W-L6** Tool windows: pinned ones push the dock, auto-hide ones slide over it and hide on
      an outside click or Escape. The pin button switches modes, and the inner edge resizes.
- [ ] **W-L7** Right-click a tab: Split Right and Move to Edge ▸ Bottom work. Shift+F10 opens the
      same menu from the keyboard.
- [ ] **W-L8** While dragging a tab, the compass appears in the hovered group and the guides at
      the dock edges work.
- [ ] **W-L9** Tab to a splitter; the arrow keys resize it.
- [ ] **W-L10** Window ▸ Layouts: switching layouts works; "Save Layout As…" saves with Enter and
      cancels with Escape.
- [ ] **W-L11** With Todo pinned (Alt+6), make the window very narrow. Todo shrinks last, and gets
      its width back when you widen the window.

### W-M. Backdrop, full screen and the large tree (`dock` example)

- [ ] **W-M1** `-- --mica`: the title bar and stripes show Mica (the wallpaper tint), and the
      panels stay opaque. With `$env:CHARIS_RENDERER="cpu"` it opens normally, without Mica.
- [ ] **W-M2** F11 toggles full screen; the edges don't resize in full screen.
- [ ] **W-M3** Alt+1, then expand "generated (100000 files)": wheel, ↓, PageDown and End are
      smooth. Double-clicking a file opens it.

### W-N. File dialogs (`lmfast_chat` example)

- [ ] **W-N1** "Document" opens the Windows file picker, modal to the window, with its filter.
      Picking two files shows two chips; Cancel changes nothing.

### W-O. Templates (new)

- [ ] **W-O1** `cargo generate --git https://github.com/cochbild/charis-ui --branch
      claude/rust-ui-framework-u88b01 templates/ide-shell --name my-ide`, then `cd my-ide`
      and `cargo run --release`. It builds and runs, with the name filled in, and no `{{` left in
      `Cargo.toml` or `README.md`.
- [ ] **W-O2** In my-ide: double-click files in the Project tree to open tabs, edit them, and use
      the palette, a layout switch and tab tear-out.
- [ ] **W-O3** The same generate command with `templates/settings-app --name my-settings`, then
      run it.
- [ ] **W-O4** In my-settings, change several settings, close and reopen. They're kept in
      `%APPDATA%\my-settings\settings.txt`. The About page shows that path. Reset asks first,
      then restores the defaults.
- [ ] **W-O5** The search box filters the sections ("accent" leaves only Appearance).
- [ ] **W-O6** Optional: `mdbook serve book` shows the book at http://localhost:3000, and every
      chapter renders.

---

## Part 2: WSL (WSLg)

WSLg runs a Wayland compositor (Weston) and XWayland, so one Ubuntu install can test both Linux
display servers. GPU acceleration goes through Mesa's D3D12 driver on your Windows GPU.

### Setup

1. `wsl --update` in PowerShell, then open Ubuntu (24.04 recommended).
2. Install:
   ```sh
   sudo apt update
   sudo apt install build-essential pkg-config libxkbcommon-dev libxkbcommon-x11-0 libwayland-dev \
       libx11-dev libxcursor-dev libxi-dev libxrandr-dev mesa-vulkan-drivers libgl1-mesa-dri \
       vulkan-tools zenity fonts-noto-core
   curl https://sh.rustup.rs -sSf | sh
   ```
3. Clone and check out the branch inside the Linux file system (`~/charis-ui`, not `/mnt/c`:
   building there is much faster).
4. Check the session: `echo $WAYLAND_DISPLAY $DISPLAY` should print `wayland-0 :0`.
   `vulkaninfo --summary` shows whether a GPU (Dozen/D3D12) is available.

To choose the display server for a run:
- **Wayland** (the default): `cargo run --release --example dock`
- **X11** (XWayland): `WAYLAND_DISPLAY= cargo run --release --example dock`

### L-A. Build and automated checks

- [ ] **L-A1** `cargo test --all-features` passes. The `gpu` parity test runs on the WSL GPU (or
      skips itself if there's no adapter; say which).
- [ ] **L-A2** `CHARIS_PROFILE=1 cargo run --release --example counter`: note which renderer and
      adapter it picks (D3D12/Dozen, GL, llvmpipe or CPU).
- [ ] **L-A3** `cargo bench --bench frames`: paste the table.

### L-B. Wayland session

- [ ] **L-B1** `counter` (a decorated window): it has a title bar, moves, resizes and closes.
      Note whether the title bar is Weston's or winit's own.
- [ ] **L-B2** The `dock` example (frameless): dragging the title bar moves the window,
      double-click maximizes, and edges and corners resize with the right cursors.
- [ ] **L-B3** The gallery: every page works (as in W-G) and text is crisp.
- [ ] **L-B4** Keyboard: all of W-E1, and Tab navigation.
- [ ] **L-B5** `editor -- --lines 100000`: W-F1 to W-F7.
- [ ] **L-B6** Scaling: change Windows' display scale (for example to 150 %) and restart the app.
      Text and 1 px lines are sharp.
- [ ] **L-B7 (new: tear-out on Wayland)** In the dock example, drag a tab out of the window onto
      the Windows desktop and release. After about a quarter of a second a new window opens (the
      compositor picks where; that's expected).
- [ ] **L-B8 (new: dropping between windows on Wayland)** Drag the tab from that new window and
      release it over a group in the main window. It docks there, at the zone under the pointer.
      No preview shows while dragging over the other window; that's expected on Wayland.
- [ ] **L-B9 (new)** Release a dragged tab over an edge of a group in the other window: it splits
      that group on that side.
- [ ] **L-B10 (new)** Drag a tab out and release it back over its own window: it behaves like a
      normal in-window drop, with no new window.
- [ ] **L-B11** "Open in new window" and "Dock back" buttons, and closing a floating window,
      work.
- [ ] **L-B12** F11 full screen; `-- --system-font` doesn't crash (with no desktop environment
      it may fall back to Inter; note which font it used).

### L-C. X11 session (XWayland)

- [ ] **L-C1** `WAYLAND_DISPLAY= cargo run --release --example dock`: frameless moving, resizing
      and double-click maximize.
- [ ] **L-C2** Tab tear-out opens the new window where the tab is released.
- [ ] **L-C3** Dragging a tab back over the main window shows the drop preview while hovering,
      and docks it on release.
- [ ] **L-C4** The gallery and the editor with 100k lines work as on Wayland.

### L-D. Integration with Windows

- [ ] **L-D1** Copy text in the gallery (a selectable text line, Ctrl+C) and paste it into
      Notepad; copy in Notepad and paste into a gallery text field.
- [ ] **L-D2 (new)** Gallery ▸ Images ▸ "Copy image", then paste it into Paint. Then take a Windows
      screenshot (Windows+Shift+S) and paste it into the paste box. Note if WSLg doesn't carry
      images across; that would be a WSLg limitation, not ours.
- [ ] **L-D3** File dialogs (the lmfast_chat "Document" button): a zenity file picker opens, and
      picking files works.
- [ ] **L-D4** Preferences overrides:
      - `CHARIS_DARK=0 cargo run --release --example gallery` starts light;
      - `CHARIS_HIGH_CONTRAST=1` starts in high contrast;
      - `CHARIS_REDUCED_MOTION=1` makes scrolling jump instead of glide.
- [ ] **L-D5** Settings-app template under WSL: settings are saved to
      `~/.config/my-settings/settings.txt` and kept across restarts.

### Not testable in WSL

- **Screen readers:** no Orca in WSLg.
- **Native IME:** WSLg doesn't carry Windows' IME into Linux apps. You'd need ibus or fcitx
  inside WSL; this is optional.
- **GNOME settings detection** (theme, contrast, animations): there's no GNOME session; use the
  `CHARIS_*` overrides in L-D4.
- **Real multi-monitor placement.**

These need a real Linux desktop later (see `docs/MAC_LINUX_QA.md`).

---

## After testing

For each failure, give the ID, what you saw, and the `CHARIS_PROFILE=1` startup lines. Include
the full output for W-A2 and L-A1 failures, and the bench tables from W-A7 and L-A3.
