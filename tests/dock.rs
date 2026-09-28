//! End-to-end docking: drag real tabs with the pointer.

use charis_ui::prelude::*;
use charis_ui::{Event, MouseButton};

struct D {
    dock: Dock<&'static str>,
}

#[derive(Clone)]
enum Msg {
    Dock(DockMsg),
}

impl App for D {
    type Msg = Msg;
    fn update(&mut self, msg: Msg, _: &mut Cx<Msg>) {
        match msg {
            Msg::Dock(m) => self.dock.update(m),
        }
    }
    fn view(&self) -> Element<Msg> {
        self.dock.view("d", |t| t.to_string(), |t| text(format!("content {t}")).id(t), Msg::Dock)
    }
}

fn setup() -> Headless<D> {
    let dock = Dock::new(DockNode::hsplit(vec![
        (1.0, DockNode::tabs(vec!["left", "other"])),
        (1.0, DockNode::tabs(vec!["right"])),
    ]));
    let mut h = Headless::new(D { dock }, 800.0, 500.0, 1.0);
    h.settle();
    h
}

fn drag(h: &mut Headless<D>, from: Point, to: Point) {
    h.move_to(from.x, from.y);
    h.event(Event::PointerDown(from, MouseButton::Left));
    for i in 1..=8 {
        let t = i as f32 / 8.0;
        h.move_to(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
    }
    h.event(Event::PointerUp(to, MouseButton::Left));
    h.settle();
}

fn layout(h: &Headless<D>) -> Vec<Vec<&'static str>> {
    h.rt.app.dock.groups().iter().map(|g| g.tabs.clone()).collect()
}

#[test]
fn drag_tab_to_center_of_other_group() {
    let mut h = setup();
    let tab = h.rt.rect_of_text("other").unwrap().center();
    drag(&mut h, tab, Point::new(600.0, 250.0));
    assert_eq!(layout(&h), vec![vec!["left"], vec!["right", "other"]]);
    // The moved tab became active in its new group.
    assert!(h.rt.rect_of_text("content other").is_some());
}

#[test]
fn drag_tab_to_bottom_edge_splits() {
    let mut h = setup();
    let tab = h.rt.rect_of_text("other").unwrap().center();
    drag(&mut h, tab, Point::new(600.0, 480.0));
    assert_eq!(layout(&h), vec![vec!["left"], vec!["right"], vec!["other"]]);
    let r = h.rt.rect_of_text("content other").unwrap();
    assert!(r.y > 250.0, "new group is below: {r:?}");
}

#[test]
fn click_without_drag_activates() {
    let mut h = setup();
    assert!(h.rt.rect_of_text("content left").is_some());
    let tab = h.rt.rect_of_text("other").unwrap().center();
    h.click(tab.x, tab.y);
    assert!(h.rt.rect_of_text("content other").is_some());
    assert_eq!(layout(&h), vec![vec!["left", "other"], vec!["right"]]);
}

#[test]
fn flex_splitter_redistributes() {
    let mut h = setup();
    let before = h.rt.rect_of_text("content right").unwrap().x;
    // Splitter between the two groups sits at x = 400.
    drag(&mut h, Point::new(400.0, 300.0), Point::new(300.0, 300.0));
    let after = h.rt.rect_of_text("content right").unwrap().x;
    assert!((before - after - 100.0).abs() < 2.0, "before {before} after {after}");
}

#[test]
fn reorder_tabs_within_group() {
    let mut h = setup();
    let left = h.rt.rect_of_text("left").unwrap().center();
    let other = h.rt.rect_of_text("other").unwrap();
    // Drop onto the right half of "other" → insert after it.
    drag(&mut h, left, Point::new(other.right() + 20.0, other.center().y));
    assert_eq!(layout(&h), vec![vec!["other", "left"], vec!["right"]]);
    assert!(h.rt.rect_of_text("content left").is_some(), "moved tab stays active");
}

#[test]
fn move_tab_into_other_group_at_position() {
    let mut h = setup();
    let other = h.rt.rect_of_text("other").unwrap().center();
    let right = h.rt.rect_of_text("right").unwrap();
    drag(&mut h, other, Point::new(right.x + 2.0, right.center().y));
    assert_eq!(layout(&h), vec![vec!["left"], vec!["other", "right"]]);
}

#[test]
fn double_click_tab_maximizes() {
    let mut h = setup();
    let r = h.rt.rect_of_text("right").unwrap().center();
    h.click(r.x, r.y);
    h.click(r.x, r.y);
    assert!(h.rt.app.dock.maximized().is_some());
    assert!(h.rt.rect_of_text("content left").is_none(), "other groups hidden while maximized");
    let full = h.rt.rect_of_text("content right").unwrap();
    assert!(full.x < 20.0, "maximized group fills the dock: {full:?}");
    let r = h.rt.rect_of_text("right").unwrap().center();
    h.click(r.x, r.y);
    h.click(r.x, r.y);
    assert!(h.rt.app.dock.maximized().is_none());
}

#[test]
fn resizing_updates_layout_weights() {
    let mut h = setup();
    drag(&mut h, Point::new(400.0, 300.0), Point::new(200.0, 300.0));
    match h.rt.app.dock.root().unwrap() {
        DockNode::Split { children, .. } => {
            let (a, b) = (children[0].0, children[1].0);
            assert!((a / (a + b) - 0.25).abs() < 0.02, "weights {a} {b}");
        }
        _ => panic!(),
    }
}

#[cfg(feature = "serde")]
#[test]
fn layout_roundtrips_through_serde() {
    let h = setup();
    let json = serde_json::to_string(&h.rt.app.dock).unwrap();
    let back: Dock<String> = serde_json::from_str(&json).unwrap();
    let tabs: Vec<Vec<String>> = back.groups().iter().map(|g| g.tabs.clone()).collect();
    assert_eq!(tabs, vec![vec!["left".to_string(), "other".to_string()], vec!["right".to_string()]]);
    assert_eq!(back.version, 1);
}
