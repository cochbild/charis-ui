//! Multi-line text editing (chat composer style).

use rust_ui::prelude::*;
use rust_ui::Event;

#[derive(Default)]
struct Ed {
    text: String,
    sent: Vec<String>,
    submit_mode: bool,
}

#[derive(Clone, Debug)]
enum Msg {
    Input(String),
    Send,
}

impl App for Ed {
    type Msg = Msg;
    fn update(&mut self, msg: Msg, _: &mut Cx<Msg>) {
        match msg {
            Msg::Input(s) => self.text = s,
            Msg::Send => {
                self.sent.push(std::mem::take(&mut self.text));
            }
        }
    }
    fn view(&self) -> Element<Msg> {
        let mut e = text_area(self.text.clone(), Msg::Input).id("ed").w(300.0).rows(1, 4).placeholder("Message");
        if self.submit_mode {
            e = e.submit_on_enter(Msg::Send);
        }
        col().p(20.0).child(e)
    }
}

fn key(h: &mut Headless<Ed>, k: Key, shift: bool) {
    h.event(Event::Key(KeyEvent { key: k, mods: Modifiers { shift, ..Default::default() }, repeat: false }));
}

fn setup(submit_mode: bool) -> Headless<Ed> {
    let mut h = Headless::new(Ed { submit_mode, ..Default::default() }, 400.0, 400.0, 1.0);
    h.settle();
    let r = h.rt.rect_of("ed").unwrap();
    h.click(r.center().x, r.center().y);
    h
}

#[test]
fn newline_and_growth() {
    let mut h = setup(false);
    let h1 = h.rt.rect_of("ed").unwrap().h;
    h.type_text("first");
    key(&mut h, Key::Enter, false);
    h.type_text("second");
    assert_eq!(h.rt.app.text, "first\nsecond");
    let h2 = h.rt.rect_of("ed").unwrap().h;
    assert!(h2 > h1 + 10.0, "grows with lines: {h1} -> {h2}");
    for _ in 0..6 {
        key(&mut h, Key::Enter, false);
        h.type_text("x");
    }
    let h3 = h.rt.rect_of("ed").unwrap().h;
    let line = 13.0 * 1.5;
    // 4 rows + 2×7px padding + 2×1px border.
    assert!(h3 <= 4.0 * line + 16.0 + 0.5, "capped at 4 rows: {h3}");
}

#[test]
fn enter_submits_shift_enter_newline() {
    let mut h = setup(true);
    h.type_text("hello");
    key(&mut h, Key::Enter, true);
    h.type_text("world");
    assert_eq!(h.rt.app.text, "hello\nworld");
    key(&mut h, Key::Enter, false);
    assert_eq!(h.rt.app.sent, vec!["hello\nworld".to_string()]);
    assert_eq!(h.rt.app.text, "");
}

#[test]
fn vertical_movement_and_line_keys() {
    let mut h = setup(false);
    h.type_text("line one");
    key(&mut h, Key::Enter, false);
    h.type_text("two");
    key(&mut h, Key::Up, false);
    h.type_text("|");
    // Caret moved to the same x on the previous line (after "lin"-ish).
    let first = h.rt.app.text.lines().next().unwrap().to_string();
    assert!(first.contains('|') && !first.ends_with('|'), "up moved into line one: {first:?}");
    key(&mut h, Key::End, false);
    h.type_text("!");
    assert!(h.rt.app.text.lines().next().unwrap().ends_with('!'));
    key(&mut h, Key::Down, false);
    key(&mut h, Key::Home, false);
    h.type_text(">");
    assert!(h.rt.app.text.lines().nth(1).unwrap().starts_with('>'), "{:?}", h.rt.app.text);
}

#[test]
fn paste_keeps_newlines_and_click_positions() {
    let mut h = setup(false);
    h.event(Event::Paste("a\r\nb\nc".into()));
    assert_eq!(h.rt.app.text, "a\nb\nc");
    // Click at the start of the second line and type.
    let r = h.rt.rect_of("ed").unwrap();
    let line = 13.0 * 1.5;
    h.click(r.x + 11.0, r.y + 7.0 + line * 1.5);
    h.type_text("*");
    assert_eq!(h.rt.app.text, "a\n*b\nc");
}
