//! Components with their own state: local events update local state, only
//! chosen outputs reach the app, nesting, lifetime, and `lazy` interplay.

use std::cell::Cell;

use charis_ui::prelude::*;

struct Counter {
    limit: u32,
    label: &'static str,
}

#[derive(Clone, Debug)]
enum CEv {
    Inc,
}

impl Component for Counter {
    type State = u32;
    type Event = CEv;
    type Output = String;
    fn update(&self, n: &mut u32, e: CEv) -> Option<String> {
        match e {
            CEv::Inc => {
                *n += 1;
                (*n == self.limit).then(|| format!("{} reached {}", self.label, self.limit))
            }
        }
    }
    fn view(&self, n: &u32) -> Element<CEv> {
        button(format!("{} {n}", self.label)).id(self.label).on_click(CEv::Inc)
    }
}

/// A panel that contains a counter and reports how often it hit its limit.
struct Panel;

#[derive(Clone, Debug)]
enum PEv {
    Toggle,
    Reached(String),
}

#[derive(Default)]
struct PanelState {
    open: bool,
    hits: u32,
}

impl Component for Panel {
    type State = PanelState;
    type Event = PEv;
    type Output = Msg;
    fn update(&self, s: &mut PanelState, e: PEv) -> Option<Msg> {
        match e {
            PEv::Toggle => {
                s.open = !s.open;
                None
            }
            PEv::Reached(what) => {
                s.hits += 1;
                Some(Msg::Log(format!("panel: {what}")))
            }
        }
    }
    fn view(&self, s: &PanelState) -> Element<PEv> {
        col()
            .child(button(format!("hits {}", s.hits)).id("toggle").on_click(PEv::Toggle))
            .child_if(s.open, || component("inner", Counter { limit: 2, label: "inner" }).map(PEv::Reached))
    }
}

#[derive(Clone, Debug)]
enum Msg {
    Log(String),
    Filter(String),
}

#[derive(Default)]
struct T {
    log: Vec<String>,
    show_b: bool,
    show_panel: bool,
    lazy_builds: Cell<u32>,
    filter_seen: Vec<String>,
}

impl App for T {
    type Msg = Msg;
    fn update(&mut self, m: Msg, _: &mut Cx<Msg>) {
        match m {
            Msg::Log(s) => self.log.push(s),
            Msg::Filter(s) => self.filter_seen.push(s),
        }
    }
    fn view(&self) -> Element<Msg> {
        let mut root =
            col().items(Align::Start).gap(4.0).child(component("a", Counter { limit: 3, label: "a" }).map(Msg::Log));
        if self.show_b {
            root = root.child(component("b", Counter { limit: 1, label: "b" }).map(Msg::Log));
        }
        if self.show_panel {
            root = root.child(component("panel", Panel));
        }
        // A component inside a memoized subtree whose deps never change.
        root.child(lazy("static", (), || {
            self.lazy_builds.set(self.lazy_builds.get() + 1);
            col().child(component("in-lazy", Counter { limit: 99, label: "lazy" }).map(Msg::Log)).child(stateful(
                "filter",
                |q: &String| text_input(q.clone(), |s| s).id("filter").w(160.0),
                |q: &mut String, s: String| {
                    *q = s;
                    // Only tell the app once the query is long enough.
                    (q.len() >= 3).then(|| Msg::Filter(q.clone()))
                },
            ))
        }))
    }
}

fn click(h: &mut Headless<T>, id: &str) {
    let r = h.rt.rect_of(id).unwrap_or_else(|| panic!("{id} not on screen"));
    h.click(r.center().x, r.center().y);
}

fn harness(f: impl FnOnce(&mut T)) -> Headless<T> {
    let mut app = T::default();
    f(&mut app);
    let mut h = Headless::new(app, 500.0, 400.0, 1.0);
    h.settle();
    h
}

#[test]
fn local_events_update_local_state_and_outputs_reach_the_app() {
    let mut h = harness(|_| {});
    click(&mut h, "a");
    click(&mut h, "a");
    assert!(h.rt.rect_of_text("a 2").is_some());
    assert!(h.rt.app.log.is_empty(), "local events don't reach the app");
    click(&mut h, "a");
    assert!(h.rt.rect_of_text("a 3").is_some());
    assert_eq!(h.rt.app.log, ["a reached 3"]);
}

#[test]
fn instances_have_separate_state_and_it_ends_with_the_component() {
    let mut h = harness(|a| a.show_b = true);
    click(&mut h, "a");
    click(&mut h, "b");
    click(&mut h, "b");
    assert!(h.rt.rect_of_text("a 1").is_some());
    assert!(h.rt.rect_of_text("b 2").is_some());
    assert_eq!(h.rt.app.log, ["b reached 1"]);

    h.rt.app.show_b = false;
    h.rt.invalidate();
    h.advance(0.016);
    h.rt.app.show_b = true;
    h.rt.invalidate();
    h.advance(0.016);
    assert!(h.rt.rect_of_text("b 0").is_some(), "state was dropped when b left the UI");
    assert!(h.rt.rect_of_text("a 1").is_some());
}

#[test]
fn nested_components_route_outputs_up() {
    let mut h = harness(|a| a.show_panel = true);
    assert!(h.rt.rect_of("inner").is_none());
    click(&mut h, "toggle");
    click(&mut h, "inner");
    click(&mut h, "inner");
    assert!(h.rt.rect_of_text("hits 1").is_some(), "the inner output updated the panel");
    assert_eq!(h.rt.app.log, ["panel: inner reached 2"]);
}

#[test]
fn components_inside_lazy_keep_state_and_rebuild_it() {
    let mut h = harness(|_| {});
    click(&mut h, "lazy");
    assert!(h.rt.rect_of_text("lazy 1").is_some(), "local update rebuilt the memoized subtree");
    // Leave the button (its hover change and transition rebuild the subtree
    // until settled).
    h.move_to(490.0, 390.0);
    h.settle();
    let builds = h.rt.app.lazy_builds.get();
    // Unrelated updates reuse the subtree, and the component's state survives.
    for _ in 0..5 {
        h.rt.send(Msg::Log("tick".into()));
        h.advance(0.016);
    }
    assert_eq!(h.rt.app.lazy_builds.get(), builds);
    click(&mut h, "lazy");
    assert!(h.rt.rect_of_text("lazy 2").is_some());
}

#[test]
fn stateful_closures_and_text_input() {
    let mut h = harness(|_| {});
    click(&mut h, "filter");
    h.type_text("ab");
    assert!(h.rt.app.filter_seen.is_empty(), "short queries stay local");
    h.type_text("c");
    assert_eq!(h.rt.app.filter_seen, ["abc"]);
    h.type_text("d");
    assert_eq!(h.rt.app.filter_seen, ["abc", "abcd"], "the query lives in the component");
}
