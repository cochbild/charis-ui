//! Dock panel commands (tab context menu, keyboard), the drop compass, the
//! dock-edge guides, and keyboard resizing of splitters.

use charis_ui::prelude::*;
use charis_ui::{Event, MouseButton};

struct D {
    dock: Dock<&'static str>,
    resized: Vec<Vec<f32>>,
}

#[derive(Clone)]
enum Msg {
    Dock(DockMsg),
}

impl App for D {
    type Msg = Msg;
    fn update(&mut self, msg: Msg, _: &mut Cx<Msg>) {
        let Msg::Dock(m) = msg;
        if let DockMsg::Resized(_, _, w) = &m {
            self.resized.push(w.clone());
        }
        self.dock.update(m);
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
    let mut h = Headless::new(D { dock, resized: Vec::new() }, 800.0, 500.0, 1.0);
    h.settle();
    h
}

fn layout(h: &Headless<D>) -> Vec<Vec<&'static str>> {
    h.rt.app.dock.groups().iter().map(|g| g.tabs.clone()).collect()
}

fn key(h: &mut Headless<D>, key: Key, shift: bool) {
    h.event(Event::Key(KeyEvent { key, mods: Modifiers { shift, ..Default::default() }, repeat: false }));
    h.settle();
}

fn click_text(h: &mut Headless<D>, s: &str) {
    let r = h.rt.rect_of_text(s).unwrap_or_else(|| panic!("no {s:?}"));
    h.click(r.center().x, r.center().y);
    h.settle();
}

