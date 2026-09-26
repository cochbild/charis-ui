//! The layout tree persists between frames; these check that every kind of
//! change still re-lays out correctly (nothing stale is reused).

use rust_ui::prelude::*;

#[derive(Default)]
struct L {
    label: String,
    wide: bool,
    swap: bool,
    extra: usize,
}

#[derive(Clone, Debug)]
enum Msg {}

impl App for L {
    type Msg = Msg;
    fn update(&mut self, _: Msg, _: &mut Cx<Msg>) {}
    fn view(&self) -> Element<Msg> {
        let chip = row().id("chip").px(8.0).bg(hex("#333")).child(text(self.label.clone()));
        let boxy = div().id("box").h(20.0).w(if self.wide { 300.0 } else { 100.0 }).bg(hex("#444"));
        let moved = div().id("moved").w(50.0).h(10.0);
        let (a, b) = if self.swap {
            (row().id("a").child(div().w(10.0).h(10.0)), row().id("b").child(moved))
        } else {
            (row().id("a").child(moved), row().id("b").child(div().w(10.0).h(10.0)))
        };
        col()
            .items(Align::Start)
            .child(row().child(chip))
            .child(boxy)
            .child(a)
            .child(b)
            .child(col().id("list").children((0..self.extra).map(|i| text(format!("line {i}")).h(20.0))))
    }
}

fn frame(h: &mut Headless<L>, f: impl FnOnce(&mut L)) {
    f(&mut h.rt.app);
    h.rt.invalidate();
    h.advance(0.016);
}

#[test]
fn text_changes_resize_their_ancestors() {
    let mut h = Headless::new(L { label: "hi".into(), ..Default::default() }, 600.0, 400.0, 1.0);
    h.settle();
    let small = h.rt.rect_of("chip").unwrap().w;
    frame(&mut h, |a| a.label = "a much longer label than before".into());
    let big = h.rt.rect_of("chip").unwrap().w;
    assert!(big > small + 100.0, "{small} -> {big}");
    frame(&mut h, |a| a.label = "hi".into());
    assert_eq!(h.rt.rect_of("chip").unwrap().w, small);
}

#[test]
fn style_changes_apply() {
    let mut h = Headless::new(L::default(), 600.0, 400.0, 1.0);
    h.settle();
    assert_eq!(h.rt.rect_of("box").unwrap().w, 100.0);
    frame(&mut h, |a| a.wide = true);
    assert_eq!(h.rt.rect_of("box").unwrap().w, 300.0);
}

#[test]
fn elements_moving_between_parents_and_lists_growing() {
    let mut h = Headless::new(L::default(), 600.0, 400.0, 1.0);
    h.settle();
    let a = h.rt.rect_of("a").unwrap();
    assert_eq!(a.w, 50.0);
    frame(&mut h, |a| a.swap = true);
    assert_eq!(h.rt.rect_of("a").unwrap().w, 10.0);
    assert_eq!(h.rt.rect_of("b").unwrap().w, 50.0);
    frame(&mut h, |a| a.swap = false);
    assert_eq!(h.rt.rect_of("a").unwrap().w, 50.0);

    frame(&mut h, |a| a.extra = 5);
    assert_eq!(h.rt.rect_of("list").unwrap().h, 100.0);
    frame(&mut h, |a| a.extra = 2);
    assert_eq!(h.rt.rect_of("list").unwrap().h, 40.0);
    assert!(h.rt.rect_of_text("line 4").is_none());
    frame(&mut h, |a| a.extra = 3);
    assert_eq!(h.rt.rect_of("list").unwrap().h, 60.0);
}

#[test]
fn window_resize_relayouts() {
    let mut h = Headless::new(L::default(), 600.0, 400.0, 1.0);
    h.settle();
    h.rt.resize(Size::new(300.0, 200.0), 1.0);
    h.advance(0.016);
    assert_eq!(h.rt.rect_of("__root").map(|r| r.w), Some(300.0));
}
