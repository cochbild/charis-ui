# Performance and scaling

Measured with `cargo run --release --example stress`. That benchmark is the worst case: N rows of
mixed widgets (icon, text, badge, button) with no virtualization. Frame times are CPU time on
a modest Linux VM without a GPU, not a fast desktop.

| Elements | Unchanged frame (hover, scroll, animation) | Every row's text changed |
|---|---|---|
| 703 | 0.7 ms | 3.9 ms |
| 7,003 | 12.5 ms | 60 ms |
| 35,003 | 121 ms | 349 ms |

A 200,000-row `virtual_list` costs about the same per frame as a 100-row one (0.6 ms vs 0.24 ms),
because only the rows near the viewport exist.

## Frame budgets

`cargo bench --bench frames` measures CPU time per frame against the roadmap's budgets (release
build, the same Linux VM; `-- --json out.json` writes the numbers for tracking). "scene" is
the work the GPU backend needs (view, layout, paint recording); "cpu" adds rasterizing on the CPU
backend.

| Scenario | Median | p95 | Budget |
|---|---|---|---|
| showcase: unchanged frame, scene | 0.79 ms | 1.13 ms | 4.0 ms ✓ |
| showcase: unchanged frame, cpu | 0.90 ms | 1.41 ms | 4.0 ms ✓ |
| showcase: hovering, scene | 0.83 ms | 1.04 ms | 4.0 ms ✓ |
| showcase: hovering, cpu | 1.31 ms | 1.58 ms | 8.0 ms ✓ |
| showcase: full redraw, cpu | 6.38 ms | 8.45 ms | — |
| 100k-row table: scrolling, scene | 0.48 ms | 0.64 ms | 8.3 ms ✓ |
| 100k-row table: scrolling, cpu | 1.37 ms | 1.97 ms | 8.3 ms ✓ |
| 100k-row tree: scrolling, scene | 0.48 ms | 0.61 ms | 8.3 ms ✓ |
| 100k-line editor: scrolling, scene | 0.91 ms | 1.21 ms | 8.3 ms ✓ |
| 100k-line editor: keystroke to frame, scene | 4.37 ms | 5.92 ms | 8.3 ms ✓ |

## Damage tracking (CPU backend)

The CPU renderer redraws only what changed. Every drawing command gets a fingerprint (its content
plus the clip and opacity it's drawn under) and bounds; commands are matched against last frame's,
and the bounding box of those that appeared, disappeared or changed drawing order is redrawn:
- into its own pixmap, offset like a layer, with a margin, so every drawing decision is the same
  as in a full frame;
- then copied over last frame's pixels.

Frames where nothing changed skip rasterizing entirely. The window presents only the redrawn
rectangle (softbuffer's `present_with_damage`, keeping older back buffers up to date by their
age). Tests check that partial frames are pixel-identical to full ones through hover, typing,
overlays, scrolling, fades, shadows and images, and through 600 random interactions at four
scales. `CHARIS_NO_DAMAGE=1` or `Runtime::set_damage_tracking(false)` turns it off.

With it, a hover frame in the showcase costs 1.3 ms instead of a 6.4 ms full redraw.

## What makes it scale

- **Incremental layout.** The taffy layout tree persists between frames, keyed by element id.
  Each frame only nodes whose style, text or children changed are marked dirty, and taffy reuses
  cached layout for everything else. An unchanged 700-element frame dropped from 12.6 ms to
  0.7 ms.
- **Shape once, re-wrap cheaply.** Text is shaped once per string and style. Layout asking for
  other widths only re-wraps it, and cache lookups don't allocate.
- **Culling.** Elements outside their clip region aren't painted, so recording cost follows what
  is visible, not what exists.
- **One GPU draw call.** Rectangles, borders, shadows and glyphs are drawn as instanced SDF quads.
- **Small elements.** Rarely used parts of an element (state styles, semantics) are boxed, so
  builder chains move less data.

## Memoized subtrees: `lazy`

`lazy(key, deps, || view)` skips its closure while `deps` hashes the same as last frame. The
runtime then reuses last frame's elements, and layout reuses their cached results. It rebuilds
the subtree automatically when:
- the theme changes;
- something inside is hovered, pressed or focused, or a transition inside is running;
- it contains live-state widgets (virtual lists, splits, dropdowns, table columns).

Nested `lazy`s are reused independently.

```rust
for (i, m) in messages.iter().enumerate() {
    list = list.child(lazy(("message", i), (&m.text, m.expanded), || message_view(m)));
}
```

Same benchmark with each row wrapped in `lazy` (unchanged frames):

| Elements | Without `lazy` | With `lazy` |
|---|---|---|
| ~800 | 0.61 ms | 0.35 ms |
| ~8,000 | 7.6 ms | 4.2 ms |
| ~40,000 | 82 ms | 47 ms |

The time left is mostly moving reused nodes into the new frame, and layout's per-node
bookkeeping.

## Guidelines for apps

1. **Virtualize long collections.** Use `virtual_list` or `table` for anything that can grow past
   a few hundred rows: logs, chats, file lists.
2. **Give dynamic items stable ids** (`.id()` or `.key()`). State (scroll, hover, layout cache)
   then follows the item instead of its position.
3. **Keep changing text small.** A per-second clock or a streaming token counter is cheap. Changing
   thousands of strings every frame means reshaping them all.
4. **Wrap expensive, rarely-changing parts in `lazy`.** Chat messages, rendered markdown,
   settings pages and sidebars are good candidates. The `chat` example memoizes each message.
5. **Profile** with `CHARIS_PROFILE=1`, which prints per-frame timings for view, flatten, layout and
   paint.

## Large text documents

`text_area` lays its value out by paragraph (the text between newlines):

- Only paragraphs on screen, at the caret, or under the pointer are shaped. The others get an
  estimated height, replaced by the real one when they're first shown; the view stays anchored
  meanwhile, like `virtual_list`.
- An edit compares the new value with the old one (common prefix and suffix) and re-measures
  only the paragraphs it touched.
- The runtime shares the value instead of copying it, and undo stores edits rather than copies
  of the text.

| 100k-line document (3.5 MB) | Median | p95 |
|---|---|---|
| open (first frame) | 18 ms | — |
| scrolling, scene | 0.91 ms | 1.21 ms |
| typing, scene | 0.84 ms | 1.28 ms |
| keystroke to frame (event, update, view, frame) | 4.37 ms | 5.92 ms |
| arrow down, scene | 0.89 ms | 1.07 ms |

Most of a keystroke's remaining cost is copying the document: the app gets the new value as a
`String` and passes a copy back in `view`. Peak memory for the document above is about 60 MB.

## Accessibility

While a screen reader is active, each frame sends only the accessibility nodes that changed
since the last update, so an idle frame sends nothing and toggling a checkbox sends a handful of
nodes.
