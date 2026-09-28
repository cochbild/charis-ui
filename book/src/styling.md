# Styling

Styling methods are chained on any element and mirror CSS. They apply to that one element; to
restyle every instance of a widget, use [style classes](theming.md#style-classes).

## Visuals

```rust
use charis_ui::prelude::*;

fn tile() -> Element<()> {
    row()
        .items_center()
        .gap(8.0)
        .px(12.0)
        .h(32.0)
        .bg(hex("#1e1f24"))
        .border(1.0, hex("#2f3138"))
        .rounded(8.0)
        .shadow(Shadow::new(0.0, 4.0, 12.0, 0.0, Color::BLACK.with_alpha(0.3)))
        .child(text("Hello"))
}
# let _ = tile();
```

| Method | CSS equivalent |
|---|---|
| `bg(color)` | `background-color` |
| `gradient(angle, stops)` | `background: linear-gradient(…)` |
| `border(width, color)`, `border_t/r/b/l` | `border`, `border-top`… |
| `rounded(px)`, `radius(Corners)`, `pill()` | `border-radius` |
| `shadow(s)`, `shadows(vec)`, `no_shadow()` | `box-shadow` (layered, blurred) |
| `opacity(f)` | `opacity` |
| `outline(width, offset, color)` | `outline` and `outline-offset` |
| `translate(x, y)` | `transform: translate(…)` |
| `cursor(Cursor::…)` | `cursor` |

Colors come from `hex("#rrggbb")`, `rgb(r, g, b)`, `rgba(r, g, b, a)` or `oklch(l, c, h)`, or
from the theme (next chapter). `Color::with_alpha(a)` makes a translucent copy.

## Text

`font_size`, `weight` (or `medium()`, `semibold()`, `bold()`), `italic()`, `mono()`,
`font(FontFamily)`, `line_height`, `letter_spacing`, `text_align`, `color`, `nowrap()` and
`ellipsis()`. Text properties inherit, as in CSS: set `color` or `font_size` on a container and
every text inside picks it up.

For mixed styles in one paragraph, use `rich_text` with spans:

```rust
# use charis_ui::prelude::*;
let line: Element<()> = rich_text(vec![span("Build "), span("failed").bold().color(hex("#f87171")), span(" in 2.1s")]);
```

`.selectable()` lets users select and copy text; `markdown(src)` renders Markdown.

## States and transitions

`hover`, `active` (pressed), `focus_style` and `disabled_style` take a closure that adjusts the
style in that state. `transition(secs)` animates the change:

```rust
# use charis_ui::prelude::*;
let card: Element<()> = col()
    .p(16.0)
    .rounded(10.0)
    .bg(hex("#1e1f24"))
    .transition(0.15)
    .hover(|s| s.bg(hex("#26282e")).translate(0.0, -1.0))
    .active(|s| s.bg(hex("#2c2e35")));
```

The default curve is `cubic-bezier(.4, 0, .2, 1)`; `easing(Easing::…)` picks another.
`animate_layout(secs)` animates position and size changes (FLIP style), which suits
indicators and reordering.

Focus rings follow `:focus-visible`: they show for keyboard focus, not for clicks. When the OS
asks for reduced motion, movement becomes instant and fades stay.

## Reusing styles

Styles are plain functions, so a mixin is a function from `Element` to `Element`:

```rust
use charis_ui::prelude::*;

fn chip<M: 'static>(e: Element<M>) -> Element<M> {
    e.px(8.0).py(2.0).pill().bg(theme().colors.hover).font_size(11.0)
}

fn tags(active: bool) -> Element<()> {
    row()
        .gap(4.0)
        .child(text("rust").apply(chip))
        .child(text("ui").apply(chip).when(active, |e| e.bg(theme().colors.accent)))
}
# let _ = tags(true);
```

`apply(f)` runs a mixin, and `when(cond, f)` runs one only if `cond` holds.

## Custom widgets

A widget is a function that returns an `Element`. Read sizes and colors from `theme()` so it
follows the app's theme, and tag it with a class so themes can restyle it:

```rust
use charis_ui::prelude::*;

fn stat<M: 'static>(label: &str, value: String) -> Element<M> {
    let th = theme();
    card()
        .gap(4.0)
        .child(text(label).font_size(th.font_size_sm).color(th.colors.text_muted))
        .child(text(value).font_size(24.0).bold())
        .class("stat")
}
# let _: Element<()> = stat("Tokens", "1,024".into());
```

For accessibility, give custom controls a role, a name and their state, and make them focusable:

```rust
# use charis_ui::prelude::*;
# #[derive(Clone)] enum Msg { ToggleWifi }
# let on = true;
let wifi: Element<Msg> =
    div().role(Role::Switch).aria_checked(on).aria_label("Wi-Fi").focusable().on_click(Msg::ToggleWifi);
```

When elements can't express something, `canvas(|cv, rect| …)` draws rounded rectangles, paths,
gradients, text and shadows with the same renderer, and `Icon::svg("M…")` turns any 24×24 SVG
path (from Lucide or Feather, say) into an icon.
