# Theming

Customization works in layers. Start at the top and go down only as far as you need:

| Layer | What it changes | Where |
|---|---|---|
| Knobs | The whole palette, radii, sizes and density, from a few settings | `ThemeConfig` |
| Tokens | Any single color, radius, size, shadow or timing | fields of `Theme` |
| Style classes | Every instance of a built-in widget, or your own named styles | `Theme::style_class` |
| Stylesheets | The same, from a CSS-like file reloaded on save | `stylesheet` module |
| Instance styling | One element | builder methods on `Element` |

Built-in widgets never hard-code a color or size: they read tokens and tag themselves with
classes, so every layer reaches them.

## Knobs

`App::theme` returns the theme. It is called every frame, so a settings screen can change it
live.

```rust
use charis_ui::prelude::*;

fn my_theme(dark: bool) -> Theme {
    let base = if dark { ThemeConfig::dark() } else { ThemeConfig::light() };
    Theme::from_config(ThemeConfig {
        accent: hex("#a371f7"), // any Color, or a preset: Accent::Teal.color(dark)
        gray: GrayTint::Slate,
        radius: 8.0,
        scaling: 1.0, // text and control size multiplier
        density: Density::Compact,
        ..base
    })
}
# let _ = my_theme(true);
```

Each seed color becomes a 12-step OKLCH scale, like Radix Colors: steps 1 and 2 are app
backgrounds, 3 to 5 component states, 6 to 8 borders, 9 and 10 solid fills, and 11 and 12 text.
The semantic palette (`colors.surface`, `.border`, `.text_muted`, `.accent`, `.focus_ring`…) is
derived from those scales, and widgets use only the semantic palette.

Text on the accent stays readable: it is white unless that falls below a 3:1 contrast ratio
(on amber or lime, say), and then turns near-black.

### High contrast

`contrast: Contrast::High` (or `theme.with_contrast(Contrast::High)`) derives a high-contrast
version of any theme: near-black or white surfaces, opaque borders, no shadows, text at 7:1 and
accents at 4.5:1 or more. `system_prefs().high_contrast` reports the OS setting, so an app can
follow it:

```rust
# use charis_ui::prelude::*;
fn theme_for_os() -> Theme {
    let prefs = system_prefs();
    let t = if prefs.dark.unwrap_or(true) { Theme::dark() } else { Theme::light() };
    if prefs.high_contrast { t.with_contrast(Contrast::High) } else { t }
}
# let _ = theme_for_os();
```

## Tokens

Every field of `Theme` is public. Override single values after generating the theme:

```rust
# use charis_ui::prelude::*;
let mut t = Theme::dark();
t.colors.background = hex("#0b0b0e");
t.shadow_popover.clear();
t.transition = 0.1;
```

Your own widgets read tokens with `theme()` while building the view, and the raw scales are
there too: `theme().scales.accent.step(3)`.

## Style classes

A style class restyles every element tagged with it, using the same builder methods as
instance styling:

```rust
use charis_ui::prelude::*;

let t = Theme::dark()
    .style_class("button", |e| e.pill().px(18.0))
    .style_class("input", |e| e.rounded(0.0))
    .style_class("sidebar", |e| e.bg(theme().colors.panel));
# let _ = t;
```

Built-in widgets tag themselves: `button` plus `button-primary` (or `-secondary`, `-ghost`,
`-danger`), `input`, `checkbox`, `switch`, `card`, `tab`, `tab-active`, `tree-row`,
`table-row`, `menu-item`, `modal`, `titlebar`, `status-bar` and more (the full list is in
`docs/CUSTOMIZING.md`). Tag your own elements with `.class("sidebar")`.

Precedence works like CSS:

1. the widget's own defaults;
2. then the theme's classes, in the order the widget applies them;
3. then whatever the app chains on that element.

So `primary_button("Go").px(4.0)` stays narrow even under the `pill().px(18.0)` class above.

## Stylesheets

A stylesheet sets the same classes from a text file:

```css
/* Rounder buttons, a flat card. */
.button { radius: 999; padding: 6 18; }
.button:hover { background: accent-soft; }
.card, .panel { background: #1b1d24; border: 1 border-strong; shadow: none; }
.tree-row:hover { background: accent / 0.18; }
```

- **Selectors:** class names, optionally with `:hover`, `:active` or `:focus`, comma-separated.
- **Colors:** `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb(…)`, `rgba(…)`, `transparent`, or a
  theme color by name (`accent`, `text-muted`, `surface`…) with an optional alpha
  (`accent / 0.3`).
- **Properties:** `background`, `color`, `border`, `border-width`, `border-color`, `radius`,
  `padding`, `margin`, `gap`, sizes, `font-size`, `font-weight`, `line-height`,
  `letter-spacing`, `opacity`, `shadow` and `transition`.

`WindowOptions::new("App").stylesheet("app.css")` watches a file and restyles the running app on
every save; errors are printed with their line number, and the last good version stays. To
apply one yourself:

```rust
use charis_ui::prelude::*;
use charis_ui::stylesheet::Stylesheet;

let (sheet, errors) = Stylesheet::parse(".button { radius: 999; }");
assert!(errors.is_empty());
let themed = sheet.apply(Theme::dark());
# let _ = themed;
```

## Fonts

Inter is bundled and used by default. `WindowOptions::system_font(true)` switches to the
platform's UI font (Segoe UI Variable, SF Pro, or the desktop font on Linux). To use your own,
load it with `WindowOptions::font(bytes)` and name it in the theme with
`font: FontFamily::Named("Geist".into())`.
