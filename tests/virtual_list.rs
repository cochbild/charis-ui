//! Virtualized lists: only visible rows are built, variable heights are
//! measured and cached, and scrolling stays anchored.

use rust_ui::prelude::*;
use rust_ui::runtime::Event;

struct List {
    count: usize,
    /// Row height pattern: uniform when false.
    varied: bool,
    follow: bool,
    clicked: Option<usize>,
}

#[derive(Clone, Debug)]
enum Msg {
    Click(usize),
}

fn row_h(i: usize, varied: bool) -> f32 {
    if varied {
        20.0 + (i % 7) as f32 * 13.0
    } else {
        24.0
    }
}

impl App for List {
    type Msg = Msg;
    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        match msg {
            Msg::Click(i) => self.clicked = Some(i),
        }
    }
    fn view(&self) -> Element<Msg> {
        let varied = self.varied;
        let list = virtual_list(self.count, move |i| {
            row().h(row_h(i, varied)).items_center().px(8.0).on_click(Msg::Click(i)).child(text(format!("Row {i}")))
        })
        .id("list")
        .item_height(24.0)
        .gap(2.0)
        .p(4.0)
        .grow(1.0);
        col().size_full().child(if self.follow { list.follow_end() } else { list })
    }
}

fn app(count: usize, varied: bool) -> List {
    List { count, varied, follow: false, clicked: None }
}

fn list_top(h: &Headless<List>) -> f32 {
    h.rt.rect_of("list").unwrap().y + 4.0
}

#[test]
fn builds_only_visible_rows_of_a_huge_list() {
    let mut h = Headless::new(app(100_000, false), 400.0, 600.0, 1.0);
    h.settle();
    let built = h.rt.virtual_rows_built("list").unwrap();
    // 600px viewport + 300px overscan below, rows 26px apart.
    assert!(built > 20 && built < 60, "built {built} rows");
    assert!(h.rt.rect_of_text("Row 0").is_some());
    assert!(h.rt.rect_of_text("Row 500").is_none());

    h.rt.scroll_to_item("list", 50_000);
    h.settle();
    let r = h.rt.rect_of_text("Row 50000").expect("row 50000 built after scroll_to_item");
    let row = r.y - (24.0 - r.h) / 2.0;
    assert!((row - list_top(&h)).abs() < 1.0, "row 50000 at {row}, list top {}", list_top(&h));
    assert!(h.rt.rect_of_text("Row 0").is_none());
    assert!(h.rt.virtual_rows_built("list").unwrap() < 60);
}

#[test]
fn wheel_scrolling_builds_new_rows_and_clicks_hit_the_right_row() {
    let mut h = Headless::new(app(10_000, false), 400.0, 600.0, 1.0);
    h.settle();
    for _ in 0..40 {
        h.event(Event::Wheel(Point::new(200.0, 300.0), Point::new(0.0, 100.0)));
        h.advance(1.0 / 60.0);
    }
    h.settle();
    // 4000px down at 26px per row is around row 153.
    let r = h.rt.rect_of_text("Row 160").expect("row 160 visible after scrolling");
    assert!(r.y > 0.0 && r.y < 600.0);
    h.click(100.0, r.y + r.h / 2.0);
    assert_eq!(h.rt.app.clicked, Some(160));
}

#[test]
fn variable_heights_are_measured_and_scrolling_stays_anchored() {
    let mut h = Headless::new(app(5_000, true), 400.0, 600.0, 1.0);
    h.settle();
    // Jump deep into rows that have never been measured.
    h.rt.scroll_to_item("list", 2_000);
    h.settle();
    let r = h.rt.rect_of_text("Row 2000").expect("row 2000");
    let top = r.y - (row_h(2000, true) - r.h) / 2.0;
    assert!((top - list_top(&h)).abs() < 1.0, "row 2000 at {top}, want {}", list_top(&h));

    // Scrolling up reveals unmeasured rows above; the rows already on screen
    // must move by exactly the scrolled distance (no jumps from re-measuring).
    let before = h.rt.rect_of_text("Row 2003").unwrap().y;
    h.event(Event::Wheel(Point::new(200.0, 300.0), Point::new(0.0, -400.0)));
    h.settle();
    let after = h.rt.rect_of_text("Row 2003").unwrap().y;
    assert!((after - before - 400.0).abs() < 1.0, "row moved {} instead of 400", after - before);
}

#[test]
fn follow_end_keeps_the_last_row_visible_as_rows_arrive() {
    let mut a = app(1_000, true);
    a.follow = true;
    let mut h = Headless::new(a, 400.0, 600.0, 1.0);
    h.settle();
    let list = h.rt.rect_of("list").unwrap();
    let last = h.rt.rect_of_text("Row 999").expect("last row visible at start");
    assert!(last.y + last.h <= list.y + list.h && last.y > list.y + list.h - 80.0);
    for n in [1_001, 1_010, 1_200] {
        h.rt.app.count = n;
        h.rt.invalidate();
        h.advance(1.0 / 60.0);
        let label = format!("Row {}", n - 1);
        let last = h.rt.rect_of_text(&label).unwrap_or_else(|| panic!("{label} not built on the first frame"));
        assert!(last.y + last.h <= list.y + list.h + 0.5, "{label} below the viewport");
        assert!(last.y > list.y + list.h - 80.0, "{label} not at the bottom: {}", last.y);
    }
}

#[test]
fn frame_cost_does_not_grow_with_row_count() {
    let time = |count: usize| {
        let mut h = Headless::new(app(count, true), 400.0, 600.0, 1.0);
        h.settle();
        h.rt.scroll_to_item("list", count / 2);
        h.settle();
        let t = std::time::Instant::now();
        for _ in 0..20 {
            h.event(Event::Wheel(Point::new(200.0, 300.0), Point::new(0.0, 60.0)));
            h.advance(1.0 / 60.0);
        }
        t.elapsed().as_secs_f64() / 40.0
    };
    let small = time(100);
    let huge = time(200_000);
    eprintln!("frame: 100 rows {:.2}ms, 200k rows {:.2}ms", small * 1e3, huge * 1e3);
    // Only the O(n) height sum grows; allow generous slack for noisy CI machines.
    assert!(huge < small * 4.0 + 0.004, "200k rows: {huge:.4}s/frame vs 100 rows: {small:.4}s/frame");
}
