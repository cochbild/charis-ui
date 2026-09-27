//! Memoized subtrees: `lazy` skips its build closure while deps are
//! unchanged, and everything inside keeps working.

use std::cell::Cell;

use charis_ui::prelude::*;

struct L {
    version: u32,
    other: u32,
    clicks: u32,
    text: String,
    accent: Color,
    dim: bool,
    moved: bool,
    builds: Cell<u32>,
    inner_builds: Cell<u32>,
}

impl Default for L {
    fn default() -> Self {
        Self {
            version: 0,
            other: 0,
            clicks: 0,
            text: String::new(),
            accent: hex("#3b82f6"),
            dim: false,
            moved: false,
            builds: Cell::new(0),
            inner_builds: Cell::new(0),
        }
    }
}

#[derive(Clone, Debug)]
enum Msg {
    Click,
    Input(String),
}

impl App for L {
    type Msg = Msg;
    fn theme(&self) -> Theme {
        Theme::dark().with_accent(self.accent)
    }
    fn update(&mut self, m: Msg, _: &mut Cx<Msg>) {
        match m {
            Msg::Click => self.clicks += 1,
            Msg::Input(s) => self.text = s,
        }
    }
    fn view(&self) -> Element<Msg> {
        let panel = lazy("panel", (self.version, &self.text), || {
            self.builds.set(self.builds.get() + 1);
            col()
                .gap(6.0)
                .child(text(format!("version {}", self.version)).id("label"))
                .child(
                    div()
                        .id("hoverable")
                        .w(80.0)
                        .h(20.0)
                        .bg(hex("#101010"))
                        .transition(0.2)
                        .hover(|s| s.bg(hex("#f0f0f0"))),
                )
                .child(primary_button("Press").id("btn").on_click(Msg::Click))
                .child(text_input(self.text.clone(), Msg::Input).id("input").w(200.0))
                .child(text("inherits color").id("inh"))
                .child(lazy("inner", (), || {
                    self.inner_builds.set(self.inner_builds.get() + 1);
                    text("inner").id("inner")
                }))
        });
        let parent_color = if self.dim { hex("#ff0000") } else { hex("#00ff00") };
        let slot_a = col().id("slot-a").color(parent_color);
        let slot_b = col().id("slot-b").pl(100.0);
        let (a, b) = if self.moved { (slot_a, slot_b.child(panel)) } else { (slot_a.child(panel), slot_b) };
        col().p(10.0).items(Align::Start).child(text(format!("other {}", self.other))).child(a).child(b)
    }
}

fn harness() -> Headless<L> {
    let mut h = Headless::new(L::default(), 500.0, 400.0, 1.0);
    h.settle();
    h
}

fn rebuild(h: &mut Headless<L>, f: impl FnOnce(&mut L)) {
    f(&mut h.rt.app);
    h.rt.invalidate();
    h.advance(1.0 / 60.0);
}

#[test]
fn build_is_skipped_until_deps_change() {
    let mut h = harness();
    let before = h.rt.app.builds.get();
    for i in 0..5 {
        rebuild(&mut h, |a| a.other = i);
    }
    assert_eq!(h.rt.app.builds.get(), before, "unchanged deps: no rebuilds");
    assert!(h.rt.rect_of_text("version 0").is_some(), "reused subtree is still on screen");

    rebuild(&mut h, |a| a.version = 7);
    assert_eq!(h.rt.app.builds.get(), before + 1);
    assert!(h.rt.rect_of_text("version 7").is_some());
    assert!(h.rt.rect_of_text("version 0").is_none());
    // The nested lazy has no deps: reused even when its parent rebuilds.
    assert_eq!(h.rt.app.inner_builds.get(), 1);
    assert!(h.rt.rect_of("inner").is_some());
}

#[test]
fn handlers_in_reused_subtrees_work() {
    let mut h = harness();
    for i in 0..3 {
        rebuild(&mut h, |a| a.other = i);
    }
    let r = h.rt.rect_of("btn").unwrap();
    h.click(r.center().x, r.center().y);
    h.click(r.center().x, r.center().y);
    assert_eq!(h.rt.app.clicks, 2);
}

