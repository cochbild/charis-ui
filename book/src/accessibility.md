# Accessibility

Charis apps work with screen readers, keyboards, high-contrast themes and reduced motion
without extra work for the built-in widgets. This chapter covers what you get and what to do
for your own widgets.

## Screen readers

With the `accessibility` feature (on by default), screen readers see the UI through
[AccessKit](https://accesskit.dev):

- **Windows:** UI Automation, for Narrator, NVDA and JAWS;
- **macOS:** NSAccessibility, for VoiceOver;
- **Linux:** AT-SPI, for Orca.

Built-in widgets expose their role, name and state. A checkbox is a checked or unchecked
"Remember me" check box, a slider has its value and range, and a dialog is modal. Text inside
a button becomes the button's name rather than a separate node, and an icon-only button is
named by its tooltip.

Screen-reader actions (activate, focus, set a value, increment, scroll) run through the same
code as mouse and keyboard input, so a control that works with the mouse works with a screen
reader too.

## Custom widgets

Give a custom control a role, a name and its state with the ARIA-style methods, and make it
focusable so keyboard users can reach it:

```rust
# use charis_ui::prelude::*;
# #[derive(Clone)] enum Msg { ToggleWifi }
# let on = true;
let wifi: Element<Msg> =
    div().role(Role::Switch).aria_checked(on).aria_label("Wi-Fi").focusable().on_click(Msg::ToggleWifi);
```

The methods are `.role()`, `.aria_label()`, `.aria_description()`, `.aria_checked()`,
`.aria_selected()`, `.aria_expanded()`, `.aria_value(value, min, max)`, `.aria_modal()` and
`.aria_hidden()`.

To check the result, `Runtime::accessibility_tree()` returns the tree for the current frame, so
headless tests can assert on roles, names and states. See [Testing](testing.md#accessibility).

## Keyboard

Tab and Shift+Tab move focus through focusable elements, and a focus ring appears only for
keyboard focus, like the web's `:focus-visible`. Space and Enter activate buttons, checkboxes
and switches, and the arrow keys work in radio groups, sliders, lists, trees, tables and
splitters.

## High contrast

`ThemeConfig { contrast: Contrast::High, .. }`, or `theme.with_contrast(Contrast::High)`,
derives a high-contrast version of any theme, in the style of Windows contrast themes:

- near-black or white surfaces;
- opaque borders, and no shadows;
- text at a 7:1 contrast ratio, and accents and state colors at 4.5:1 or more.

The crate's tests check these ratios for every accent, gray tint and mode.
`system_prefs().high_contrast` reports the OS setting, so an app can follow it; see
[Theming](theming.md#high-contrast) for an example.

## Reduced motion

The runtime follows the OS setting: "Animation effects" on Windows, "Reduce motion" on macOS
and "Animations" on GNOME. When it's on:

- smooth scrolling, pane slides, layout animations and `translate` transitions become instant;
- color and opacity fades still fade, because they don't move anything.

Custom animations can check `anim::reduced_motion()`, and `anim::set_reduced_motion(Some(true))`
overrides the OS setting from an in-app preference (`None` follows the OS again).
