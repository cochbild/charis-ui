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
- [ ] While composing, the text being composed shows inside the input with an underline, and the
      IME candidate window opens next to the caret (not at the window's corner).
- [ ] With focus outside a text input (e.g. after clicking a button), single-letter shortcuts are
      not swallowed by the IME.
- [ ] Wheel and touchpad scrolling are smooth.

**Accessibility** (turn on Narrator with Ctrl+Win+Enter; NVDA works too)
- [ ] Narrator reads the window, and Tab moves between controls, announcing each one's name and
      type ("Save, button"; "Remember me, check box, not checked").
- [ ] Pressing Space or Enter (or Narrator's Caps Lock+Enter) activates the focused button or
      checkbox, and the new state is announced.
- [ ] In the lmfast demo's Settings screen, the accent swatches read as radio buttons, and the
      corner-radius slider announces its value and changes with the arrow keys.
- [ ] Narrator's scan mode (Caps Lock+Space) can move through headings and text in a chat reply.
- [ ] Accessibility Insights for Windows (optional) shows the tree with sensible names and no
      unnamed buttons.

**High contrast and reduced motion**
- [ ] Turn on a contrast theme (Settings → Accessibility → Contrast themes → Aquatic), then switch
      back to the lmfast demo: it starts (or, after refocusing, reports) high contrast. Settings →
      Appearance → Contrast → High looks the same.
- [ ] Turn off Settings → Accessibility → Visual effects → Animation effects, then refocus the
      demo: wheel scrolling jumps instead of gliding, and collapsing a sidebar is instant. Hover
      color fades still work.

**Multiple windows and tab tear-out** (`cargo run --release --example dock`)
- [ ] Drag the "Terminal" tab out of the window: a new window opens where you release it, with
      Windows' own title bar, snap and shadow.
- [ ] Drag that tab back onto a panel in the main window: the edge/center preview shows while
      hovering, and it docks there when released; the empty window closes.
- [ ] "Open in new window" (↗ in a panel header) and "Dock back" work; closing a floating window
      with its ✕ puts its tabs back in the main window.
- [ ] On a second monitor with different scaling: drag a tab out onto it; the new window is
      sharp and correctly sized.
- [ ] `cargo run --release --example multiwindow`: inspector windows open, share the count, and
      close independently.

Report anything that fails, with a screenshot if possible.