/// Press at `from`, move along to `to`, and optionally release.
fn drag_to(h: &mut Headless<D>, from: Point, to: Point, release: bool) {
    h.move_to(from.x, from.y);
    h.event(Event::PointerDown(from, MouseButton::Left));
    for i in 1..=10 {
        let t = i as f32 / 10.0;
        h.move_to(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
    }
    if release {
        h.event(Event::PointerUp(to, MouseButton::Left));
    }
    h.settle();
}

#[test]
fn tab_context_menu() {
    let mut h = setup();
    let tab = h.rt.rect_of_text("other").unwrap().center();
    h.event(Event::PointerDown(tab, MouseButton::Right));
    h.event(Event::PointerUp(tab, MouseButton::Right));
    h.settle();
    assert!(h.rt.rect_of_text("Close Others").is_some(), "menu open");
    click_text(&mut h, "Split Right");
    assert_eq!(layout(&h), vec![vec!["left"], vec!["other"], vec!["right"]]);
    assert!(h.rt.rect_of_text("Close Others").is_none(), "menu closed");

    // Submenu: move "right" to the dock's bottom edge (full width).
    let tab = h.rt.rect_of_text("right").unwrap().center();
    h.event(Event::PointerDown(tab, MouseButton::Right));
    h.event(Event::PointerUp(tab, MouseButton::Right));
    h.settle();
    let sub = h.rt.rect_of_text("Move to Edge").unwrap().center();
    h.move_to(sub.x, sub.y);
    h.settle();
    click_text(&mut h, "Bottom");
    let r = h.rt.rect_of_text("content right").unwrap();
    assert!(r.y > 300.0 && r.x < 20.0, "bottom panel spans the width: {r:?}");

    // A click elsewhere dismisses the menu.
    let tab = h.rt.rect_of_text("left").unwrap().center();
    h.event(Event::PointerDown(tab, MouseButton::Right));
    h.event(Event::PointerUp(tab, MouseButton::Right));
    h.settle();
    h.click(700.0, 150.0);
    h.settle();
    assert!(h.rt.rect_of_text("Close Others").is_none());
}

#[test]
fn tab_menu_from_the_keyboard() {
    let mut h = setup();
    // Focus the tab, then Shift+F10 opens its menu with the first item focused.
    click_text(&mut h, "other");
    key(&mut h, Key::F(10), true);
    assert!(h.rt.rect_of_text("Close Others").is_some());
    // Close > Close Others: Down, Enter.
    key(&mut h, Key::Down, false);
    key(&mut h, Key::Enter, false);
    assert_eq!(layout(&h), vec![vec!["other"], vec!["right"]]);
    // Escape closes it.
    click_text(&mut h, "other");
    key(&mut h, Key::F(10), true);
    assert!(h.rt.rect_of_text("Split Right").is_some());
    key(&mut h, Key::Escape, false);
    assert!(h.rt.rect_of_text("Split Right").is_none());
}

#[test]
fn compass_targets() {
    let mut h = setup();
    // The right group spans x 400..800, body y 34..500: compass centered at
    // (600, 267), buttons 38 px apart. Its left button splits left, which
    // the edge band alone would treat as "join" there.
    let tab = h.rt.rect_of_text("other").unwrap().center();
    let body_c = Point::new(600.5, (500.0 + h.rt.app.dock.tab_height.max(0.0) + 34.0) / 2.0);
    drag_to(&mut h, tab, Point::new(body_c.x - 38.0, body_c.y), false);
    assert!(h.rt.app.dock.dragging());
    h.event(Event::PointerUp(Point::new(body_c.x - 38.0, body_c.y), MouseButton::Left));
    h.settle();
    assert_eq!(layout(&h), vec![vec!["left"], vec!["other"], vec!["right"]]);

    // Without the compass, the same spot joins the group.
    let mut h = setup();
    h.rt.app.dock.compass = false;
    let tab = h.rt.rect_of_text("other").unwrap().center();
    drag_to(&mut h, tab, Point::new(body_c.x - 38.0, body_c.y), true);
    assert_eq!(layout(&h), vec![vec!["left"], vec!["right", "other"]]);
}

#[test]
fn edge_guides() {
    let mut h = setup();
    let tab = h.rt.rect_of_text("right").unwrap().center();
    // Start dragging so the guides appear, then drop on the left guide.
    drag_to(&mut h, tab, Point::new(600.0, 300.0), false);
    let g = h.rt.rect_of("d/edge-guide/Left").expect("guide shown while dragging");
    assert!(g.x < 20.0 && (g.center().y - 250.0).abs() < 2.0, "{g:?}");
    for i in 1..=6 {
        let t = i as f32 / 6.0;
        h.move_to(600.0 + (g.center().x - 600.0) * t, 300.0 + (g.center().y - 300.0) * t);
    }
    h.settle();
    h.event(Event::PointerUp(g.center(), MouseButton::Left));
    h.settle();
    assert_eq!(layout(&h), vec![vec!["right"], vec!["left", "other"]]);
    assert!(h.rt.rect_of("d/edge-guide/Left").is_none(), "guides hide after the drag");
    let r = h.rt.rect_of_text("content right").unwrap();
    assert!(r.x < 20.0, "{r:?}");
}

#[test]
fn keyboard_splitter() {
    let mut h = setup();
    // Tab order: the left group's tabs, then the splitter between the groups.
    let before = h.rt.rect_of_text("content right").unwrap().x;
    let mut found = false;
    for _ in 0..12 {
        key(&mut h, Key::Tab, false);
        key(&mut h, Key::Right, true);
        if !h.rt.app.resized.is_empty() {
            found = true;
            break;
        }
    }
    assert!(found, "a focused splitter reacts to arrow keys");
    let after = h.rt.rect_of_text("content right").unwrap().x;
    assert!((after - before - 50.0).abs() < 2.0, "Shift+Right moves 50 px: {before} -> {after}");
    key(&mut h, Key::Left, false);
    let x = h.rt.rect_of_text("content right").unwrap().x;
    assert!((x - after + 10.0).abs() < 2.0, "Left moves 10 px back: {x}");
    // Home / End: to the limits (panes keep their 90 px minimum).
    key(&mut h, Key::Home, false);
    let x = h.rt.rect_of_text("content right").unwrap().x;
    assert!((80.0..120.0).contains(&x), "{x}");
    key(&mut h, Key::End, false);
    let x = h.rt.rect_of_text("content right").unwrap().x;
    assert!((690.0..720.0).contains(&x), "{x}");
}
