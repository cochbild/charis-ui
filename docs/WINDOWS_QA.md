# Windows QA checklist

This container can only type-check the Windows code, so these checks need a real Windows 10 or 11
machine. Run:

```powershell
cargo run --release --example showcase
cargo run --release --example dock
```

Set `$env:RUI_PROFILE=1` to print the renderer and adapter in use.

**Frameless window chrome**
- [ ] The window has a drop shadow and (on Windows 11) rounded corners, with no white border or
      title strip.
- [ ] Dragging empty title-bar space moves the window.
- [ ] Dragging the window to a screen edge or corner snaps it (Aero Snap).
- [ ] Shaking the window by its title bar minimizes the others.
- [ ] Double-clicking empty title-bar space maximizes it; double-clicking again restores it.
- [ ] **Windows 11: hovering the maximize button shows the Snap Layouts flyout.**
- [ ] The maximize button shows a hover highlight, and clicking it maximizes or restores.
- [ ] Right-clicking the title bar, or pressing Alt+Space, opens the system menu, and its items
      work.
- [ ] The menu bar (File/Edit/View), layout toggle buttons and the minimize and close buttons are
      clickable, not draggable.
- [ ] All four edges and corners resize the window, with the right resize cursors.
- [ ] Maximized, nothing is cut off at the screen edges and the title bar sits at the very top.
- [ ] Switching the theme to Light makes the system menu light; Dark makes it dark.
- [ ] Moving the window between monitors with different scaling keeps it crisp and correctly sized.

**Rendering**
- [ ] `RUI_PROFILE=1` reports the GPU renderer on your GPU (not a fallback).
- [ ] `$env:RUI_RENDERER="cpu"` also works and looks the same.
- [ ] Text is crisp at 100%, 125%, 150% and 200% display scaling.

**Input**
- [ ] Typing in text inputs works, including Ctrl+C, Ctrl+V, Ctrl+A and Ctrl+Backspace.
- [ ] IME input (e.g. Microsoft Pinyin or Japanese IME) commits text into inputs.
- [ ] Wheel and touchpad scrolling are smooth.

Report anything that fails, with a screenshot if possible.
