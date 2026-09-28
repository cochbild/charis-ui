# Versioning and stability policy

Charis follows [Semantic Versioning](https://semver.org/) as Cargo interprets it. This page
says what counts as the public API, what may change in which release, and how changes are
announced.

## Before 1.0 (the 0.x series)

- A **minor** release (0.1 → 0.2) may break the API. Every break is listed in `CHANGELOG.md`
  under **Breaking**, with what to change in your code.
- A **patch** release (0.2.0 → 0.2.1) never breaks the API. It carries fixes, and additions
  that can't break a build (new functions, new methods, new variants of `#[non_exhaustive]`
  enums).
- Breaking changes are batched, so there are few minor releases.

## From 1.0

- **Major** releases (1.x → 2.0) are the only ones that break the API.
- **Minor** releases add API and may raise the minimum supported Rust version.
- **Patch** releases fix bugs.
- Anything slated for removal is deprecated with `#[deprecated]` for at least one minor release
  first, with the replacement named in the deprecation note.

## What is public API

The API covered by these guarantees is everything reachable from the crate root in rustdoc,
including `charis_ui::prelude`, with these exceptions:

- Items marked `#[doc(hidden)]`, such as the `cpu` module, are internal. They are public only so
  that sibling parts of the crate, benchmarks or tests can reach them. Don't depend on them.
- **Enums marked `#[non_exhaustive]` may gain variants in any minor release**, and before 1.0 in
  any patch release. Examples are `Event`, `WindowRequest`, `Key`, `Role`, `Icon`, `Cursor`,
  `Fit`, `Backdrop`, `Damage`, `DialogKind` and `TreeEvent`. Any `match` on them needs a
  wildcard arm (`_ => {}`).
- **Message enums** that built-in components send to themselves may gain variants in a minor
  release even though they aren't marked `#[non_exhaustive]`. Examples are `DockMsg`, `ToolMsg`,
  `LayoutMsg`, `PaletteMsg`, `KeymapMsg` and `TreeMsg`. Your app stores and forwards these
  messages; it shouldn't match on them exhaustively.
- **Structs with public fields** that are also built with a `new()` or `Default` constructor may
  gain fields in a minor release. Examples are `WindowOptions`, `ThemeConfig`, `Style` and
  `Modifiers`. Build them with the constructor or with `..Default::default()`, not with a
  literal that lists every field.
- **Feature flags** are part of the API. Removing or renaming a feature is a breaking change;
  adding one isn't.

## What isn't covered

- **Pixels.** Rendering may change in any release: anti-aliasing, text gamma, default theme
  colors, shadow falloff. Golden screenshot tests in your app should use a tolerance, the way
  `tests/golden.rs` does.
- **Default theme values.** The spacing, radii and colors of the built-in theme may be tuned in
  minor releases. If you need exact values, set them in your own `Theme`.
- **Timing.** Animation curves and durations, and the frame scheduling of the runtime.
- **Debug output.** `Debug` formatting, log messages, the inspector overlay, and the text of
  error messages (including stylesheet errors).
- **Serialized data** saved with the `serde` feature (dock layouts, keymaps, workspaces) is kept
  loadable across minor releases. A release that can't read an older format says so in the
  changelog and ships a migration path.

## Minimum supported Rust version (MSRV)

The MSRV is set in `Cargo.toml` (`rust-version`). Raising it is allowed in a minor release (a
0.x minor release before 1.0) and is always listed in the changelog. The MSRV is never newer than
the stable Rust release from six months earlier.

## Dependencies in the public API

A few types from dependencies show up in Charis's API: `wgpu` types (in the `gpu` module) and
`tiny_skia` pixmaps (`Runtime::render` and the headless screenshots). Moving to a new major
version of one of these is a breaking change for Charis and follows the rules above.