#[test]
fn hover_and_transitions_inside_reused_subtrees() {
    let mut h = harness();
    for i in 0..3 {
        rebuild(&mut h, |a| a.other = i);
    }
    let r = h.rt.rect_of("hoverable").unwrap();
    let (x, y) = (r.center().x, r.center().y);
    h.move_to(x, y);
    // Mid-transition: the reused node kept its transition state, so the
    // hover animates instead of snapping.
    h.advance(0.08);
    let mid = h.pixel(x, y)[0];
    assert!(mid > 0x20 && mid < 0xe0, "animating, got {mid:#x}");
    h.settle();
    assert_eq!(h.pixel(x, y)[0], 0xf0);
    h.move_to(400.0, 390.0);
    h.settle();
    assert_eq!(h.pixel(x, y)[0], 0x10);
    // Settled again: back to being reused.
    let before = h.rt.app.builds.get();
    rebuild(&mut h, |a| a.other = 99);
    assert_eq!(h.rt.app.builds.get(), before);
}

#[test]
fn typing_into_an_input_inside() {
    let mut h = harness();
    let r = h.rt.rect_of("input").unwrap();
    h.click(r.center().x, r.center().y);
    h.type_text("hello");
    assert_eq!(h.rt.app.text, "hello");
}

#[test]
fn theme_changes_rebuild() {
    let mut h = harness();
    let before = h.rt.app.builds.get();
    rebuild(&mut h, |a| a.accent = hex("#e11d48"));
    assert_eq!(h.rt.app.builds.get(), before + 1);
    h.settle();
    let r = h.rt.rect_of("btn").unwrap();
    let px = h.pixel(r.x + 4.0, r.center().y);
    assert!(px[0] > px[2], "button follows the new accent: {px:?}");
}

#[test]
fn inherited_styles_and_moves_apply_without_rebuilding() {
    let mut h = harness();
    let before = h.rt.app.builds.get();
    let color_at = |h: &Headless<L>| {
        let r = h.rt.rect_of("inh").unwrap();
        // Brightest pixel along the text's middle line.
        (0..r.w as i32)
            .map(|dx| h.pixel(r.x + dx as f32, r.center().y))
            .max_by_key(|p| p[0] as u32 + p[1] as u32)
            .unwrap()
    };
    let g = color_at(&h);
    assert!(g[1] > 150 && g[0] < 80, "green text: {g:?}");
    rebuild(&mut h, |a| a.dim = true);
    let r = color_at(&h);
    assert!(r[0] > 150 && r[1] < 80, "inherits the parent's new color: {r:?}");

    let x0 = h.rt.rect_of("btn").unwrap().x;
    rebuild(&mut h, |a| a.moved = true);
    assert_eq!(h.rt.rect_of("btn").unwrap().x, x0 + 100.0, "moved with its new parent");
    assert_eq!(h.rt.app.builds.get(), before, "none of this rebuilt the subtree");
    let r = h.rt.rect_of("btn").unwrap();
    h.click(r.center().x, r.center().y);
    assert_eq!(h.rt.app.clicks, 1);
}

struct Rows {
    builds: std::rc::Rc<Cell<u32>>,
}

impl App for Rows {
    type Msg = Msg;
    fn update(&mut self, _: Msg, _: &mut Cx<Msg>) {}
    fn view(&self) -> Element<Msg> {
        let b = self.builds.clone();
        virtual_list(10_000, move |i| {
            lazy(("row", i), (), || {
                b.set(b.get() + 1);
                row().h(24.0).child(text(format!("row {i}")))
            })
        })
        .id("list")
        .item_height(24.0)
        .grow(1.0)
    }
}

#[test]
fn lazy_rows_in_virtual_lists() {
    let mut h = Headless::new(Rows { builds: Default::default() }, 300.0, 240.0, 1.0);
    h.settle();
    let first = h.rt.app.builds.get();
    assert!(first > 0);
    h.rt.invalidate();
    h.advance(1.0 / 60.0);
    assert_eq!(h.rt.app.builds.get(), first, "visible rows reused");
    assert!(h.rt.rect_of_text("row 0").is_some());
    let r = h.rt.rect_of("list").unwrap();
    for _ in 0..30 {
        h.event(charis_ui::Event::Wheel(r.center(), Point::new(0.0, 100.0)));
        h.advance(1.0 / 60.0);
    }
    h.settle();
    assert!(h.rt.rect_of_text("row 0").is_none());
    assert!(h.rt.app.builds.get() > first, "new rows built as they scroll in");
}
