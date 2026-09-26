//! Data tables: fixed and weighted columns, a header that stays put, sort
//! indicators, user-resizable columns, and a virtualized body.
//!
//! ```no_run
//! # use rust_ui::prelude::*;
//! # #[derive(Clone)] enum Msg { Sort(usize, SortDir), Select(usize) }
//! # let models: Vec<(String, u64)> = vec![];
//! let rows = std::rc::Rc::new(models);
//! let data = rows.clone();
//! let t: Element<Msg> = table(
//!     "models",
//!     vec![Column::new("Model").weight(3.0).sortable(), Column::new("Size").fixed(90.0).align_end().sortable()],
//!     rows.len(),
//!     move |r, c| match c {
//!         0 => cell_text(data[r].0.clone()),
//!         _ => cell_text(format!("{} GB", data[r].1)),
//!     },
//! )
//! .sort(0, SortDir::Asc, Msg::Sort)
//! .on_row_click(Msg::Select)
//! .into();
//! ```

use std::rc::Rc;

use crate::element::*;
use crate::icons::Icon;
use crate::semantics::Role;
use crate::style::*;
use crate::theme::theme;

/// How a column takes up width.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColumnWidth {
    /// Exactly this many pixels.
    Fixed(f32),
    /// A share of the remaining width (like CSS `flex-grow` / iced `FillPortion`).
    Weight(f32),
}

/// A table column definition.
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub title: String,
    pub width: ColumnWidth,
    pub min_width: f32,
    pub align: Justify,
    pub sortable: bool,
    pub resizable: bool,
}

impl Column {
    /// A weighted (weight 1) column that is resizable but not sortable.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            width: ColumnWidth::Weight(1.0),
            min_width: 48.0,
            align: Justify::Start,
            sortable: false,
            resizable: true,
        }
    }
    pub fn fixed(mut self, px: f32) -> Self {
        self.width = ColumnWidth::Fixed(px);
        self
    }
    pub fn weight(mut self, w: f32) -> Self {
        self.width = ColumnWidth::Weight(w.max(0.0));
        self
    }
    pub fn min_width(mut self, px: f32) -> Self {
        self.min_width = px;
        self
    }
    /// Right-align the column (numbers, sizes).
    pub fn align_end(mut self) -> Self {
        self.align = Justify::End;
        self
    }
    pub fn align_center(mut self) -> Self {
        self.align = Justify::Center;
        self
    }
    /// Clicking the header emits the table's `on_sort` message.
    pub fn sortable(mut self) -> Self {
        self.sortable = true;
        self
    }
    pub fn resizable(mut self, r: bool) -> Self {
        self.resizable = r;
        self
    }
}

/// Sort direction shown in a column header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SortDir {
    Asc,
    Desc,
}

impl SortDir {
    pub fn flip(self) -> Self {
        match self {
            SortDir::Asc => SortDir::Desc,
            SortDir::Desc => SortDir::Asc,
        }
    }
}

type CellFn<M> = Rc<dyn Fn(usize, usize) -> Element<M>>;

/// Builder returned by [`table`]. Converts into an [`Element`].
pub struct Table<M> {
    id: String,
    columns: Vec<Column>,
    rows: usize,
    cell: CellFn<M>,
    sort: Option<(usize, SortDir)>,
    on_sort: Option<Rc<dyn Fn(usize, SortDir) -> M>>,
    on_row_click: Option<Rc<dyn Fn(usize) -> M>>,
    selected: Option<usize>,
    striped: bool,
    row_height: f32,
    empty: Option<Element<M>>,
}

/// A data table with `rows` rows. `cell(row, col)` builds each visible cell
/// on demand, so tables with 100k+ rows stay fast. The `id` identifies the
/// table's retained state (column widths, scroll) and doubles as the body's
/// scroll id for `cx.scroll_to_item(id, row)`.
///
/// Columns can be resized by dragging the header dividers (double-click resets).
/// Widths are kept by the runtime, so the app needs no state for it.
pub fn table<M: 'static>(
    id: &str,
    columns: Vec<Column>,
    rows: usize,
    cell: impl Fn(usize, usize) -> Element<M> + 'static,
) -> Table<M> {
    Table {
        id: id.to_string(),
        columns,
        rows,
        cell: Rc::new(cell),
        sort: None,
        on_sort: None,
        on_row_click: None,
        selected: None,
        striped: false,
        row_height: 34.0,
        empty: None,
    }
}

/// A single-line cell label that ellipsizes when its column is too narrow.
pub fn cell_text<M: 'static>(s: impl Into<String>) -> Element<M> {
    text(s).nowrap().ellipsis().min_w(0.0)
}

