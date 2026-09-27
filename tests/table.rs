//! Data table: header/body alignment, sorting, selection, column resizing.

use std::rc::Rc;

use charis_ui::prelude::*;

struct Models {
    rows: Rc<Vec<(String, u32)>>,
    sort: (usize, SortDir),
    selected: Option<usize>,
    log: Vec<String>,
}

#[derive(Clone, Debug)]
enum Msg {
    Sort(usize, SortDir),
    Select(usize),
}

impl App for Models {
    type Msg = Msg;
    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        self.log.push(format!("{msg:?}"));
        match msg {
            Msg::Sort(c, d) => self.sort = (c, d),
            Msg::Select(r) => self.selected = Some(r),
        }
    }
    fn view(&self) -> Element<Msg> {
        let data = self.rows.clone();
        let t = table(
            "models",
            vec![
                Column::new("Name").weight(3.0).sortable(),
                Column::new("Publisher").weight(2.0),
                Column::new("Size").fixed(90.0).align_end().sortable(),
            ],
            data.len(),
            move |r, c| match c {
                0 => cell_text(data[r].0.clone()),
                1 => cell_text(format!("pub{}", r % 7)),
                _ => cell_text(format!("{} MB", data[r].1)),
            },
        )
        .sort(self.sort.0, self.sort.1, Msg::Sort)
        .on_row_click(Msg::Select)
        .selected(self.selected)
        .empty_state(text("No models"));
        col().size_full().child(Element::from(t).grow(1.0))
    }
}

fn app(n: usize) -> Models {
    Models {
        rows: Rc::new((0..n).map(|i| (format!("model-{i}"), 100 + i as u32)).collect()),
        sort: (0, SortDir::Asc),
        selected: None,
        log: vec![],
    }
}

fn x_of(h: &Headless<Models>, s: &str) -> f32 {
    h.rt.rect_of_text(s).unwrap_or_else(|| panic!("{s} not found")).x
}

#[test]
fn header_and_body_columns_line_up() {
    let mut h = Headless::new(app(10_000), 800.0, 500.0, 1.0);
    h.settle();
    assert!((x_of(&h, "Name") - x_of(&h, "model-0")).abs() < 0.5);
    assert!((x_of(&h, "Publisher") - x_of(&h, "pub0")).abs() < 0.5);
    // Right-aligned column: right edges match.
    let head = h.rt.rect_of_text("Size").unwrap();
    let cell = h.rt.rect_of_text("100 MB").unwrap();
    assert!((head.x + head.w - (cell.x + cell.w)).abs() < 14.0, "Size header has a sort icon; roughly aligned");
    // Weighted columns share the width 3:2 after the fixed column.
    let name_w = x_of(&h, "Publisher") - x_of(&h, "Name");
    let pub_w = head.x + head.w + 10.0 - 90.0 - x_of(&h, "Publisher") + 10.0;
    assert!((name_w / pub_w - 1.5).abs() < 0.1, "weights: {name_w} vs {pub_w}");
    assert!(h.rt.virtual_rows_built("models").unwrap() < 40);
}

#[test]
fn sorting_and_row_selection_emit_messages() {
    let mut h = Headless::new(app(100), 800.0, 500.0, 1.0);
    h.settle();
    let name = h.rt.rect_of_text("Name").unwrap();
    h.click(name.x + 5.0, name.y + 5.0);
    assert_eq!(h.rt.app.sort, (0, SortDir::Desc), "same column flips");
    let size = h.rt.rect_of_text("Size").unwrap();
    h.click(size.x + 5.0, size.y + 5.0);
    assert_eq!(h.rt.app.sort, (2, SortDir::Asc), "new column starts ascending");
    // Publisher isn't sortable.
    let publ = h.rt.rect_of_text("Publisher").unwrap();
    h.click(publ.x + 5.0, publ.y + 5.0);
    assert_eq!(h.rt.app.sort, (2, SortDir::Asc));

    let r = h.rt.rect_of_text("model-3").unwrap();
    h.click(r.x + 200.0, r.y + r.h / 2.0);
    assert_eq!(h.rt.app.selected, Some(3));
}

#[test]
fn dragging_a_header_divider_resizes_the_column_and_double_click_resets() {
    let mut h = Headless::new(app(100), 800.0, 500.0, 1.0);
    h.settle();
    let before = x_of(&h, "Publisher");
    let head = h.rt.rect_of_text("Publisher").unwrap();
    // The divider between Name and Publisher is at the Name cell's right edge.
    let grip = (before - 10.0 - 3.5, head.y + head.h / 2.0);
    h.drag(grip, (grip.0 + 60.0, grip.1), 6);
    h.settle();
    let after = x_of(&h, "Publisher");
    assert!((after - before - 60.0).abs() < 1.0, "Publisher moved {} (want 60)", after - before);
    assert!((x_of(&h, "pub0") - after).abs() < 0.5, "body cells follow the header");
    assert_eq!(h.rt.app.sort, (0, SortDir::Asc), "resizing must not sort");
    let saved = h.rt.column_widths("models");
    assert!(saved[0].is_some() && saved.get(1).copied().flatten().is_none(), "{saved:?}");

    let grip = (after - 10.0 - 3.5, grip.1);
    h.click(grip.0, grip.1);
    h.click(grip.0, grip.1);
    h.settle();
    assert!((x_of(&h, "Publisher") - before).abs() < 0.5, "double-click resets the width");
}

#[test]
fn empty_table_shows_the_empty_state() {
    let mut h = Headless::new(app(0), 800.0, 500.0, 1.0);
    h.settle();
    assert!(h.rt.rect_of_text("No models").is_some());
    assert!(h.rt.rect_of_text("Name").is_some(), "header still shown");
}
