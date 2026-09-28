//! Dock tabs across windows: tear out, drop into another window, pop out /
//! dock back buttons, closing a floating window.

use charis_ui::dock::{Dock, DockNode, DockSpace, DockSpaceMsg};
use charis_ui::headless::HeadlessApp;
use charis_ui::prelude::*;

struct Ide {
    dock: DockSpace<&'static str>,
}

#[derive(Clone, Debug)]
enum Msg {
    Dock(DockSpaceMsg),
}

fn content(t: &&'static str) -> Element<Msg> {
    // The dock keys the content element itself, so the id goes one level in.
    col().size_full().child(col().id(&format!("content-{t}")).size_full().child(text(format!("Content {t}"))))
}

impl App for Ide {
    type Msg = Msg;
    fn update(&mut self, m: Msg, _: &mut Cx<Msg>) {
        let Msg::Dock(m) = m;
        self.dock.update(m);
    }
    fn view(&self) -> Element<Msg> {
        self.dock.view(None, |t| t.to_string(), content, Msg::Dock)
    }
    fn windows(&self) -> Vec<WindowSpec<Msg>> {
        self.dock.windows(|t| t.to_string(), Msg::Dock)
    }
    fn window_view(&self, key: &str) -> Element<Msg> {
        self.dock.view(Some(key), |t| t.to_string(), content, Msg::Dock)
    }
}

fn harness() -> HeadlessApp<Ide> {
    let dock =
        Dock::new(DockNode::hsplit(vec![(1.0, DockNode::tabs(vec!["a", "b"])), (1.0, DockNode::tabs(vec!["c"]))]));
    let mut h = HeadlessApp::new(Ide { dock: DockSpace::new(dock) }, 800.0, 500.0);
    h.set_origin(None, 0.0, 0.0);
    h
}

