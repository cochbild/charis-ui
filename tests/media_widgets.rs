//! Images, SVGs, radio groups and number inputs.

use charis_ui::image::{Fit, Image};
use charis_ui::prelude::*;
use charis_ui::Event;

/// `w`×`h`, left half red, right half blue.
fn halves(w: u32, h: u32) -> Image {
    let mut px = Vec::new();
    for _ in 0..h {
        for x in 0..w {
            px.extend_from_slice(if x < w / 2 { &[255, 0, 0, 255] } else { &[0, 0, 255, 255] });
        }
    }
    Image::from_rgba(w, h, &px).unwrap()
}

struct Pics {
    img: Image,
}

#[derive(Clone)]
enum Msg {}

impl App for Pics {
    type Msg = Msg;
    fn update(&mut self, _: Msg, _: &mut Cx<Msg>) {}
    fn view(&self) -> Element<Msg> {
        col()
            .items(Align::Start)
            .gap(10.0)
            .p(10.0)
            .child(image(self.img.clone()).id("natural"))
            .child(image(self.img.clone()).id("wide").w(200.0))
            .child(image(self.img.clone()).id("cover").w(100.0).h(100.0).fit(Fit::Cover).rounded(20.0))
            .child(image(self.img.clone()).id("contain").w(100.0).h(100.0).fit(Fit::Contain))
    }
}

#[test]
fn images_size_fit_and_round() {
    let mut h = Headless::new(Pics { img: halves(40, 20) }, 400.0, 400.0, 1.0);
    h.settle();
    let r = |h: &Headless<Pics>, id: &str| h.rt.rect_of(id).unwrap();
    assert_eq!((r(&h, "natural").w, r(&h, "natural").h), (40.0, 20.0));
    assert_eq!((r(&h, "wide").w, r(&h, "wide").h), (200.0, 100.0), "aspect ratio kept");
    // Cover: the middle of a 2:1 image fills the square (red left, blue right).
    let c = r(&h, "cover");
    let left = h.pixel(c.x + 25.0, c.center().y);
    let right = h.pixel(c.x + 75.0, c.center().y);
    assert!(left[0] > 200 && left[2] < 60, "{left:?}");
    assert!(right[2] > 200 && right[0] < 60, "{right:?}");
    // Rounded corner: background, not image.
    let corner = h.pixel(c.x + 1.0, c.y + 1.0);
    assert!(corner[0] < 60 && corner[2] < 60, "{corner:?}");
    // Contain: a 100×50 band in the middle; above it is background.
    let k = r(&h, "contain");
    assert!(h.pixel(k.x + 25.0, k.y + 10.0)[0] < 60);
    assert!(h.pixel(k.x + 25.0, k.center().y)[0] > 200);
    // Crisp at 2x too: the raster is made for the device size.
    let mut h2 = Headless::new(Pics { img: halves(40, 20) }, 400.0, 400.0, 2.0);
    h2.settle();
    let w = h2.rt.rect_of("wide").unwrap();
    assert!(h2.pixel(w.x + 50.0, w.center().y)[0] > 240);
}

