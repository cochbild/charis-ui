//! Undo/redo in text inputs.

use rust_ui::prelude::*;
use rust_ui::Event;

#[derive(Default)]
struct Ed {
    value: String,
}

#[derive(Clone, Debug)]
enum Msg {
    Input(String),
    Clear,
}

impl App for Ed {
    type Msg = Msg;
    fn update(&mut self, msg: Msg, _: &mut Cx<Msg>) {
        match msg {
            Msg::Input(s) => self.value = s,
            Msg::Clear => self.value.clear(),
        }
    }
    fn view(&self) -> Element<Msg> {
        col().p(20.0).child(text_input(self.value.clone(), Msg::Input).id("ed").w(300.0).on_submit(Msg::Clear))
    }
}

fn setup() -> Headless<Ed> {
    let mut h = Headless::new(Ed::default(), 400.0, 200.0, 1.0);
    h.settle();
    let r = h.rt.rect_of("ed").unwrap();
    h.click(r.center().x, r.center().y);
    h
}

fn key(h: &mut Headless<Ed>, k: Key, ctrl: bool, shift: bool) {
    h.event(Event::Key(KeyEvent {
        key: k,
        mods: Modifiers { ctrl, shift, meta: cfg!(target_os = "macos") && ctrl, ..Default::default() },
        repeat: false,
    }));
}

/// Type like a keyboard: one text event per character.
fn typ(h: &mut Headless<Ed>, s: &str) {
    for c in s.chars() {
        h.type_text(&c.to_string());
    }
}

fn undo(h: &mut Headless<Ed>) {
    key(h, Key::Char('z'), true, false);
}

fn redo(h: &mut Headless<Ed>) {
    key(h, Key::Char('z'), true, true);
}

#[test]
fn typing_undoes_word_by_word_and_redoes() {
    let mut h = setup();
    typ(&mut h, "hello world again");
    undo(&mut h);
    assert_eq!(h.rt.app.value, "hello world");
    undo(&mut h);
    assert_eq!(h.rt.app.value, "hello");
    undo(&mut h);
    assert_eq!(h.rt.app.value, "");
    undo(&mut h);
    assert_eq!(h.rt.app.value, "", "nothing left to undo");
    redo(&mut h);
    assert_eq!(h.rt.app.value, "hello");
    key(&mut h, Key::Char('y'), true, false);
    assert_eq!(h.rt.app.value, "hello world");
}

#[test]
fn pauses_deletes_and_pastes_are_separate_steps() {
    let mut h = setup();
    typ(&mut h, "abc");
    h.advance(1.5);
    typ(&mut h, "def");
    key(&mut h, Key::Backspace, false, false);
    key(&mut h, Key::Backspace, false, false);
    assert_eq!(h.rt.app.value, "abcd");
    h.event(Event::Paste("XYZ".into()));
    assert_eq!(h.rt.app.value, "abcdXYZ");
    undo(&mut h);
    assert_eq!(h.rt.app.value, "abcd", "paste undone alone");
    undo(&mut h);
    assert_eq!(h.rt.app.value, "abcdef", "both backspaces undone together");
    undo(&mut h);
    assert_eq!(h.rt.app.value, "abc", "the pause split the typing");
    // A new edit clears the redo stack.
    typ(&mut h, "!");
    redo(&mut h);
    assert_eq!(h.rt.app.value, "abc!");
}

#[test]
fn app_side_changes_reset_the_history() {
    let mut h = setup();
    typ(&mut h, "sent message");
    key(&mut h, Key::Enter, false, false);
    assert_eq!(h.rt.app.value, "");
    typ(&mut h, "next");
    undo(&mut h);
    assert_eq!(h.rt.app.value, "", "undo stops at the app's reset");
    undo(&mut h);
    assert_eq!(h.rt.app.value, "", "the sent text is not resurrected");
}

#[test]
fn undo_restores_the_selection() {
    let mut h = setup();
    typ(&mut h, "keep this");
    key(&mut h, Key::Char('a'), true, false);
    typ(&mut h, "replaced");
    undo(&mut h);
    assert_eq!(h.rt.app.value, "keep this");
    // The whole text is selected again, so typing replaces it.
    typ(&mut h, "x");
    assert_eq!(h.rt.app.value, "x");
}
