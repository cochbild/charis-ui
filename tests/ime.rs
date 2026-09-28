//! IME composition (pre-edit): shown inline, keys go to the IME, commits insert.

use charis_ui::prelude::*;
use charis_ui::Event;

#[derive(Default)]
struct Form {
    name: String,
    notes: String,
}

#[derive(Clone, Debug)]
enum Msg {
    Name(String),
    Notes(String),
}

impl App for Form {
    type Msg = Msg;
    fn update(&mut self, msg: Msg, _: &mut Cx<Msg>) {
        match msg {
            Msg::Name(s) => self.name = s,
            Msg::Notes(s) => self.notes = s,
        }
    }
    fn view(&self) -> Element<Msg> {
        col()
            .p(20.0)
            .gap(12.0)
            .child(text_input(self.name.clone(), Msg::Name).id("name").w(300.0))
            .child(text_area(self.notes.clone(), Msg::Notes).id("notes").w(300.0).rows(2, 4))
            .child(button("Not an input").id("btn"))
    }
}

fn focus(h: &mut Headless<Form>, id: &str) {
    let r = h.rt.rect_of(id).unwrap();
    h.click(r.center().x, r.center().y);
}

fn preedit(h: &mut Headless<Form>, t: &str) {
    let n = t.len();
    h.event(Event::Preedit { text: t.into(), cursor: Some((n, n)) });
}

#[test]
fn composition_is_shown_but_not_committed_until_the_ime_commits() {
    let mut h = Headless::new(Form::default(), 400.0, 300.0, 1.0);
    h.settle();
    assert!(!h.rt.text_input_focused());
    focus(&mut h, "name");
    assert!(h.rt.text_input_focused());
    h.type_text("Hi ");
    let caret0 = h.rt.ime_cursor_area().unwrap();

    preedit(&mut h, "に");
    preedit(&mut h, "にほ");
    assert_eq!(h.rt.preedit_text(), Some("にほ"));
    assert_eq!(h.rt.app.name, "Hi ", "composition is not part of the value yet");
    let caret1 = h.rt.ime_cursor_area().unwrap();
    assert!(caret1.x > caret0.x + 10.0, "candidate window follows the composition caret");

    // Editing keys belong to the IME while composing.
    h.event(Event::Key(KeyEvent { key: Key::Backspace, mods: Modifiers::default(), repeat: false }));
    assert_eq!(h.rt.app.name, "Hi ");

    h.event(Event::Text("日本".into()));
    assert_eq!(h.rt.app.name, "Hi 日本");
    assert_eq!(h.rt.preedit_text(), None);
    // Keys work again after the commit.
    h.event(Event::Key(KeyEvent { key: Key::Backspace, mods: Modifiers::default(), repeat: false }));
    assert_eq!(h.rt.app.name, "Hi 日");
}

#[test]
fn composition_is_painted_inline() {
    let mut h = Headless::new(Form::default(), 400.0, 300.0, 1.0);
    h.settle();
    focus(&mut h, "name");
    let r = h.rt.rect_of("name").unwrap();
    let ink = |h: &Headless<Form>| {
        let mut n = 0;
        for x in (r.x as i32 + 12)..(r.x as i32 + 120) {
            for y in (r.y as i32 + 4)..(r.y + r.h - 4.0) as i32 {
                let p = h.pixel(x as f32, y as f32);
                // Low enough to count thin, antialiased CJK strokes from any system font.
                if p[0] > 80 {
                    n += 1;
                }
            }
        }
        n
    };
    let empty = ink(&h);
    preedit(&mut h, "ありがとう");
    h.advance(0.01);
    assert!(ink(&h) > empty + 100, "composition text and underline are drawn");
    // Cancelling the composition (empty pre-edit) removes it.
    preedit(&mut h, "");
    h.advance(0.01);
    assert!(ink(&h) < empty + 20);
    assert_eq!(h.rt.app.name, "");
}

#[test]
fn multiline_composition_and_focus_change_cancels_it() {
    let mut h = Headless::new(Form::default(), 400.0, 300.0, 1.0);
    h.settle();
    focus(&mut h, "notes");
    h.type_text("line one");
    h.event(Event::Key(KeyEvent { key: Key::Enter, mods: Modifiers::default(), repeat: false }));
    let before = h.rt.ime_cursor_area().unwrap();
    preedit(&mut h, "한국");
    let after = h.rt.ime_cursor_area().unwrap();
    assert!(after.x > before.x && (after.y - before.y).abs() < 1.0, "caret moves along the second line");

    focus(&mut h, "btn");
    assert_eq!(h.rt.preedit_text(), None, "moving focus drops the composition");
    assert!(!h.rt.text_input_focused());
    assert_eq!(h.rt.app.notes, "line one\n");
}
