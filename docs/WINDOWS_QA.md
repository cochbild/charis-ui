# Windows QA checklist

For a full pass on a home Windows 11 machine (and WSL), use
[`HOME_TEST_PLAN.md`](HOME_TEST_PLAN.md): it includes this list and everything added since.

This container can only type-check the Windows code, so these checks need a real Windows 10 or 11
machine. Run:

```powershell
cargo run --release --example showcase
cargo run --release --example dock
```

Set `$env:CHARIS_PROFILE=1` to print the renderer and adapter in use.

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
- [ ] `CHARIS_PROFILE=1` reports the GPU renderer on your GPU (not a fallback).
- [ ] `$env:CHARIS_RENDERER="cpu"` also works and looks the same.
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
- [ ] In the `chat` example's Settings screen, the accent swatches read as radio buttons, and the
      corner-radius slider announces its value and changes with the arrow keys.
- [ ] Narrator's scan mode (Caps Lock+Space) can move through headings and text in a chat reply.
- [ ] Accessibility Insights for Windows (optional) shows the tree with sensible names and no
      unnamed buttons.

**High contrast and reduced motion**
- [ ] Turn on a contrast theme (Settings → Accessibility → Contrast themes → Aquatic), then switch
      back to the `chat` example: it starts (or, after refocusing, reports) high contrast. Settings →
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

**Menus** (`cargo run --release --example dock`)
- [ ] File / View / Window in the title bar open on click; hovering "Open Panel" opens its
      submenu to the right; picking an item runs it and closes the menu; clicking elsewhere closes it.
- [ ] Shortcuts: Ctrl+Shift+T toggles the theme, Ctrl+N opens an editor tab, even with focus in
      the Assistant's text box.

**Tool windows** (dock example)
- [ ] The ★ (left), branch and ✓ (right) and ▶ (bottom) stripe buttons open Bookmarks, Git, Todo
      and Build. Todo is pinned (pushes the dock aside); the others slide over it and hide when
      you click the editor or press Escape.
- [ ] The pin button in a panel's header switches between pinned and auto-hide; dragging an
      auto-hide panel's inner edge resizes it.

**Panel commands, compass and layouts** (dock example)
- [ ] Right-click a tab: the menu opens at the pointer; "Split Right" and "Move to Edge ▸ Bottom"
      do what they say. With a tab focused (click it), Shift+F10 opens the menu, ↑/↓ move, Enter
      picks, Escape closes.
- [ ] While dragging a tab over a group, the compass appears in its middle; releasing on its left
      square splits the group left. The four guides at the dock's edges add full-height or
      full-width panels.
- [ ] Tab to a splitter (it lights up), then ←/→ resize it.
- [ ] Window ▸ Layouts: "Focus" and "Review" switch the arrangement (the title bar badge follows);
      "Save Layout As…" opens a dialog with the name selected for typing, Enter saves, Escape
      cancels.

**Commands and key bindings** (dock example)
- [ ] Ctrl+Shift+P (or F1) opens the command palette; typing filters, ↑/↓ select, Enter runs,
      Escape closes.
- [ ] Ctrl+K, then Ctrl+S: the status bar says it's waiting after Ctrl+K, then the Keyboard
      Shortcuts dialog opens. "Change" on a command, press a key, Enter: the new key works and
      the View menu shows it.
- [ ] Ctrl+1 … Ctrl+6 move focus to the dock's panels; Alt+2/4/6/9 toggle tool windows.
- [ ] Open the pinned Todo tool window (Alt+6), then make the window very narrow: Todo shrinks
      only after the dock's panels reached their minimum, and gets its width back when the
      window is wide again.

**Backdrop, full screen, fonts, tree** (dock example)
- [ ] `cargo run --release --example dock -- --mica` on Windows 11: the title bar and the tool
      window stripes show Mica (the wallpaper's tint shows through); panels stay opaque.
      Without a GPU (`$env:CHARIS_RENDERER="cpu"`), the window opens normally, without Mica.
- [ ] F11 toggles full screen; the frame edges don't resize while in full screen.
- [ ] `-- --system-font`: text uses Segoe UI Variable.
- [ ] Move the window between monitors with different scaling (100 % and 150 %): text and 1 px
      lines stay sharp on both, and the window keeps its size in logical pixels.
- [ ] Alt+1 opens the Project tree; expand "generated (100000 files)", scroll with the wheel and
      the keyboard (↓, PageDown, End); it stays smooth. Double-clicking a file opens it in the
      editor group.

**File dialogs** (`cargo run --release --example chat --features markdown`)
- [ ] "Document" under the message box opens the Windows file picker (modal to the window) with a
      "Documents" filter; picking two files shows two chips; ✕ removes one; Cancel changes nothing.

Report anything that fails, with a screenshot if possible.
