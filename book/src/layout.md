# Layout

Every element is a box laid out with flexbox or grid, the same model as CSS (rust-ui uses
[taffy](https://github.com/DioxusLabs/taffy)). Sizes are in logical pixels; the runtime scales
them for the display.

## Boxes

| Function | What it makes |
|---|---|
| `div()` | A plain box. |
| `row()` | A box whose children go left to right (`flex-direction: row`). |
| `col()` | A box whose children go top to bottom (`flex-direction: column`). |
| `text(s)` | A text run. It wraps to its box unless you call `.nowrap()`. |
| `icon(Icon::…)` | A vector icon. |
| `spacer()` | An empty box that grows to fill free space. |
| `canvas(|cv, rect| …)` | Custom drawing. |

Children are added with `.child(e)`, `.children(iter)` and `.child_if(cond, || e)`.

## Flexbox

```rust
use rust_ui::prelude::*;
# #[derive(Clone)] enum Msg { Back }

fn toolbar() -> Element<Msg> {
    row()
        .h(40.0)
        .px(12.0)
        .gap(8.0)
        .items_center() // align-items: center
        .child(icon_button(Icon::ChevronLeft).on_click(Msg::Back))
        .child(text("Settings").semibold())
        .child(spacer()) // pushes what follows to the right
        .child(text("v1.2").font_size(11.0))
}
# let _ = toolbar();
```

The flex methods mirror CSS:

- **Direction and wrapping:** `flex_row()`, `flex_col()`, `flex_wrap()`.
- **Growing:** `grow(f)`, `shrink(f)`, `basis(len)`, and `flex1()` (grow and shrink from zero).
- **Alignment:** `justify(Justify::…)`, `justify_between()`, `items(Align::…)`,
  `items_center()`, `self_align(Align::…)`, and `center()` (both axes).
- **Spacing:** `gap(px)` and `gap_xy(x, y)`.

## Sizes

- `w(len)`, `h(len)`, `size(w, h)`, `square(px)`.
- `w_full()`, `h_full()`, `size_full()` (100 %).
- `min_w`, `min_h`, `max_w`, `max_h`, `aspect_ratio(r)`.

A length is a number of pixels, or a percentage with `pct(50.0)`:

```rust
# use rust_ui::prelude::*;
let sidebar: Element<()> = col().w(pct(25.0)).min_w(180.0).max_w(420.0).h_full();
```

Padding is `p`, `px`, `py`, `pt`, `pr`, `pb`, `pl`; margin is `m`, `mx`, `my`, `mt`, `mr`, `mb`,
`ml`. `ml_auto()` and `mt_auto()` push an element to the far end, as `margin-left: auto` does.

## Grid

`grid(columns)` turns a box into a grid. Tracks are `Track::Px`, `Track::Fr`, `Track::Percent`
or `Track::Auto`. Children flow into cells in order; `col_span(n)` and `row_span(n)` make them
span.

```rust
use rust_ui::prelude::*;

fn settings_form() -> Element<()> {
    div()
        .grid(vec![Track::Px(140.0), Track::Fr(1.0)])
        .gap_xy(16.0, 10.0)
        .items_center()
        .child(text("Name"))
        .child(text_input("Ada", |_| ()))
        .child(text("Theme"))
        .child(segmented(vec![("Light".into(), false, ()), ("Dark".into(), true, ()), ("System".into(), false, ())]))
        .child(text("A note that spans both columns.").col_span(2))
}
# let _ = settings_form();
```

## Scrolling and clipping

`scroll_y()`, `scroll_x()` and `scroll_both()` make a box scroll, with smooth scrolling and
overlay scrollbars. `clip()` hides overflow without scrolling.

A scrolling box needs a bounded size, from its own height or from its parent's flex layout
(`grow(1.0)` inside a column of fixed height, for example).

For long lists, `virtual_list(count, |i| row)` builds only the rows near the viewport. Rows can
have different heights; each is measured the first time it's shown.

```rust
# use rust_ui::prelude::*;
let log: Element<()> = virtual_list(100_000, |i| text(format!("line {i}")).h(20.0)).id("log").h(400.0);
```

## Positioning

`absolute()` takes an element out of the flow, placed with `top`, `right`, `bottom`, `left` (or
`inset0()`) inside its nearest positioned ancestor. `fixed()` places it relative to the window,
above everything else, and escapes any clipping; menus and tooltips use it. `z_index(n)` orders
overlapping elements.

## Splits

`hsplit` and `vsplit` lay out panes with draggable splitters between them:

```rust
use rust_ui::prelude::*;

#[derive(Clone)]
enum Msg {
    Collapsed(usize, bool),
}

fn shell(sidebar_open: bool) -> Element<Msg> {
    hsplit(
        "main",
        vec![
            Pane::fixed(260.0, text("Explorer")).min(170.0).max(520.0).collapsible(true).collapsed(!sidebar_open),
            Pane::fill(vsplit(
                "center",
                vec![Pane::fill(text("Editor")), Pane::fixed(220.0, text("Terminal")).key("terminal")],
            )),
        ],
    )
    .on_collapse(Msg::Collapsed)
}
# let _ = shell(true);
```

- `Pane::fixed(px, e)` keeps a size in pixels, `Pane::fill(e)` takes what's left, and
  `Pane::flex(weight, e)` takes a proportional share.
- `.min`, `.max` and `.collapsible(true)` bound a pane; collapsing it slides it shut.
- `.priority(Priority::High)` panes shrink first when the window gets small; `Priority::Low`
  panes keep their size longest.
- `.key("name")` gives a pane that comes and goes its last size back when it returns.
- Users can resize splitters with the keyboard: Tab to one, then the arrow keys, Home and End.

For whole IDE layouts with tabs, see [Panels and docking](panels.md).
