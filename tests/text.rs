//! Rich text, read-only selection, copy and links.

use rust_ui::prelude::*;
use rust_ui::{Event, MouseButton};

#[derive(Default)]
struct T {
    links: Vec<String>,
}

#[derive(Clone, Debug)]
enum Msg {
    Link(String),
}

impl App for T {
    type Msg = Msg;
    fn update(&mut self, msg: Msg, _: &mut Cx<Msg>) {
        let Msg::Link(l) = msg;
        self.links.push(l);
    }
    fn view(&self) -> Element<Msg> {
        col().p(20.0).gap(20.0).child(text("alpha beta gamma delta").id("plain").selectable().w(400.0)).child(
            rich_text([
                span("Visit "),
                span("the docs").link("https://docs"),
                span(" now, "),
                span("RED").color(hex("#ff0000")).bold(),
            ])
            .id("rich")
            .selectable()
            .on_link(Msg::Link)
            .w(400.0),
        )
    }
}

fn setup() -> Headless<T> {
    let mut h = Headless::new(T::default(), 500.0, 200.0, 1.0);
    h.settle();
    h
}

fn key(h: &mut Headless<T>, c: char) {
    h.event(Event::Key(KeyEvent {
        key: Key::Char(c),
        mods: Modifiers { ctrl: true, meta: true, ..Default::default() },
        repeat: false,
    }));
}

#[test]
fn drag_select_and_copy() {
    let mut h = setup();
    let r = h.rt.rect_of("plain").unwrap();
    // Drag from the start to roughly after "alpha beta".
    h.drag((r.x + 1.0, r.center().y), (r.x + 68.0, r.center().y), 6);
    let sel = h.rt.selected_text().expect("selection");
    assert!(sel.starts_with("alpha b"), "selected {sel:?}");
    key(&mut h, 'c');
    assert_eq!(h.rt.clipboard(), sel);
    // Clicking elsewhere clears the selection.
    h.click(450.0, 180.0);
    assert!(h.rt.selected_text().is_none());
}

#[test]
fn double_and_triple_click() {
    let mut h = setup();
    let r = h.rt.rect_of("plain").unwrap();
    let p = (r.x + 60.0, r.center().y); // inside "beta"
    h.click(p.0, p.1);
    h.click(p.0, p.1);
    assert_eq!(h.rt.selected_text().as_deref(), Some("beta"));
    h.click(p.0, p.1);
    assert_eq!(h.rt.selected_text().as_deref(), Some("alpha beta gamma delta"));
}

#[test]
fn link_click_and_span_color() {
    let mut h = setup();
    let r = h.rt.rect_of("rich").unwrap();
    // "Visit " is ~35px wide; the link follows.
    h.move_to(r.x + 60.0, r.center().y);
    assert_eq!(h.rt.cursor(), Cursor::Pointer, "pointer over links");
    h.click(r.x + 60.0, r.center().y);
    assert_eq!(h.rt.app.links, vec!["https://docs".to_string()]);
    // Clicking plain text doesn't.
    h.click(r.x + 5.0, r.center().y);
    assert_eq!(h.rt.app.links.len(), 1);
    // The red span renders red.
    let pm = h.rt.render().clone();
    let reddish = (0..pm.width())
        .flat_map(|x| (r.y as u32..(r.y + r.h) as u32).map(move |y| (x, y)))
        .filter_map(|(x, y)| pm.pixel(x, y))
        .filter(|p| p.red() > 180 && p.green() < 80 && p.blue() < 80)
        .count();
    assert!(reddish > 10, "found {reddish} red pixels");
}
