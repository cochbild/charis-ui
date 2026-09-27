# Contributing to Charis

Thanks for helping. Bug reports, fixes, docs, widgets and platform QA are all welcome. This page
covers how to build, test and submit a change.

By taking part you agree to follow the [code of conduct](CODE_OF_CONDUCT.md).

## Getting set up

You need Rust 1.89 or newer (see `rust-version` in `Cargo.toml`). On Linux you also need the
usual windowing libraries:

```sh
# Debian / Ubuntu
sudo apt install libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxi-dev libxrandr-dev
```

Then run the examples:

```sh
cargo run --example showcase     # every widget, light and dark
cargo run --example dock         # the IDE shell: docking, tool windows, commands
cargo run --example gallery      # one page per widget
```

Useful environment variables while working:

| Variable | Effect |
|---|---|
| `CHARIS_RENDERER=cpu` | Use the CPU renderer instead of wgpu. |
| `CHARIS_PROFILE=1` | Print per-frame timings. |
| `CHARIS_NO_DAMAGE=1` | Turn off CPU damage tracking (full redraw every frame). |
| `CHARIS_DARK=1`, `CHARIS_HIGH_CONTRAST=1`, `CHARIS_REDUCED_MOTION=1` | Override the OS preferences. |
| `CHARIS_DEBUG_EVENTS=1` | Log window events. |
| `WGPU_ADAPTER_NAME=…` | Render with the GPU whose name contains this (e.g. `Microsoft Basic Render Driver` for WARP). |

In debug builds, F12 (or Ctrl+Shift+I, or Cmd+Alt+I on macOS) opens the element inspector.

## Before you open a pull request

Run the same checks the maintainers run:

```sh
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --no-default-features --lib
cargo doc --no-deps --all-features
```

Platform code (`src/platform`, the `cfg(windows)` and `cfg(target_os = "macos")` parts of
`src/window.rs`) should at least pass clippy for the other targets:

```sh
rustup target add x86_64-pc-windows-msvc aarch64-apple-darwin
cargo clippy --all-features --target x86_64-pc-windows-msvc -- -D warnings
cargo clippy --all-features --target aarch64-apple-darwin -- -D warnings
```

### Tests

- **Every feature ships with a headless test.** `Headless` drives an app without a window:
  send messages, click and type, advance time, and check pixels, layout rects or the
  accessibility tree. The files in `tests/` show the patterns; the book's testing chapter
  explains them.
- **Golden screenshots** (`tests/golden.rs`) compare against the PNGs in `tests/golden/` with a
  small tolerance. If you changed rendering on purpose, regenerate them with
  `UPDATE_GOLDEN=1 cargo test --test golden` and look at every changed image before committing.
- **GPU parity** (`tests/gpu.rs`) renders the same scenes with wgpu and the CPU renderer and
  compares them. It skips itself when no adapter is available. On Linux, Mesa's lavapipe
  (`mesa-vulkan-drivers`) is enough.
- **Benchmarks:** `cargo bench --bench frames` prints frame times against the budgets in
  `docs/PERFORMANCE.md`. Run it before and after a change to the runtime, layout or renderers.

### Style

- `rustfmt.toml` sets a 120-column width. Keep the formatting `cargo fmt` gives.
- No `unwrap`/`expect` in library code on anything that depends on user input, the OS or the
  GPU. Where one is truly impossible to hit, say why in a comment.
- Every public item gets a doc comment (the crate has `#![warn(missing_docs)]`). Add a short
  example to anything a user calls directly.
- New enums that may grow get `#[non_exhaustive]`; see [docs/SEMVER.md](docs/SEMVER.md).
- Built-in widgets read their colors, sizes and radii from the `Theme`, never from literals,
  and support a style class (`.class("…")`), so apps can restyle them.
- Keep accessibility in mind: give custom widgets a `Role` and a label, and make them reachable
  from the keyboard.

### Commits and pull requests

- Keep a pull request to one topic, and explain the why in its description.
- Add a line to the `Unreleased` section of `CHANGELOG.md` for anything a user would notice. List
  breaking changes under **Breaking**, with what to change.
- A change to rendering, input or windows should say which platforms you tried it on. The QA
  checklists are in `docs/WINDOWS_QA.md` and `docs/MAC_LINUX_QA.md`.

## Reporting bugs

Please include your OS and version, the renderer (GPU adapter name, or `CHARIS_RENDERER=cpu`), the
display scale, and the smallest app that shows the problem. A failing headless test is the best
possible bug report.

## License

Charis is dual-licensed under MIT or Apache-2.0. Unless you say otherwise, any contribution you
submit for inclusion is licensed the same way, without any additional terms or conditions.