fn tabs(d: &Dock<&'static str>) -> Vec<Vec<&'static str>> {
    d.groups().iter().map(|g| g.tabs.clone()).collect()
}

fn tab_center(h: &mut HeadlessApp<Ide>, window: Option<&str>, t: &str) -> Point {
    let hw = match window {
        None => h.main(),
        Some(k) => h.window(k).unwrap(),
    };
    hw.rt.rect_of_text(t).unwrap_or_else(|| panic!("tab {t} not in {window:?}")).center()
}

/// Tear tab "b" out to screen point (1200, 300): a floating window opens there.
fn tear_out_b(h: &mut HeadlessApp<Ide>) {
    let from = tab_center(h, None, "b");
    h.drag(None, from, Point::new(1200.0, 300.0), 8);
}

#[test]
fn dragging_a_tab_out_of_the_window_floats_it() {
    let mut h = harness();
    tear_out_b(&mut h);
    assert_eq!(h.window_keys(), ["dock-1"]);
    assert_eq!(tabs(&h.app().dock.main), [vec!["a"], vec!["c"]]);
    {
        let app = h.app();
        let f = &app.dock.floating[0];
        assert_eq!(tabs(&f.dock), [vec!["b"]]);
        assert_eq!(f.position, Some((1140.0, 286.0)), "opens where the tab was dropped");
    }
    let spec = h.app().windows().remove(0);
    assert_eq!(spec.title, "b");
    assert!(h.window("dock-1").unwrap().rt.rect_of("content-b").is_some());
}

#[test]
fn dropping_on_another_window_docks_there() {
    let mut h = harness();
    tear_out_b(&mut h);
    h.set_origin(Some("dock-1"), 1140.0, 286.0);
    // Drag "b" from the floating window onto the center of group "c".
    let from = tab_center(&mut h, Some("dock-1"), "b");
    let target = h.main().rt.rect_of("content-c").unwrap().center();
    h.drag(Some("dock-1"), from, target, 10);
    assert_eq!(tabs(&h.app().dock.main), [vec!["a"], vec!["c", "b"]]);
    assert!(h.window_keys().is_empty(), "the empty floating window closed");
}

#[test]
fn dropping_on_an_edge_in_another_window_splits() {
    let mut h = harness();
    tear_out_b(&mut h);
    h.set_origin(Some("dock-1"), 1140.0, 286.0);
    let from = tab_center(&mut h, Some("dock-1"), "b");
    let r = h.main().rt.rect_of("content-a").unwrap();
    // Near the bottom edge of group "a".
    let target = Point::new(r.center().x, r.bottom() - 10.0);
    h.drag(Some("dock-1"), from, target, 10);
    assert_eq!(tabs(&h.app().dock.main), [vec!["a"], vec!["b"], vec!["c"]]);
}

#[test]
fn a_drag_that_passes_over_another_window_leaves_no_state_there() {
    let mut h = harness();
    tear_out_b(&mut h);
    h.set_origin(Some("dock-1"), 1140.0, 286.0);
    // Drag "a" over the floating window, then back onto group "c" at home.
    let from = tab_center(&mut h, None, "a");
    let over_float = Point::new(1140.0 + 200.0, 286.0 + 200.0);
    let home = h.main().rt.rect_of("content-c").unwrap().center();
    h.drag_path(None, from, &[over_float, home], 6);
    assert_eq!(tabs(&h.app().dock.main), [vec!["c", "a"]]);
    assert_eq!(tabs(&h.app().dock.floating[0].dock), [vec!["b"]], "the floating dock is untouched");
    assert!(!h.app().dock.floating[0].dock.dragging(), "no leftover drag or preview");
}

#[test]
fn dragging_into_a_floating_window_docks_there() {
    let mut h = harness();
    tear_out_b(&mut h);
    h.set_origin(Some("dock-1"), 1140.0, 286.0);
    // Drag "a" out over the floating window and back into the main window.
    let from = tab_center(&mut h, None, "a");
    let over_float = Point::new(1140.0 + 200.0, 286.0 + 200.0);
    h.drag(None, from, over_float, 6);
    // Dropped onto the floating window's group: "a" joined it.
    assert_eq!(tabs(&h.app().dock.floating[0].dock), [vec!["b", "a"]]);
    assert!(!h.app().dock.floating[0].dock.dragging(), "no leftover drag state");
    assert!(!h.app().dock.main.dragging());
}

#[test]
fn pop_out_and_dock_back_buttons() {
    let mut h = harness();
    // "Open in new window" on group "c" (its header's first button).
    let c = h.main().rt.rect_of_text("c").unwrap();
    let group_right = h.main().rt.rect_of("content-c").unwrap().right();
    // The header buttons sit at the right end of the tab strip: pop out, maximize.
    let pop = Point::new(group_right - 4.0 - 24.0 - 2.0 - 12.0, c.center().y);
    h.click(None, pop.x, pop.y);
    assert_eq!(h.window_keys(), ["dock-1"], "c popped out");
    assert_eq!(tabs(&h.app().dock.main), [vec!["a", "b"]]);
    // "Dock back" in the floating window.
    let w = h.window("dock-1").unwrap();
    let right = w.rt.rect_of("content-c").unwrap().right();
    let y = w.rt.rect_of_text("c").unwrap().center().y;
    h.click(Some("dock-1"), right - 4.0 - 24.0 - 2.0 - 12.0, y);
    assert!(h.window_keys().is_empty());
    assert_eq!(tabs(&h.app().dock.main), [vec!["a", "b", "c"]]);
}

#[test]
fn closing_a_floating_window_docks_its_tabs_back() {
    let mut h = harness();
    tear_out_b(&mut h);
    h.close("dock-1");
    assert!(h.window_keys().is_empty());
    assert_eq!(tabs(&h.app().dock.main), [vec!["a", "b"], vec!["c"]]);
}

/// Like Wayland: no window knows where it is on the screen.
fn harness_without_positions() -> HeadlessApp<Ide> {
    let dock =
        Dock::new(DockNode::hsplit(vec![(1.0, DockNode::tabs(vec!["a", "b"])), (1.0, DockNode::tabs(vec!["c"]))]));
    HeadlessApp::new(Ide { dock: DockSpace::new(dock) }, 800.0, 500.0)
}

#[test]
fn without_window_positions_tabs_still_tear_out_and_dock_into_other_windows() {
    let mut h = harness_without_positions();
    let from = tab_center(&mut h, None, "b");
    h.drag(None, from, Point::new(1200.0, 300.0), 8);
    assert_eq!(h.window_keys(), ["dock-1"], "torn out");
    assert_eq!(h.app().dock.floating[0].position, None, "the compositor places it");

    // Drag "b" back out of its window and release over group "c" in the main window.
    let from = tab_center(&mut h, Some("dock-1"), "b");
    let target = h.main().rt.rect_of("content-c").unwrap().center();
    h.drag_to_window(Some("dock-1"), from, None, target);
    assert_eq!(tabs(&h.app().dock.main), [vec!["a"], vec!["c", "b"]]);
    assert!(h.window_keys().is_empty(), "the empty floating window closed");
    assert!(!h.app().dock.main.dragging(), "no leftover drag state");

    // And onto an edge: "a" to the bottom of group "c" splits it.
    let from = tab_center(&mut h, None, "c");
    h.drag(None, from, Point::new(1200.0, 300.0), 8);
    let key = h.window_keys().remove(0);
    let from = tab_center(&mut h, Some(&key), "c");
    let r = h.main().rt.rect_of("content-a").unwrap();
    h.drag_to_window(Some(&key), from, None, Point::new(r.center().x, r.bottom() - 10.0));
    assert_eq!(tabs(&h.app().dock.main), [vec!["a"], vec!["c"], vec!["b"]]);
}
