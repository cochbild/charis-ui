//! Multi-line inputs with very large documents: only what's on screen is
//! laid out, and editing, scrolling, selection and undo stay correct.

use rust_ui::prelude::*;
use rust_ui::Event;

struct Ed {
    text: String,
}

#[derive(Clone)]
enum Msg {
    Edit(String),
}

impl App for Ed {
    type Msg = Msg;
    fn update(&mut self, m: Msg, _: &mut Cx<Msg>) {
        let Msg::Edit(t) = m;
        self.text = t;
    }
    fn view(&self) -> Element<Msg> {
        col().size_full().child(text_area(self.text.clone(), Msg::Edit).id("ed").mono().size_full())
    }
}

fn doc(lines: usize) -> String {
    (0..lines).map(|i| format!("line {i:06}\n")).collect()
}

fn open(lines: usize) -> Headless<Ed> {
    let mut h = Headless::new(Ed { text: doc(lines) }, 600.0, 400.0, 1.0);
    h.settle();
    let r = h.rt.rect_of("ed").unwrap();
    h.click(r.x + 20.0, r.y + 12.0);
    h.settle();
    h
}

fn key(h: &mut Headless<Ed>, key: Key, ctrl: bool, shift: bool) {
    h.event(Event::Key(KeyEvent { key, mods: Modifiers { ctrl, shift, ..Default::default() }, repeat: false }));
    h.settle();
}

/// The line the caret is on, found by typing a marker and looking for it.
fn caret_line(h: &mut Headless<Ed>) -> usize {
    h.type_text("§");
    h.settle();
    let line = h.rt.app.text.lines().position(|l| l.contains('§')).expect("marker");
    key(h, Key::Backspace, false, false);
    line
}

#[test]
fn a_100k_line_document_opens_and_edits_at_both_ends() {
    let mut h = open(100_000);
    assert_eq!(caret_line(&mut h), 0);
    // Ctrl+A then Right: the caret goes to the very end, and the view follows.
    key(&mut h, Key::Char('a'), true, false);
    key(&mut h, Key::Right, false, false);
    h.type_text("END");
    h.settle();
    assert!(h.rt.app.text.ends_with("line 099999\nEND"));
    let r = h.rt.rect_of("ed").unwrap();
    let caret = h.rt.ime_cursor_area().expect("caret");
    assert!(r.contains(caret.center()), "the caret {caret:?} is scrolled into view in {r:?}");
    // Arrow keys move between paragraphs near the end.
    key(&mut h, Key::Up, false, false);
    key(&mut h, Key::Up, false, false);
    assert_eq!(caret_line(&mut h), 99_998);
}

#[test]
fn wheel_scrolling_and_clicking_hit_the_right_line() {
    let mut h = open(100_000);
    let r = h.rt.rect_of("ed").unwrap();
    let lh = 13.0 * 1.5;
    // Scroll down about 1000 lines, then click the first visible line.
    for _ in 0..50 {
        h.event(Event::Wheel(r.center(), Point::new(0.0, 20.0 * lh)));
    }
    h.settle();
    h.click(r.x + 20.0, r.y + 7.0 + lh * 0.5);
    h.settle();
    let line = caret_line(&mut h);
    assert!((990..=1010).contains(&line), "clicked line {line} after scrolling ~1000 lines");
    // The click didn't jump the view: the caret is where the pointer was.
    let caret = h.rt.ime_cursor_area().unwrap();
    assert!((caret.y - (r.y + 7.0)).abs() < lh * 1.5, "caret at {caret:?}");
}

#[test]
fn selections_across_many_paragraphs_delete_and_undo_exactly() {
    let mut h = open(20_000);
    let original = h.rt.app.text.clone();
    // Select from the start of line 0 down 500 lines, delete, then undo.
    for _ in 0..500 {
        h.event(Event::Key(KeyEvent {
            key: Key::Down,
            mods: Modifiers { shift: true, ..Default::default() },
            repeat: true,
        }));
    }
    h.settle();
    key(&mut h, Key::Backspace, false, false);
    assert!(h.rt.app.text.starts_with("line 000500\n"), "{:?}", &h.rt.app.text[..24]);
    assert_eq!(h.rt.app.text.len(), original.len() - 500 * 12);
    key(&mut h, Key::Char('z'), true, false);
    assert_eq!(h.rt.app.text, original, "undo restores the document");
    key(&mut h, Key::Char('z'), true, true);
    assert!(h.rt.app.text.starts_with("line 000500\n"), "redo deletes again");
}

#[test]
fn wrapped_paragraphs_move_by_visual_line() {
    let long = "word ".repeat(200);
    let mut h = Headless::new(Ed { text: format!("{long}\nshort\n") }, 600.0, 400.0, 1.0);
    h.settle();
    let r = h.rt.rect_of("ed").unwrap();
    h.click(r.x + 20.0, r.y + 12.0);
    h.settle();
    // Down moves within the wrapped first paragraph before reaching "short".
    key(&mut h, Key::Down, false, false);
    assert_eq!(caret_line(&mut h), 0, "still inside the wrapped paragraph");
    for _ in 0..40 {
        key(&mut h, Key::Down, false, false);
    }
    assert_eq!(caret_line(&mut h), 2, "past the wrapped paragraph and 'short'");
}
