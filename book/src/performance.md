# Performance

The target is a frame in well under 4 ms of CPU time for an IDE-sized UI, and 120 fps while
scrolling a 100,000-row table. `cargo bench --bench frames` measures both against those
budgets; the current numbers are in `docs/PERFORMANCE.md` in the repository.

## What the runtime does for you

- **Only rebuild when something changed.** `view` runs after a message, an input event that
  changes UI state, or an animation frame, not on a timer.
- **Incremental layout.** The layout tree lives between frames; only nodes whose style, text or
  children changed are laid out again.
- **Text is shaped once** per string and style, and only re-wrapped for new widths.
- **Culling.** Elements outside their clip aren't painted.
- **One GPU draw call** for the whole frame: rectangles, borders, shadows, glyphs and images are
  instanced quads.
- **Damage tracking** on the CPU renderer: only the changed area is rasterized and presented.

## What your app should do

1. **Virtualize long collections.** Use `virtual_list`, `table` or the `tree` module for anything
   that can grow past a few hundred rows. They build only the rows near the viewport, so 200,000
   rows cost about the same as 100.
2. **Give dynamic items stable ids** with `.id()` or `.key()`. Their scroll, hover and layout
   state then follows the item instead of its position.
3. **Memoize big parts that change rarely** with `lazy(key, deps, || view)`: chat messages,
   rendered markdown, settings pages, sidebars.

   ```rust
   # use charis_ui::prelude::*;
   # struct Message { text: String, expanded: bool }
   # fn message_view(m: &Message) -> Element<()> { text(m.text.clone()) }
   # let messages = vec![Message { text: "hi".into(), expanded: false }];
   let mut list = col();
   for (i, m) in messages.iter().enumerate() {
       list = list.child(lazy(("message", i), (&m.text, m.expanded), || message_view(m)));
   }
   ```

4. **Keep changing text small.** A clock or a token counter is cheap; rewriting thousands of
   strings every frame means shaping them all again.
5. **Do slow work off the UI thread** with `cx.spawn` or `cx.spawn_blocking`.

## Measuring

- `CHARIS_PROFILE=1` prints the GPU adapter and per-frame timings for view, flatten, layout and
  paint.
- `cargo run --release --example stress -- 5000` shows how a large, non-virtualized UI behaves.
- `cargo bench --bench frames -- --json out.json` writes the frame-budget numbers for tracking.
