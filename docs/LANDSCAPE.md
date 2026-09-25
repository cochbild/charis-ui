# Rust UI landscape and design research (September 2026)

Research done before settling rust-ui's visual style and architecture.

**How this was checked.** Versions come from crates.io. Activity comes from git history. Behaviours were checked against source code (VS Code, Zed, Slint, Tailwind, shadcn, Radix, Fluent) wherever the documentation sites were unreachable. A few items could not be verified; they are marked *(unverified)*.

## 1. Existing Rust GUI frameworks

| Framework | Latest | Model | Render / layout | Default look and styling | Splits / docking | AccessKit |
|---|---|---|---|---|---|---|
| egui | 0.36.2 | Immediate | wgpu/glow; own single-pass layout | "Tool" look; `Style` struct | Built-in panels; docking via egui_dock / egui_tiles (tabs, splits, floating) | Yes |
| iced | 0.14.0 (0.15-dev) | Elm | wgpu + tiny-skia; own layout | Plain/flat; style closures | pane_grid (splits and rearranging, no tabs) | No (libcosmic fork: yes) |
| Slint | 1.18.1 | Reactive DSL | FemtoVG / Skia / software / Vello (experimental) | Native-ish styles (Fluent default) | **None**: splitter #7513 and docking #1723 are open issues | Yes |
| Dioxus Native (Blitz) | 0.7.10 / blitz 0.3 beta | React-like | Vello; Stylo + Taffy | **Real CSS**, looks like the web | None | Yes |
| Tauri | 2.11.6 | Webview | System webview | Electron-like (HTML/CSS) | Via JS libraries | Browser |
| GPUI (Zed) + GPUI Kit | gpui-pre 0.3.6 / kit 0.6.6 | Hybrid | Metal / wgpu / DirectX; Taffy | Tailwind-like methods; shadcn-style kit | **Full dock, serializable** | Yes (Zed main) |
| Freya | 0.4.3 (0.5 release candidate) | Signals | Skia; Torin | Modern, flat | Resizable panels; docking in 0.5 *(release unverified)* | Yes |
| Makepad | 1.0 on crates.io, active in git | Retained + DSL | Own SDF shaders | Shader styling; very free-form | Dock and splitter | Not found |
| Xilem / Masonry | 0.4.0 | Reactive | Vello, Skia; Parley text | Minimal | Split only | Yes |
| Floem | 0.2.0 on crates.io, active in git | Signals | vger / Vello / Skia; Taffy | Style closures; CSS-like transitions and springs | Resizable | Not found |
| Vizia | 0.4.0 | Reactive | Skia; morphorm | **CSS stylesheets with hot reload** | Resizable | Yes |
| gtk4-rs / Relm4 | 0.11 | Native | GTK | Looks like GNOME | Paned | AT-SPI |

**Main finding.** No framework combines all three of our goals:
- a modern web look out of the box,
- deep customization,
- IDE-grade docking with collapsible, animated panels.

The closest options each miss something:
- **GPUI Kit** has the look and the dock, but it sits on unversioned Zed internals and has poor docs.
- **Dioxus/Blitz** has real CSS but no dock, and is still beta.
- **egui** has the best dock ecosystem, but the immediate-mode look and layout.
- **Slint** is polished but has no splitter at all.
- **Tauri** gives you everything through JavaScript, but with webview inconsistencies, especially WebKitGTK on Linux.

## 2. Panel and docking behaviours to match (VS Code source is the reference)

**Sashes (resize handles)**
- 4px hit area drawn as a 1px line.
- Hover highlight appears after 300ms.
- Double-click resets sizes.

**Snap-collapse**
- A pane collapses when dragged past `floor(min / 2)` and restores when dragged back.
- Side bar: minimum 170px, snappable.
- Panel: minimum width 300, minimum height 77.

**Proportional sizing with priorities**
- Sizes are proportional, so each pane keeps its share when the window resizes.
- High-priority views are resized first.

**Drop targets**
- Edge zones rather than a "compass": 10% of the group, or 1/3 when splitting is preferred.
- The preview rectangle animates between zones.
- Modifier keys:
  - Shift: drop into the editor instead of splitting.
  - Ctrl: copy instead of move.
  - Alt: flip the split direction.
- A compass (Visual Studio / Qt Advanced Docking System) is an optional alternative style.

**Workbench features**
- Maximize a panel.
- Move the panel to left, right, top or bottom.
- Secondary side bar.
- Grid of editor groups.
- Pinned and preview tabs.
- Middle-click to close a tab.
- Drag a tab out to get a floating OS window.

**JetBrains tool-window modes**
- Pinned, unpinned (auto-hide), undocked overlay, floating, and separate window.
- Tool windows attach to stripes along the window edges.

**Layout persistence**
- The layout is a serializable tree with a version number (as in dockview, Qt ADS "perspectives", ImGui `.ini` files and GPUI Kit).

## 3. Visual conventions behind the "modern web" look

**Tailwind v4 defaults**
- 4px spacing unit.
- Text sizes 12/14/16px with line heights 16/20/24.
- Radius scale 2–16px.
- Two-layer, low-alpha shadows.
- 150ms transitions with `cubic-bezier(.4,0,.2,1)`.
- Colors defined in OKLCH.

**shadcn/ui**
- Semantic token pairs: background/foreground, card, popover, primary, muted, accent, border, input, ring.
- One radius knob, with the other radii derived from it.
- Dark surfaces are not pure black: background ≈ `oklch(.145 0 0)`, cards `.205`.
- Borders are translucent white at 10%.
- Focus ring: 3px at 50% opacity, shown only on `:focus-visible`.

