//! The dialog of a `modal` is clickable although its full-window wrapper
//! lets clicks through (`pointer_events(false)` with an opt-back-in).

use rust_ui::prelude::*;
struct A {
    n: u32,
}
#[derive(Clone, Debug)]
enum Msg {
    Hit,
    Dismiss,
}
impl App for A {
    type Msg = Msg;
    fn update(&mut self, m: Msg, _: &mut Cx<Msg>) {
        if let Msg::Hit = m {
            self.n += 1
        }
    }
    fn view(&self) -> Element<Msg> {
        div().size_full().child(modal("T", text("body"), vec![button("OK").id("ok").on_click(Msg::Hit)], Msg::Dismiss))
    }
}
#[test]
fn modal_buttons_are_clickable() {
    let mut h = Headless::new(A { n: 0 }, 800.0, 600.0, 1.0);
    h.settle();
    let r = h.rt.rect_of("ok").unwrap();
    h.click(r.center().x, r.center().y);
    h.settle();
    assert_eq!(h.rt.app.n, 1);
}
