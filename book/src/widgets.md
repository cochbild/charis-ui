# Widgets

Every widget is a function that returns an ordinary `Element`, so you can restyle any of them
with the same builder methods as a `div()`. They read their colors, sizes and radii from the
[theme](theming.md).

## The built-in set

- **Buttons:** `button`, `primary_button`, `ghost_button`, `danger_button` and `icon_button`.
  Add an icon with `.with_icon(Icon::Check)`.
- **Text entry:** `text_input`, `text_area`, `search_input` and `number_input`. Text inputs
  handle selection, word navigation, the clipboard, undo and redo, IME composition and password
  masking. `text_area` stays fast on documents with hundreds of thousands of lines.
- **Choices:** `checkbox`, `switch`, `radio_group`, `pick_list`, `combo_box`, `segmented`,
  `tab_bar` and `slider`.
- **Lists:** `list_item`, `tree_row`, `virtual_list`, trees and tables (below).
- **Menus and overlays:** `menubar`, `menu_panel`, `context_menu`, `modal`, `backdrop`, and
  `.tooltip(..)` on any element.
- **Window chrome:** `titlebar`, `window_controls`, `status_bar` and `status_item`. See
  [Windows and platforms](windows.md).
- **Display:** `text`, `rich_text`, `markdown`, `card`, `badge`, `tag`, `kbd`, `avatar`,
  `progress`, `section_header` and `separator`.
- **Media:** `image` and `svg`, with `.fit(Fit::Cover)`, `.rounded()` and `.tint()`.

```rust
# use charis_ui::prelude::*;
# #[derive(Clone)] enum Msg { Save, Wrap(bool) }
# fn toolbar(wrap: bool) -> Element<Msg> {
row()
    .gap(8.0)
    .items_center()
    .child(primary_button("Save").with_icon(Icon::Check).on_click(Msg::Save))
    .child(checkbox("Word wrap", wrap).on_click(Msg::Wrap(!wrap)))
    .child(kbd("Ctrl+S"))
    .into()
# }
# let _ = toolbar(true);
```

The `gallery` example shows every widget, one page each:

```sh
cargo run --release --example gallery
```

## Long lists

`virtual_list(count, |i| row)` builds only the rows near the viewport, so a list of 200,000
rows costs about the same per frame as one of 100.

```rust
# use charis_ui::prelude::*;
let names: Vec<String> = (0..100_000).map(|i| format!("Item {i}")).collect();
let list: Element<()> = virtual_list(names.len(), move |i| text(names[i].clone()).px(12.0));
```

Rows can have different heights. Each row is measured the first time it's shown, and the
scroll position stays anchored while estimates are replaced by real heights. Lists support
`.follow_end()` (keep the last row visible as rows arrive, as in a log or chat),
`.on_scroll()`, `cx.scroll_to_end(id)` and `cx.scroll_to_item(id, i)`.

## Trees

Implement `TreeModel` for your data and keep a `TreeState` in your app. The tree asks for a
node's children only when the node is expanded, so the model can load them lazily from a file
system or a database. Like `virtual_list`, it builds only the rows on screen.

```rust
use std::rc::Rc;
use charis_ui::prelude::*;
use charis_ui::tree::{TreeEvent, TreeModel, TreeMsg, TreeState};

struct Files;
impl TreeModel for Files {
    type Id = String;
    fn children(&self, parent: Option<&String>) -> Vec<String> {
        match parent {
            None => vec!["src".into()],
            Some(_) => vec!["src/main.rs".into()],
        }
    }
    fn has_children(&self, id: &String) -> bool {
        id == "src"
    }
    fn label(&self, id: &String) -> String {
        id.rsplit('/').next().unwrap_or(id).into()
    }
}

struct Explorer {
    files: Rc<Files>,
    tree: TreeState<String>,
}

#[derive(Clone)]
enum Msg {
    Tree(TreeMsg<String>),
}

impl App for Explorer {
    type Msg = Msg;
    fn update(&mut self, msg: Msg, cx: &mut Cx<Msg>) {
        let Msg::Tree(m) = msg;
        if let Some(TreeEvent::Activated(path)) = self.tree.update(m, &*self.files, cx) {
            println!("open {path}");
        }
    }
    fn view(&self) -> Element<Msg> {
        self.tree.view(&self.files, Msg::Tree)
    }
}
# let _ = Explorer { files: Rc::new(Files), tree: TreeState::new("files") };
```

`tree.update` reports `Selected`, `Activated`, `Expanded` and `Collapsed`, and scrolls the
selection into view. Trees have indent guides, chevrons that toggle, and double-click to open.
The keyboard works like VS Code's explorer:

- ↑ and ↓ move the selection;
- → expands a node or moves to its first child, and ← collapses it or moves to its parent;
- Home, End, PageUp and PageDown jump;
- Enter activates and Space toggles;
- typing jumps to a matching label.

## Tables

`table(id, columns, rows, |row, col| cell)` is a data grid with a header that stays put and a
virtualized body.

```rust
# use charis_ui::prelude::*;
#[derive(Clone)]
enum Msg {
    Sort(usize, SortDir),
    Select(usize),
}

let models = std::rc::Rc::new(vec![("llama-3-8b".to_string(), 5u64), ("qwen-7b".to_string(), 4)]);
let data = models.clone();
let t: Element<Msg> = table(
    "models",
    vec![
        Column::new("Model").weight(3.0).sortable(),
        Column::new("Size").fixed(90.0).align_end().sortable(),
    ],
    models.len(),
    move |r, c| match c {
        0 => cell_text(data[r].0.clone()),
        _ => cell_text(format!("{} GB", data[r].1)),
    },
)
.sort(0, SortDir::Asc, Msg::Sort)
.on_row_click(Msg::Select)
.into();
```

Columns are fixed (`.fixed(px)`) or take a share of the remaining width (`.weight(w)`). Rows
are clickable and can be selected and striped, and an empty table shows an empty state. Users
resize columns by dragging the header dividers, and double-clicking a divider resets it. The
runtime keeps those widths, so your app needs no state for them; read or restore them with
`Runtime::column_widths`.

## Icons and custom drawing

Icons are vectors, so they stay sharp at any scale. There's a built-in set (`Icon::Folder`,
`Icon::Settings` and so on), and any 24×24 SVG path works through `Icon::svg("M…")`, so you
can paste in icons from sets such as Lucide or Feather.

For anything the widgets don't cover, `canvas(|cv, rect| …)` draws directly: shapes, paths,
text and images. See [Styling](styling.md#custom-widgets) for building your own widgets from
elements.
