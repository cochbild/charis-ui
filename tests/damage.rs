//! Damage tracking: a frame that redraws only what changed must be pixel
//! for pixel the same as a full redraw, through hover, typing, overlays,
//! scrolling, animations, layers, shadows and images; and small changes
//! must redraw small areas.

use charis_ui::damage::Damage;
use charis_ui::image::Image;
use charis_ui::prelude::*;
use charis_ui::{Event, MouseButton};

struct Ui {
    text: String,
    pick: Option<usize>,
    faded: bool,
    count: u32,
    img: Image,
}

#[derive(Clone)]
enum Msg {
    Text(String),
    Pick(usize),
    Fade,
    Inc,
}

impl App for Ui {
    type Msg = Msg;
    fn update(&mut self, m: Msg, _: &mut Cx<Msg>) {
        match m {
            Msg::Text(t) => self.text = t,
            Msg::Pick(i) => self.pick = Some(i),
            Msg::Fade => self.faded = !self.faded,
            Msg::Inc => self.count += 1,
        }
    }
    fn view(&self) -> Element<Msg> {
        let th = theme();
        col()
            .size_full()
            .p(16.0)
            .gap(12.0)
            .child(
                row()
                    .gap(8.0)
                    .child(primary_button("Increment").id("inc").on_click(Msg::Inc))
                    .child(button("Fade").id("fade").on_click(Msg::Fade))
                    .child(text(format!("Count: {}", self.count)).id("count")),
            )
            .child(text_input(self.text.clone(), Msg::Text).id("field").w(240.0))
            .child(pick_list(["One", "Two", "Three"], self.pick, Msg::Pick).id("pick").w(160.0))
            .child(
                card()
                    .w(260.0)
                    .opacity(if self.faded { 0.4 } else { 1.0 })
                    .transition(0.2)
                    .child(text("A card with a shadow").semibold())
                    .child(image(self.img.clone()).w(60.0).rounded(8.0)),
            )
            .child(col().id("list").h(90.0).scroll_y().border(1.0, th.colors.border).children(
                (0..40).map(|i| list_item(Some(Icon::File), format!("Row {i}"), i == 3).id(&format!("row{i}"))),
            ))
    }
}

fn app(scale: f32) -> Headless<Ui> {
    let mut px = Vec::new();
    for y in 0..20u32 {
        for x in 0..30u32 {
            px.extend_from_slice(&[(x * 8) as u8, (y * 12) as u8, 180, 255]);
        }
    }
    let img = Image::from_rgba(30, 20, &px).unwrap();
    let mut h = Headless::new(Ui { text: String::new(), pick: None, faded: false, count: 0, img }, 520.0, 460.0, scale);
    h.settle();
    h
}

/// The damage-tracked frame equals a full redraw of the same state.
fn check(h: &mut Headless<Ui>, step: &str) -> Damage {
    let partial = h.rt.render().clone();
    let damage = h.rt.last_damage();
    h.rt.set_damage_tracking(false);
    h.rt.invalidate();
    let full = h.rt.render().clone();
    h.rt.set_damage_tracking(true);
    if partial.data() != full.data() {
        let w = full.width() as usize;
        let bad = partial.data().chunks(4).zip(full.data().chunks(4)).position(|(a, b)| a != b).unwrap();
        let (x, y) = (bad % w, bad / w);
        panic!("{step}: partial redraw ({damage:?}) differs from full at ({x}, {y})");
    }
    // Next frame starts from the full one; make it a tracked frame again.
    h.rt.invalidate();
    let _ = h.rt.render();
    damage
}

