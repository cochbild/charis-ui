# Architecture

How Charis turns an app's `view` into pixels, and where each part lives in `src/`. For using
the framework, read [the book](../book/src/introduction.md). This page is for contributors and
for anyone curious about the internals.

## A frame

1. **View.** When something changed, the runtime calls the app's `view` (and `window_view` for
   extra windows). The result is a tree of `Element`s, built with chained methods. Parts wrapped
   in `lazy(key, deps, …)` are reused from the last frame while their `deps` are unchanged.
2. **Retain.** The runtime matches the new tree against the last one by identity (`.id()`,
   `.key()`, or position) and keeps UI-only state on the matching nodes: scroll offsets,
   splitter positions, text cursors, focus and running animations. The app's model never holds
   that state.
3. **Layout.** [taffy] computes flexbox and grid layout. Layout is incremental: only nodes whose
   style or content changed, and their ancestors, are laid out again. Text is shaped and
   measured by [cosmic-text].
4. **Record.** The frame is recorded once into a display list (`scene`): rectangles, borders,
   shadows, glyph runs, paths and images, with clips and layers.
5. **Draw.** A renderer turns the display list into pixels: the GPU renderer by default, or the
   CPU renderer as a fallback and in tests.
6. **Events.** Input from winit (or from a headless test) is hit-tested against the laid-out
   tree, routed to handlers, and turned into the app's messages. `update` runs, and the cycle
   starts again.

## Layers

| Layer | Where |
|---|---|
| Declarative tree and builders | `element`, `style`, `widgets`, `table`, `tree`, `dock`, `toolwin`, `layouts` |
| App model: messages, effects, async work, subscriptions, components | `runtime`, `effects`, `subscription`, `component` |
| Runtime: retained state, events, hit-testing, focus, drag and drop, animations, memoized subtrees | `runtime` (`mod`, `memo`), `anim` |
| Layout (flexbox and grid) | [taffy] |
| Text shaping and rasterization | `text`, `text_doc` ([cosmic-text] and swash, with Inter bundled) |
| Display list | `scene`, `paint` (the `canvas` API records into it) |
| GPU renderer (default) | `gpu`, `gpu.wgsl`: wgpu |
| CPU renderer (fallback, tests) | `cpu`, `damage`: [tiny-skia] |
| Theme and styling | `theme`, `color`, `stylesheet` |
| Commands, menus and key bindings | `commands`, `menu`, `native_menu` |
| Accessibility | `semantics`, `runtime::a11y` ([AccessKit]) |
| Windowing and platform integration | `window`, `platform`, `system`, `dialog`: winit, with wgpu surfaces or softbuffer, arboard for the clipboard and rfd for file dialogs |
| Headless testing | `headless` |
| Developer tools: the element inspector and hot-reloaded stylesheets | `runtime::inspector`, `stylesheet` |

## Renderers

**GPU.** The whole frame is one instanced draw call. Rounded rectangles and borders are signed
distance fields, shadows are analytic Gaussian blurs, and glyphs and paths come from texture
atlases. The GPU renderer is used when wgpu finds an adapter: Vulkan, Metal, DX12 or GL.

**CPU.** tiny-skia draws the same display list, with bounded scratch buffers for clips and
layers. It redraws only the parts of the window that changed (damage tracking) and presents
just those regions. Set `CHARIS_NO_DAMAGE=1` to force full redraws when debugging.

**Choosing one.** Set `CHARIS_RENDERER=cpu` to force the CPU renderer, or build without the
`gpu` feature. `WGPU_ADAPTER_NAME` picks a specific adapter by name. `CHARIS_PROFILE=1` prints
the chosen adapter and backend at startup, followed by per-frame timings.

**Parity.** A test renders the same frame on both renderers and fails if they differ by more
than a small tolerance, so the CPU renderer stays a faithful reference.

## Text quality

Glyph coverage gets DirectWrite-style contrast enhancement and gamma correction, the approach
Windows Terminal and Zed use to match browser and native text. Baselines snap to the pixel
grid. Both renderers apply the same correction.

Large documents in `text_area` are split into paragraphs. Only the paragraphs that are visible,
under the caret or being hit-tested are shaped, so a 100,000-line document opens in tens of
milliseconds and edits stay fast.

## Performance

An unchanged frame of the showcase app costs under a millisecond of CPU time, and a full redraw
on the CPU renderer about 6 ms. On the GPU renderer, the CPU side of a frame is view, layout and
recording, and the GPU draws everything in one call. Small changes cost little, because layout
is incremental and damage tracking limits redraws.

`cargo bench --bench frames` measures frame times for typical scenarios against their budgets.
[PERFORMANCE.md](PERFORMANCE.md) has the numbers and guidelines for keeping apps fast.

[taffy]: https://github.com/DioxusLabs/taffy
[cosmic-text]: https://github.com/pop-os/cosmic-text
[tiny-skia]: https://github.com/RazrFalcon/tiny-skia
[AccessKit]: https://accesskit.dev