**Radix Colors: a 12-step scale per color**

| Steps | Use |
|---|---|
| 1–2 | App backgrounds |
| 3–5 | Component background: normal / hover / pressed |
| 6–8 | Borders |
| 9–10 | Solid fills |
| 11–12 | Text, with contrast guarantees |

**Fluent 2**
- 14px base type size.
- Durations 50–500ms.
- Decelerate easing.

**VS Code**
- 13px system UI font.
- Grayscale antialiasing on macOS.

**Why native Rust UIs look "off" next to browsers**
1. **Text gamma and contrast.** Browsers and DirectWrite apply alpha correction and contrast enhancement to glyph edges. Naive blending makes text look thin or washed out. Zed ported Windows Terminal's gamma/contrast shader for this (zed PR #37167).
2. **Hinting and subpixel rendering.** Text looks poor at 1x scale without hinting and stem darkening (vello #204).
3. **Pixel snapping.** Text placed at fractional positions comes out blurry.
4. **Missing web details.** Layered soft shadows, 150–200ms ease-out transitions, translucent borders in dark mode, focus-visible-only rings, and font features such as tabular numbers.

## 4. Rendering

**How the leading frameworks render**
- **GPUI:**
  - Batched GPU primitives.
  - Rounded rectangles and borders drawn with signed distance fields (SDFs).
  - **Analytic Gaussian shadows** computed with `erf`, not a blur pass.
  - Glyph atlas with gamma correction.
  - Renders the full frame every time; this is cheap on the GPU.
- **Makepad:**
  - SDF shaders.
  - Text via SDF/MSDF glyph atlases.
- **Slint:**
  - Multiple renderers, including a software renderer with **dirty-region partial repaint**.

**CPU rendering**
- tiny-skia is 20–100% slower than Skia on x86 and has no text support.
- vello_cpu uses SIMD and multiple threads, and is much faster.
- A full CPU redraw of a 4K frame at 120Hz is not viable.

## 5. What this means for rust-ui

What we already match:
- Taffy flexbox/grid layout.
- CSS-like styles with transitions.
- Proportional plus fixed splits.
- Snap-collapse at `min / 2` (the same rule VS Code uses).
- Double-click to reset a sash.
- Drag-and-drop docking with edge zones.
- Frameless chrome.
- Headless tests.

Changes indicated by the research, in priority order:

1. **Renderer.** Make a wgpu GPU renderer the primary path:
   - SDF quads.
   - Analytic `erf` shadows.
   - Glyph atlas with gamma/contrast correction and pixel snapping.

   Keep the CPU renderer as a fallback and for tests, and add dirty-region repaint to it.
2. **Theme tokens.** Rebuild tokens on the Radix/shadcn model:
   - OKLCH 12-step primitive scales.
   - A semantic role layer that components reference.
   - A few global knobs (accent, gray, radius, scaling, density).
   - A theme generated from one seed color.
3. **Typography and motion defaults.** Use a Tailwind/Fluent scale:
   - 13–14px base text.
   - 4px spacing unit.
   - 150ms transitions with `cubic-bezier(.4,0,.2,1)`.
   - Spring easing as an option.
   - Focus rings only for keyboard focus (focus-visible).
4. **Dock and splits.** Add to what exists:
   - 300ms hover delay on sashes.
   - Resize priorities.
   - Animated drop preview.
   - Tab reordering with an insertion marker.
   - Maximize a panel.
   - Move a panel to any edge.
   - Auto-hide (unpinned) panels.
   - Floating OS windows.
   - Serde persistence with a layout version.
5. **Styling.** An optional stylesheet layer (as in Vizia and Blitz) with hot reload, on top of the builder API.
6. **Platform.** AccessKit and IME pre-edit (the text being composed) from the start.
7. **Tooling.** An element inspector and an inspection/automation protocol (as in egui 0.35 and Floem).

## Sources (selection)

- VS Code:
  - https://github.com/microsoft/vscode/blob/main/src/vs/base/browser/ui/sash/sash.ts
  - https://github.com/microsoft/vscode/blob/main/src/vs/base/browser/ui/splitview/splitview.ts
  - https://github.com/microsoft/vscode/blob/main/src/vs/workbench/browser/parts/editor/editorDropTarget.ts
- Zed / GPUI:
  - https://github.com/zed-industries/zed/pull/37167
  - https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md
  - https://github.com/longbridge/gpui-kit
- egui: https://github.com/emilk/egui/blob/main/CHANGELOG.md, https://github.com/Adanos020/egui_dock
- iced: https://github.com/iced-rs/iced/releases/tag/0.14.0
- Slint:
  - https://github.com/slint-ui/slint/blob/master/CHANGELOG.md
  - https://github.com/slint-ui/slint/issues/1723
  - https://github.com/slint-ui/slint/issues/7513
- Dioxus: https://dioxuslabs.com/blog/release-070/, https://github.com/dioxuslabs/blitz
- Tauri: https://v2.tauri.app/develop/debug/linux-graphics/
- Other frameworks: https://github.com/marc2332/freya, https://github.com/makepad/makepad, https://github.com/linebender/xilem
- Design tokens:
  - https://github.com/tailwindlabs/tailwindcss/blob/main/packages/tailwindcss/theme.css
  - https://github.com/shadcn-ui/ui/blob/main/apps/v4/app/globals.css
  - https://github.com/radix-ui/themes
- Rendering: https://github.com/linebender/vello/issues/204, https://github.com/linebender/tiny-skia