#[cfg(feature = "svg")]
#[test]
fn svg_renders_and_tints() {
    use charis_ui::image::Svg;
    struct S(Svg);
    impl App for S {
        type Msg = Msg;
        fn update(&mut self, _: Msg, _: &mut Cx<Msg>) {}
        fn view(&self) -> Element<Msg> {
            row()
                .items(Align::Start)
                .child(svg(&self.0).id("plain").w(96.0))
                .child(svg(&self.0).id("tinted").w(96.0).tint(Color::rgb(0, 255, 0)))
        }
    }
    let s = Svg::parse(r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="12"><rect width="24" height="12" fill="#ff0000"/></svg>"##)
        .unwrap();
    assert_eq!(s.size(), (24.0, 12.0));
    let mut h = Headless::new(S(s), 300.0, 100.0, 1.0);
    h.settle();
    let p = h.rt.rect_of("plain").unwrap();
    assert_eq!((p.w, p.h), (96.0, 48.0));
    assert_eq!(h.pixel(p.center().x, p.center().y)[..3], [255, 0, 0]);
    let t = h.rt.rect_of("tinted").unwrap();
    assert_eq!(h.pixel(t.center().x, t.center().y)[..3], [0, 255, 0]);
}

struct Form {
    choice: Option<usize>,
    size: f64,
}

#[derive(Clone)]
enum F {
    Pick(usize),
    Size(f64),
}

impl App for Form {
    type Msg = F;
    fn update(&mut self, m: F, _: &mut Cx<F>) {
        match m {
            F::Pick(i) => self.choice = Some(i),
            F::Size(v) => self.size = v,
        }
    }
    fn view(&self) -> Element<F> {
        col()
            .p(10.0)
            .gap(12.0)
            .child(radio_group(["Small", "Medium", "Large"], self.choice, F::Pick).id("sizes"))
            .child(
                number_input("font-size", self.size, F::Size).range(8.0, 72.0).step(0.5).decimals(1).label("Font size"),
            )
            .child(button("elsewhere").id("elsewhere"))
    }
}

fn key(h: &mut Headless<Form>, key: Key) {
    h.event(Event::Key(KeyEvent { key, mods: Modifiers::default(), repeat: false }));
    h.settle();
}

#[test]
fn radio_group_by_pointer_and_keys() {
    let mut h = Headless::new(Form { choice: None, size: 13.0 }, 400.0, 300.0, 1.0);
    h.settle();
    let m = h.rt.rect_of_text("Medium").unwrap().center();
    h.click(m.x, m.y);
    h.settle();
    assert_eq!(h.rt.app.choice, Some(1));
    // The group has focus now: arrows move the choice, wrapping.
    key(&mut h, Key::Down);
    assert_eq!(h.rt.app.choice, Some(2));
    key(&mut h, Key::Down);
    assert_eq!(h.rt.app.choice, Some(0));
    key(&mut h, Key::Up);
    assert_eq!(h.rt.app.choice, Some(2));
    key(&mut h, Key::Home);
    assert_eq!(h.rt.app.choice, Some(0));
}

#[test]
fn number_input_typing_stepping_and_clamping() {
    let mut h = Headless::new(Form { choice: None, size: 13.0 }, 400.0, 300.0, 1.0);
    h.settle();
    assert!(h.rt.rect_of_text("13.0").is_none(), "an input's value isn't a text node");
    // Steppers.
    let plus = h.rt.rect_of("font-size/field").unwrap();
    h.click(plus.right() + 13.0, plus.center().y);
    h.settle();
    assert_eq!(h.rt.app.size, 13.5);
    h.click(plus.x - 13.0, plus.center().y);
    h.click(plus.x - 13.0, plus.center().y);
    h.settle();
    assert_eq!(h.rt.app.size, 12.5);
    // Type into the field: select all, type; values apply as they parse.
    h.click(plus.center().x, plus.center().y);
    h.event(Event::Key(KeyEvent {
        key: Key::Char('a'),
        mods: Modifiers { ctrl: true, meta: cfg!(target_os = "macos"), ..Default::default() },
        repeat: false,
    }));
    h.type_text("2");
    h.settle();
    assert_eq!(h.rt.app.size, 8.0, "2 clamps to the minimum while typing");
    h.type_text("4.");
    h.settle();
    assert_eq!(h.rt.app.size, 24.0, "'24.' parses as 24");
    // Arrow keys step from the value.
    key(&mut h, Key::Up);
    assert_eq!(h.rt.app.size, 24.5);
    key(&mut h, Key::PageDown);
    assert_eq!(h.rt.app.size, 19.5);
    // Too big: clamped when leaving the field.
    h.event(Event::Key(KeyEvent {
        key: Key::Char('a'),
        mods: Modifiers { ctrl: true, meta: cfg!(target_os = "macos"), ..Default::default() },
        repeat: false,
    }));
    h.type_text("999");
    h.settle();
    assert_eq!(h.rt.app.size, 72.0);
    let e = h.rt.rect_of("elsewhere").unwrap().center();
    h.click(e.x, e.y);
    h.settle();
    assert_eq!(h.rt.app.size, 72.0);
    // At the maximum, + does nothing.
    h.click(plus.right() + 13.0, plus.center().y);
    h.settle();
    assert_eq!(h.rt.app.size, 72.0);
}
