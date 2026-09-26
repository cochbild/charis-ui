# Performance and scaling

Measured with `cargo run --release --example stress`. That benchmark is the worst case: N rows of
mixed widgets (icon, text, badge, button) with no virtualization. Frame times are CPU time on
the build container, not a fast desktop.

| Elements | Unchanged frame (hover, scroll, animation) | Every row's text changed |
|---|---|---|
| 703 | 0.7 ms | 3.9 ms |
| 7,003 | 12.5 ms | 60 ms |
| 35,003 | 121 ms | 349 ms |

A 200,000-row `virtual_list` costs about the same per frame as a 100-row one (0.6 ms vs 0.24 ms),
because only the rows near the viewport exist.

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
   settings pages and sidebars are good candidates. The lmfast demo memoizes each chat message.
5. **Profile** with `RUI_PROFILE=1`, which prints per-frame timings for view, flatten, layout and
   paint.

## Next

- Damage-region repaint on the CPU backend.
- Incremental accessibility-tree updates (currently a full tree per frame while a screen reader
  is active).
