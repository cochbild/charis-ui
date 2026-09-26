# {{project-name}}

A settings app built on [rust-ui](https://github.com/cochbild/rust-ui):

- sections in a sidebar, with a search box that filters them;
- forms built from switches, dropdowns, segmented controls, sliders, number inputs and text
  fields;
- the theme (mode, accent, radius, density, contrast) applies live as you change it;
- every change is saved to `settings.txt` in the user's config directory, and read on start;
- a reset with a confirmation dialog.

```sh
cargo run --release
```

Where to start in `src/main.rs`:

- `Settings` holds the values; `from_text` and `to_text` are the file format.
- `Change` is one edit; `Settings::apply` applies it.
- `Section` lists the pages, and `Prefs::section_view` builds each one from `group` and
  `setting` rows.