impl<M: 'static> Table<M> {
    /// Show the sort indicator on `col` and emit `f(col, dir)` when a sortable
    /// header is clicked (same column flips the direction; a new one starts
    /// ascending). The app sorts its data and passes the new state back.
    pub fn sort(mut self, col: usize, dir: SortDir, f: impl Fn(usize, SortDir) -> M + 'static) -> Self {
        self.sort = Some((col, dir));
        self.on_sort = Some(Rc::new(f));
        self
    }
    /// Emit `f(col, dir)` on header clicks without a current sort column.
    pub fn on_sort(mut self, f: impl Fn(usize, SortDir) -> M + 'static) -> Self {
        self.on_sort = Some(Rc::new(f));
        self
    }
    pub fn on_row_click(mut self, f: impl Fn(usize) -> M + 'static) -> Self {
        self.on_row_click = Some(Rc::new(f));
        self
    }
    /// Highlight a row as selected.
    pub fn selected(mut self, row: Option<usize>) -> Self {
        self.selected = row;
        self
    }
    /// Alternate row backgrounds.
    pub fn striped(mut self, s: bool) -> Self {
        self.striped = s;
        self
    }
    /// Row height (every row has this exact height, which keeps scrolling exact).
    pub fn row_height(mut self, h: f32) -> Self {
        self.row_height = h.max(8.0);
        self
    }
    /// Shown instead of the body when there are no rows.
    pub fn empty_state(mut self, el: Element<M>) -> Self {
        self.empty = Some(el);
        self
    }

    fn build(self) -> Element<M> {
        let th = theme();
        let c = th.colors.clone();
        let tid = global_id(&self.id);
        let cols = Rc::new(self.columns);
        // Cell wrapper shared by the header and body so their widths can't drift.
        let sized = {
            let cols = cols.clone();
            move |e: Element<M>, ci: usize| -> Element<M> {
                let col = &cols[ci];
                let mut w = row().items_center().justify(col.align).px(10.0).min_w(col.min_width).clip();
                w = match col.width {
                    ColumnWidth::Fixed(px) => w.w(px).shrink(0.0),
                    ColumnWidth::Weight(g) => w.grow(g).shrink(1.0).basis(0.0),
                };
                w.behavior = Behavior::TableCell { table: tid, col: ci };
                w.role(Role::Cell).child(e)
            }
        };

        let mut header =
            row().role(Role::Row).h(th.row_height + 8.0).shrink(0.0).px(10.0).border_b(1.0, c.border).bg(c.panel);
        for (ci, col) in cols.iter().enumerate() {
            let sorted = self.sort.filter(|(sc, _)| *sc == ci).map(|(_, d)| d);
            let mut label = row()
                .items_center()
                .gap(4.0)
                .min_w(0.0)
                .font_size(th.font_size_sm)
                .medium()
                .color(if sorted.is_some() { c.text } else { c.text_muted })
                .child(cell_text(col.title.clone()));
            if let Some(d) = sorted {
                label = label.child(
                    icon(if d == SortDir::Asc { Icon::ChevronUp } else { Icon::ChevronDown })
                        .font_size(12.0)
                        .color(c.accent),
                );
            }
            let mut cell = sized(label, ci).h_full().role(Role::ColumnHeader);
            if let Some(d) = sorted {
                cell = cell.aria_description(if d == SortDir::Asc { "sorted ascending" } else { "sorted descending" });
            }
            if col.sortable {
                if let Some(f) = &self.on_sort {
                    let next = match sorted {
                        Some(d) => d.flip(),
                        None => SortDir::Asc,
                    };
                    cell = cell.transition(0.1).hover(|s| s.bg(c.hover)).on_click(f(ci, next));
                }
            }
            // The last column has nothing after it to resize against.
            if col.resizable && ci + 1 < cols.len() {
                let mut grip = div()
                    .absolute()
                    .top(6.0)
                    .bottom(6.0)
                    .right(0.0)
                    .w(7.0)
                    .center()
                    .cursor(Cursor::ResizeCol)
                    .child(div().w(1.0).h_full().bg(c.border_strong));
                grip.behavior = Behavior::ColumnResize { table: tid, col: ci, min: col.min_width };
                grip = grip.aria_label(format!("Resize {} column", col.title));
                cell = cell.child(grip);
            }
            header = header.child(cell);
        }

        let body: Element<M> = if self.rows == 0 && self.empty.is_some() {
            div().grow(1.0).center().p(24.0).color(c.text_faint).children(self.empty)
        } else {
            let cell_fn = self.cell.clone();
            let ncols = cols.len();
            let (sel, striped, rh, click) = (self.selected, self.striped, self.row_height, self.on_row_click.clone());
            let cc = c.clone();
            let radius = th.radius_sm;
            virtual_list(self.rows, move |r| {
                let mut tr = row()
                    .role(Role::Row)
                    .aria_selected(sel == Some(r))
                    .h(rh)
                    .px(6.0)
                    .items_center()
                    .rounded(radius)
                    .transition(0.08)
                    .when(striped && r % 2 == 1, |e| e.bg(cc.hover.with_alpha(cc.hover.a * 0.6)))
                    .hover(|s| s.bg(cc.hover));
                if sel == Some(r) {
                    tr = tr.bg(cc.accent_soft).hover(|s| s.bg(cc.accent_soft));
                }
                if let Some(f) = &click {
                    tr = tr.on_click(f(r));
                }
                for ci in 0..ncols {
                    tr = tr.child(sized(cell_fn(r, ci), ci));
                }
                tr
            })
            .id(&self.id)
            .item_height(rh)
            .p(4.0)
            .gap(1.0)
            .grow(1.0)
        };
        col().role(Role::Table).min_h(0.0).min_w(0.0).child(header).child(body)
    }
}

impl<M: 'static> From<Table<M>> for Element<M> {
    fn from(t: Table<M>) -> Self {
        t.build()
    }
}
