//! The virtualized tree: only visible rows are built, children are asked
//! for only when expanded, keyboard navigation and scrolling into view,
//! mouse toggling and activation.

use std::cell::Cell;
use std::rc::Rc;

use charis_ui::prelude::*;
use charis_ui::tree::{TreeEvent, TreeModel, TreeMsg, TreeState};
use charis_ui::Event;

/// `roots` folders ("Folder r", ids 1..=roots) of `fanout` files each
/// ("file r.j", ids r * 1_000_000 + j).
struct Big {
    roots: u64,
    fanout: u64,
    labels: Cell<usize>,
    children_calls: Cell<usize>,
}

impl TreeModel for Big {
    type Id = u64;
    fn children(&self, parent: Option<&u64>) -> Vec<u64> {
        self.children_calls.set(self.children_calls.get() + 1);
        match parent {
            None => (1..=self.roots).collect(),
            Some(&p) if p < 1_000_000 => (0..self.fanout).map(|j| p * 1_000_000 + j).collect(),
            Some(_) => Vec::new(),
        }
    }
    fn has_children(&self, id: &u64) -> bool {
        *id < 1_000_000
    }
    fn label(&self, id: &u64) -> String {
        self.labels.set(self.labels.get() + 1);
        if *id < 1_000_000 {
            format!("Folder {id}")
        } else {
            format!("file {}.{}", id / 1_000_000, id % 1_000_000)
        }
    }
    fn icon(&self, id: &u64, expanded: bool) -> Option<Icon> {
        Some(match (*id < 1_000_000, expanded) {
            (true, _) => Icon::Folder,
            _ => Icon::File,
        })
    }
}

struct T {
    model: Rc<Big>,
    tree: TreeState<u64>,
    events: Vec<TreeEvent<u64>>,
}

#[derive(Clone)]
enum Msg {
    Tree(TreeMsg<u64>),
}

impl App for T {
    type Msg = Msg;
    fn update(&mut self, msg: Msg, cx: &mut Cx<Msg>) {
        let Msg::Tree(m) = msg;
        if let Some(e) = self.tree.update(m, &*self.model, cx) {
            self.events.push(e);
        }
    }
    fn view(&self) -> Element<Msg> {
        col().size_full().child(self.tree.view(&self.model, Msg::Tree))
    }
}

fn setup(roots: u64, fanout: u64) -> Headless<T> {
    let model = Rc::new(Big { roots, fanout, labels: Cell::new(0), children_calls: Cell::new(0) });
    let mut h = Headless::new(T { model, tree: TreeState::new("tree"), events: Vec::new() }, 600.0, 400.0, 1.0);
    h.settle();
    h
}

fn key(h: &mut Headless<T>, key: Key) {
    h.event(Event::Key(KeyEvent { key, mods: Modifiers::default(), repeat: false }));
    h.settle();
}

fn selected(h: &Headless<T>) -> Option<u64> {
    h.rt.app.tree.selected().copied()
}

#[test]
fn only_visible_rows_are_built() {
    let mut h = setup(100, 1000);
    assert_eq!(h.rt.app.model.children_calls.get(), 1, "only the roots so far");
    for r in 1..=100 {
        h.rt.app.tree.expand(r);
    }
    h.rt.invalidate();
    h.rt.app.model.labels.set(0);
    let t0 = std::time::Instant::now();
    h.settle();
    let built = h.rt.app.model.labels.get();
    assert_eq!(h.rt.app.tree.rows(&*h.rt.app.model).len(), 100_100);
    assert!(built < 120, "labels built for {built} rows, not 100k");
    eprintln!("100k-row tree: first frame {:?}", t0.elapsed());
    // Jump to the end: still only a screenful is built.
    h.rt.app.model.labels.set(0);
    h.rt.scroll_to_item("tree", 100_050);
    h.settle();
    assert!(h.rt.app.model.labels.get() < 120);
    assert!(h.rt.rect_of_text("file 100.950").is_some());
}

#[test]
fn keyboard_navigation() {
    let mut h = setup(5, 3);
    // Focus the tree by clicking its first row.
    let r = h.rt.rect_of_text("Folder 1").unwrap().center();
    h.click(r.x, r.y);
    h.settle();
    assert_eq!(selected(&h), Some(1));
    key(&mut h, Key::Down);
    assert_eq!(selected(&h), Some(2));
    key(&mut h, Key::Right); // expand
    assert!(h.rt.app.tree.is_expanded(&2));
    assert!(h.rt.rect_of_text("file 2.0").is_some());
    key(&mut h, Key::Right); // first child
    assert_eq!(selected(&h), Some(2_000_000));
    key(&mut h, Key::Down);
    key(&mut h, Key::Left); // to the parent
    assert_eq!(selected(&h), Some(2));
    key(&mut h, Key::Left); // collapse
    assert!(!h.rt.app.tree.is_expanded(&2));
    key(&mut h, Key::End);
    assert_eq!(selected(&h), Some(5));
    key(&mut h, Key::Home);
    assert_eq!(selected(&h), Some(1));
    // Type-ahead: "f" cycles through the folders; Enter toggles a folder.
    key(&mut h, Key::Char('f'));
    assert_eq!(selected(&h), Some(2));
    key(&mut h, Key::Enter);
    assert!(h.rt.app.tree.is_expanded(&2));
    key(&mut h, Key::Down);
    key(&mut h, Key::Enter);
    assert_eq!(h.rt.app.events.last(), Some(&TreeEvent::Activated(2_000_000)));
}

#[test]
fn keyboard_selection_scrolls_into_view() {
    let mut h = setup(1, 500);
    h.rt.app.tree.expand(1);
    h.rt.invalidate();
    h.settle();
    let r = h.rt.rect_of_text("Folder 1").unwrap().center();
    h.click(r.x, r.y);
    h.settle();
    key(&mut h, Key::End);
    let last = h.rt.rect_of_text("file 1.499").expect("last row built");
    assert!(last.bottom() <= 400.0 + 0.5 && last.y > 300.0, "at the bottom edge: {last:?}");
    // Up a few rows: no scrolling while visible.
    key(&mut h, Key::Up);
    key(&mut h, Key::Up);
    assert_eq!(h.rt.rect_of_text("file 1.499").unwrap().y, last.y);
    key(&mut h, Key::Home);
    let first = h.rt.rect_of_text("Folder 1").unwrap();
    assert!(first.y >= 0.0 && first.y < 20.0, "{first:?}");
    // PageDown moves 20 rows.
    key(&mut h, Key::PageDown);
    assert_eq!(selected(&h), Some(1_000_000 + 19));
}

#[test]
fn mouse() {
    let mut h = setup(3, 2);
    // The chevron toggles without selecting; a double-click on a file activates it.
    let row = h.rt.rect_of_text("Folder 3").unwrap();
    // Chevron (14 px), gap, folder icon (15 px), gap, then the label.
    h.click(row.x - 34.0, row.center().y);
    h.settle();
    assert!(h.rt.app.tree.is_expanded(&3));
    assert_eq!(selected(&h), None);
    let f = h.rt.rect_of_text("file 3.1").unwrap().center();
    h.click(f.x, f.y);
    h.click(f.x, f.y);
    h.settle();
    assert_eq!(h.rt.app.events.last(), Some(&TreeEvent::Activated(3_000_001)));
    // Double-clicking a folder toggles it.
    let r = h.rt.rect_of_text("Folder 3").unwrap().center();
    h.click(r.x, r.y);
    h.click(r.x, r.y);
    h.settle();
    assert!(!h.rt.app.tree.is_expanded(&3));
    assert!(h.rt.rect_of_text("file 3.1").is_none());
}
