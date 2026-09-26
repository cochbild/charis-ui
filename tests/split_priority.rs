//! Split resize priorities (fixed panes shrink/grow by priority when the
//! container changes) and panes that remember their size (Pane::key).

use rust_ui::prelude::*;
use rust_ui::{Event, MouseButton};

struct S {
    /// Include a flex pane in the middle.
    flex: bool,
    /// Show the keyed bottom panel.
    panel: bool,
    keyed: bool,
}

#[derive(Clone)]
enum Msg {}

impl App for S {
    type Msg = Msg;
    fn update(&mut self, _: Msg, _: &mut Cx<Msg>) {}
    fn view(&self) -> Element<Msg> {
        let mut panes = vec![Pane::fixed(200.0, div().id("low")).min(60.0).priority(Priority::Low)];
        if self.flex {
            panes.push(Pane::fill(div().id("mid")).min(100.0));
        }
        panes.push(Pane::fixed(200.0, div().id("high")).min(60.0).priority(Priority::High));
        let top = hsplit("h", panes);
        let mut rows = vec![Pane::fill(top)];
        if self.panel {
            let p = Pane::fixed(150.0, div().id("panel"));
            rows.push(if self.keyed { p.key("panel") } else { p });
        }
        vsplit("v", rows)
    }
}

fn w(h: &Headless<S>, id: &str) -> f32 {
    h.rt.rect_of(id).unwrap_or_else(|| panic!("{id}")).w
}

fn resize(h: &mut Headless<S>, width: f32, height: f32) {
    h.rt.resize(Size::new(width, height), 1.0);
    h.settle();
}

#[test]
fn fixed_panes_shrink_by_priority_and_come_back() {
    let mut h = Headless::new(S { flex: true, panel: false, keyed: false }, 800.0, 400.0, 1.0);
    h.settle();
    assert_eq!((w(&h, "low"), w(&h, "high")), (200.0, 200.0));
    // Too narrow: the flex pane stops at its minimum, then the High pane
    // gives up space while the Low one keeps its size.
    resize(&mut h, 400.0, 400.0);
    assert!((w(&h, "mid") - 100.0).abs() < 1.0, "mid {}", w(&h, "mid"));
    assert!((w(&h, "low") - 200.0).abs() < 1.5, "low keeps its size: {}", w(&h, "low"));
    assert!((w(&h, "high") - 98.0).abs() < 2.0, "high shrinks: {}", w(&h, "high"));
    // Narrower still: High stops at its minimum, then Low shrinks.
    resize(&mut h, 300.0, 400.0);
    assert!((w(&h, "high") - 60.0).abs() < 1.0, "high at min: {}", w(&h, "high"));
    assert!(w(&h, "low") < 150.0);
    // Wide again: both are back to their own sizes.
    resize(&mut h, 800.0, 400.0);
    assert_eq!((w(&h, "low"), w(&h, "high")), (200.0, 200.0));
}

#[test]
fn without_a_flex_pane_the_high_priority_pane_takes_the_room() {
    let mut h = Headless::new(S { flex: false, panel: false, keyed: false }, 800.0, 400.0, 1.0);
    h.settle();
    assert!((w(&h, "low") - 200.0).abs() < 1.5, "{}", w(&h, "low"));
    assert!((w(&h, "high") - 599.0).abs() < 2.0, "{}", w(&h, "high"));
}

#[test]
fn dragging_a_squeezed_pane_starts_from_its_size_on_screen() {
    let mut h = Headless::new(S { flex: true, panel: false, keyed: false }, 400.0, 400.0, 1.0);
    h.settle();
    let high = h.rt.rect_of("high").unwrap();
    let before = high.w;
    // Drag the splitter left of "high" 20 px right: it shrinks by 20 from
    // what's shown. (Starting from its stored 200 px, it would have stayed
    // squeezed at 98 and the drag would do nothing.)
    let x = high.x - 0.5;
    h.drag((x, 200.0), (x + 20.0, 200.0), 6);
    h.settle();
    let after = w(&h, "high");
    assert!((after - (before - 20.0)).abs() < 1.5, "{before} -> {after}");
}

#[test]
fn keyed_panes_remember_their_size() {
    for keyed in [true, false] {
        let mut h = Headless::new(S { flex: true, panel: true, keyed }, 800.0, 600.0, 1.0);
        h.settle();
        let p = h.rt.rect_of("panel").unwrap();
        assert!((p.h - 150.0).abs() < 1.0);
        // The user makes the panel 100 px taller.
        h.event(Event::PointerDown(Point::new(400.0, p.y - 0.5), MouseButton::Left));
        for i in 1..=5 {
            h.move_to(400.0, p.y - 0.5 - 20.0 * i as f32);
        }
        h.event(Event::PointerUp(Point::new(400.0, p.y - 100.5), MouseButton::Left));
        h.settle();
        assert!((h.rt.rect_of("panel").unwrap().h - 250.0).abs() < 1.5);
        // Hide it, show it again.
        h.rt.app.panel = false;
        h.rt.invalidate();
        h.settle();
        assert!(h.rt.rect_of("panel").is_none());
        h.rt.app.panel = true;
        h.rt.invalidate();
        h.settle();
        let back = h.rt.rect_of("panel").unwrap().h;
        if keyed {
            assert!((back - 250.0).abs() < 1.5, "remembered: {back}");
        } else {
            assert!((back - 150.0).abs() < 1.5, "without a key it starts over: {back}");
        }
    }
}
