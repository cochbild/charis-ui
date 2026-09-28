# Introduction

Charis is a desktop UI framework for Rust that borrows the good parts of the web: CSS-like
styling, flexbox and grid layout, design tokens and transitions. It adds the pieces that
IDE-class apps need and most Rust toolkits lack: docking with tabs torn out into their own
windows, JetBrains-style tool windows, named layouts, commands with a rebindable keymap, and
frameless windows that keep their native behavior.

This book is the guide. The API reference is the rustdoc (`cargo doc --open`), and every public
item in it is documented.

## What you get

- **A declarative UI.** Your `view` returns a tree of `Element`s built with chained methods; the
  runtime diffs, lays out and draws it. State lives in your app struct, and changes arrive as
  messages (the Elm pattern).
- **Styling like CSS.** Padding, margins, borders, per-corner radii, blurred shadows, gradients,
  hover and press states, and transitions.
- **Themes built from a few knobs.** An accent color, a gray tint, a radius, a density and a
  scale generate every color and size, in light, dark and high-contrast versions. Style classes
  and hot-reloaded stylesheets restyle built-in widgets.
- **Panels.** Splits, docking, tool windows and workspaces, all serializable.
- **Two renderers.** wgpu on the GPU by default, and a CPU renderer on tiny-skia as a fallback.
  Both produce the same pixels, which a test checks.
- **Testability.** A headless runtime drives your app without a window, so tests can click,
  type, drag, read the layout, check pixels and inspect the accessibility tree.

## How this book is organized

1. [Getting started](getting-started.md) builds and runs a first app.
2. [The app model](app-model.md) covers messages, effects, async work, components and multiple
   windows.
3. [Layout](layout.md) and [Styling](styling.md) cover the element tree.
4. [Theming](theming.md) covers tokens, style classes and stylesheets.
5. [Widgets](widgets.md) covers the built-in widgets, long lists, trees and tables.
6. [Panels and docking](panels.md) and [Menus and commands](commands.md) cover the IDE-shell
   features.
7. [Windows and platforms](windows.md) covers window options and per-platform behavior.
8. [Accessibility](accessibility.md) covers screen readers, the keyboard, high contrast and
   reduced motion.
9. [Testing](testing.md) and [Performance](performance.md) cover keeping an app correct and
   fast.

The Rust examples in this book are compiled and run as part of the crate's test suite, so they
stay in step with the API.
