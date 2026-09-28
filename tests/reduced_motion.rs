//! Reduced motion: movement is instant, fades still fade.

use charis_ui::anim::set_reduced_motion;
use charis_ui::prelude::*;

#[derive(Default)]
struct M {
    moved: bool,
    lit: bool,
    collapsed: bool,
    scroll: f32,
}

#[derive(Clone, Debug)]
enum Msg {
    Scrolled(ScrollInfo),
}

impl App for M {
    type Msg = Msg;
    fn update(&mut self, m: Msg, _: &mut Cx<Msg>) {
        let Msg::Scrolled(s) = m;
        self.scroll = s.offset.y;
    }
    fn view(&self) -> Element<Msg> {
        let list = col()
            .id("list")
            .h(200.0)
            .scroll_y()
            .on_scroll(Msg::Scrolled)
            .children((0..100).map(|i| text(format!("row {i}")).h(20.0)));
        let boxes = row()
            .child(
                div()
                    .id("mover")
                    .w(20.0)
                    .h(20.0)
                    .bg(hex("#ffffff"))
                    .transition(0.3)
                    .translate(if self.moved { 100.0 } else { 0.0 }, 0.0),
            )
            .child(div().id("fader").w(20.0).h(20.0).transition(0.3).bg(if self.lit {
                hex("#ffffff")
            } else {
                hex("#000000")
            }));
        let side = hsplit(
            "split",
            vec![
                Pane::fixed(120.0, div().id("pane")).collapsible(true).collapsed(self.collapsed),
                Pane::fill(div().id("rest").size_full()),
            ],
        );
        col().child(list).child(boxes).child(div().h(100.0).child(side))
    }
}

fn run(reduced: bool) -> (Headless<M>, bool, f32, u8, f32) {
    set_reduced_motion(Some(reduced));
    let mut h = Headless::new(M::default(), 400.0, 500.0, 1.0);
    h.settle();
    let r = h.rt.rect_of("list").unwrap();
    h.event(charis_ui::Event::Wheel(r.center(), Point::new(0.0, 300.0)));
    h.advance(1.0 / 60.0);
    // How far the content actually moved (on_scroll reports the target).
    let scroll_after_one_frame = r.y - h.rt.rect_of_text("row 0").map_or(r.y - 400.0, |t| t.y);

    let x0 = h.rt.rect_of("mover").unwrap().x;
    h.rt.app.moved = true;
    h.rt.app.lit = true;
    h.rt.app.collapsed = true;
    h.rt.invalidate();
    h.advance(1.0 / 60.0);
    h.advance(0.05);
    let mover = h.rt.rect_of("mover").unwrap();
    let moved_fully =
        (h.pixel(x0 + 100.0 + 10.0, mover.y + 10.0)[0] == 255) && h.pixel(x0 + 10.0, mover.y + 10.0)[0] != 255;
    let f = h.rt.rect_of("fader").unwrap();
    let fade = h.pixel(f.center().x, f.center().y)[0];
    // Where the neighbouring pane starts: 0 once the side pane is closed.
    let pane_w = h.rt.rect_of("rest").map_or(0.0, |r| r.x);
    (h, moved_fully, scroll_after_one_frame, fade, pane_w)
}

#[test]
fn reduced_motion_makes_movement_instant() {
    let (_, moved, scroll, fade, pane_w) = run(true);
    assert!(scroll >= 299.0, "wheel scroll jumps: {scroll}");
    assert!(moved, "translate transition snaps");
    assert!(fade > 20 && fade < 235, "color transitions still fade: {fade}");
    assert!(pane_w < 6.0, "pane collapse is instant: {pane_w}");
    set_reduced_motion(None);
}

#[test]
fn normal_motion_animates() {
    let (_, moved, scroll, fade, pane_w) = run(false);
    assert!(scroll < 299.0, "wheel scroll is smooth: {scroll}");
    assert!(!moved, "translate animates");
    assert!(fade > 20 && fade < 235, "{fade}");
    assert!(pane_w > 6.0, "pane slides closed: {pane_w}");
    set_reduced_motion(None);
}
