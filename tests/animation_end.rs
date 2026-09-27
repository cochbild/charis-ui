//! The last frame of an animation is always drawn: a transition that ends
//! between two frames still gets a frame showing its end state.

use charis_ui::prelude::*;

struct A;
#[derive(Clone)]
enum Msg {}
impl App for A {
    type Msg = Msg;
    fn update(&mut self, _: Msg, _: &mut Cx<Msg>) {}
    fn view(&self) -> Element<Msg> {
        div()
            .id("b")
            .size(100.0, 100.0)
            .bg(Color::rgb(255, 0, 0))
            .hover(|s| s.bg(Color::rgb(0, 0, 255)))
            .transition(0.1)
    }
}

#[test]
fn final_transition_frame_is_rendered() {
    let mut h = Headless::new(A, 200.0, 200.0, 1.0);
    h.settle();
    h.move_to(50.0, 50.0);
    h.settle();
    assert_eq!(h.pixel(50.0, 50.0)[..3], [0, 0, 255]);
    // Leave, and render a frame just before the fade ends.
    h.move_to(150.0, 150.0);
    let t0 = h.rt.time();
    let _ = h.rt.render(); // the fade starts
    h.rt.set_time(t0 + 0.04);
    let _ = h.rt.render();
    assert_ne!(h.pixel(50.0, 50.0)[..3], [255, 0, 0], "not finished yet");
    // Time passes the end without a frame: one more frame is still due.
    h.rt.set_time(t0 + 0.3);
    assert!(h.rt.next_frame().is_some_and(|t| t <= h.rt.time()), "a frame showing the end state is due");
    let _ = h.rt.render();
    assert_eq!(h.pixel(50.0, 50.0)[..3], [255, 0, 0]);
    assert_eq!(h.rt.next_frame(), None, "then it's idle");
}
