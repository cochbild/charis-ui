# macOS and Linux QA checklist

The automated tests run headlessly on Linux (X11 under Xvfb), so these checks need real machines: macOS 15, and Ubuntu (or Fedora) with GNOME on Wayland and on
X11. Run:

```sh
cargo run --release --example dock
cargo run --release --example showcase
```

## macOS

**Title bar and full screen**
- [ ] The dock demo's title bar shows the three traffic lights at its left, vertically
      centered, with the File / View / Window menus to their right, not under them.
- [ ] Resizing the window keeps the traffic lights in place (AppKit resets them; the app moves
      them back).
- [ ] The green button, View ▸ Full Screen, and ⌃⌘F enter native full screen (a new Space, with
      the animation). In full screen the traffic lights hide and the title bar content moves to
      the left edge. The same keys leave full screen.
- [ ] Dragging the title bar moves the window; double-clicking it zooms.

**Keys and menus**
- [ ] In the Assistant's text field: ⌘A, ⌘C, ⌘V, ⌘X, ⌘Z; ⌘←/⌘→ go to the line's start and
      end; ⌥←/⌥→ move by word; ⌘⌫ deletes to the line start; ⌥⌫ deletes a word.
- [ ] The menu bar has the app menu (About, Hide ⌘H, Quit ⌘Q) and a Window menu with
      Minimize ⌘M, Zoom, Enter Full Screen and Close ⌘W before the app's own items.
- [ ] ⇧⌘P opens the command palette, and ⌘K ⌘S opens Keyboard Shortcuts.

**Scaling and fonts**
- [ ] On a Retina display, text and 1 px lines are sharp. Moving the window to a non-Retina
      external display re-renders it sharp there too, at the same size.
- [ ] `cargo run --release --example dock -- --system-font`: text uses SF Pro.

## Linux

**Decorations** (GNOME Wayland, then GNOME X11)
- [ ] `cargo run --release --example counter` (a decorated window): on Wayland, GNOME has no
      server-side decorations, so winit draws its Adwaita title bar. It's dark when the app's
      theme is dark. On X11 the window manager's title bar is used.
- [ ] The dock demo (frameless): dragging the title bar moves the window; its edges and
      corners resize it, with resize cursors; double-click maximizes. On both Wayland and X11.

**Fractional scaling** (Settings ▸ Displays ▸ Scale 125 % or 150 %, with fractional scaling
enabled)
- [ ] Borders and separators are one crisp line (not a blurry two-pixel smear); text is sharp.
- [ ] Moving the window between a 100 % and a 150 % monitor re-renders it sharp at the same
      size.

**Fonts and full screen**
- [ ] `-- --system-font`: GNOME uses Adwaita Sans (or Cantarell), KDE Noto Sans, Ubuntu the
      Ubuntu font.
- [ ] F11 toggles full screen.

Report anything that fails, with a screenshot if possible.