fn run_script(scale: f32) {
    let mut h = app(scale);
    let _ = h.rt.render();
    let click = |h: &mut Headless<Ui>, id: &str| {
        let r = h.rt.rect_of(id).unwrap().center();
        h.click(r.x, r.y);
    };
    // Hover a button (animated background): frames mid-transition too.
    let inc = h.rt.rect_of("inc").unwrap().center();
    h.move_to(inc.x, inc.y);
    for i in 0..4 {
        h.advance(0.03);
        check(&mut h, &format!("hover frame {i}"));
    }
    click(&mut h, "inc");
    for i in 0..12 {
        h.advance(0.02);
        check(&mut h, &format!("increment frame {i}"));
    }
    // Typing, with the caret blinking.
    click(&mut h, "field");
    h.type_text("hello");
    h.advance(0.1);
    check(&mut h, "typed");
    h.advance(0.6);
    check(&mut h, "caret blink");
    // A dropdown overlay opens over other content, then closes.
    click(&mut h, "pick");
    h.settle();
    check(&mut h, "dropdown open");
    let two = h.rt.rect_of_text("Two").unwrap().center();
    h.click(two.x, two.y);
    h.settle();
    check(&mut h, "dropdown picked");
    // An opacity layer fading.
    click(&mut h, "fade");
    for i in 0..3 {
        h.advance(0.07);
        check(&mut h, &format!("fade frame {i}"));
    }
    h.settle();
    check(&mut h, "faded");
    // Scrolling a list.
    let l = h.rt.rect_of("list").unwrap().center();
    h.event(Event::Wheel(l, Point::new(0.0, 60.0)));
    for i in 0..4 {
        h.advance(0.05);
        check(&mut h, &format!("scroll frame {i}"));
    }
    // Pointer leaves everything.
    h.event(Event::PointerDown(Point::new(500.0, 440.0), MouseButton::Left));
    h.event(Event::PointerUp(Point::new(500.0, 440.0), MouseButton::Left));
    h.event(Event::PointerLeave);
    h.settle();
    check(&mut h, "leave");
}

#[test]
fn partial_redraws_match_full_redraws() {
    for scale in [1.0, 1.5, 2.0] {
        run_script(scale);
    }
}

#[test]
fn small_changes_redraw_small_areas() {
    let mut h = app(1.0);
    let _ = h.rt.render();
    // Nothing changed: nothing redrawn.
    h.rt.invalidate();
    let _ = h.rt.render();
    assert_eq!(h.rt.last_damage(), Damage::None);
    // The counter's text changes: a small area around it.
    h.rt.send(Msg::Inc);
    let _ = h.rt.render();
    let Damage::Area(a) = h.rt.last_damage() else { panic!("{:?}", h.rt.last_damage()) };
    let count = h.rt.rect_of("count").unwrap();
    assert!(a.w * a.h < 520.0 * 460.0 * 0.1, "{a:?}");
    assert!(a.intersect(&count).w > 0.0, "{a:?} covers {count:?}");
    // A resize redraws everything.
    h.rt.resize(Size::new(500.0, 460.0), 1.0);
    let _ = h.rt.render();
    assert_eq!(h.rt.last_damage(), Damage::Full);
}

/// Random pointer moves, clicks, wheel scrolls and typing, checking every
/// frame, at several scales.
#[test]
fn random_interaction_matches_full_redraws() {
    for scale in [1.0, 1.25, 1.5, 2.0] {
        let mut h = app(scale);
        let _ = h.rt.render();
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15 ^ (scale * 100.0) as u64;
        let mut rnd = move |n: u32| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as f32
        };
        for step in 0..150 {
            let p = Point::new(rnd(520), rnd(460));
            match rnd(10) as u32 {
                0..=4 => h.move_to(p.x, p.y),
                5 | 6 => h.click(p.x, p.y),
                7 => h.event(Event::Wheel(p, Point::new(0.0, rnd(120) - 60.0))),
                8 => h.type_text("x"),
                _ => h.event(Event::Key(KeyEvent { key: Key::Down, mods: Modifiers::default(), repeat: false })),
            }
            h.advance(0.016 + rnd(80) as f64 / 1000.0);
            check(&mut h, &format!("scale {scale} step {step}"));
        }
    }
}
