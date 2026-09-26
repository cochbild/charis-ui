//! Developer tools: the element inspector and stylesheets.

use rust_ui::prelude::*;
use rust_ui::stylesheet::Stylesheet;
use rust_ui::Event;

struct A {
    clicks: u32,
}

#[derive(Clone)]
enum Msg {
    Click,
}

impl App for A {
    type Msg = Msg;
    fn update(&mut self, _: Msg, _: &mut Cx<Msg>) {
        self.clicks += 1;
    }
    fn view(&self) -> Element<Msg> {
        col()
            .p(20.0)
            .gap(12.0)
            .child(primary_button("Save").id("save").on_click(Msg::Click))
            .child(card().id("card").w(200.0).h(80.0).child(text("Card")))
    }
}

fn app() -> Headless<A> {
    let mut h = Headless::new(A { clicks: 0 }, 700.0, 400.0, 1.0);
    h.rt.set_inspector_enabled(true);
    h.settle();
    h
}

fn key(h: &mut Headless<A>, key: Key, mods: Modifiers) {
    h.event(Event::Key(KeyEvent { key, mods, repeat: false }));
    h.settle();
}

fn texts(h: &Headless<A>, s: &str) -> bool {
    h.rt.rect_of_text(s).is_some()
}

#[test]
fn inspector_points_pins_and_describes() {
    let mut h = app();
    key(&mut h, Key::F(12), Modifiers::default());
    assert!(h.rt.inspector_open());
    let save = h.rt.rect_of("save").unwrap();
    // The innermost element under the pointer: the button's text…
    h.move_to(save.center().x, save.center().y);
    h.settle();
    assert!(h.rt.rect_of("__inspector_label").is_some());
    // …and in its padding, the button itself. The label sits next to it
    // (below here: no room above).
    h.move_to(save.x + 4.0, save.center().y);
    h.settle();
    let label = h.rt.rect_of("__inspector_label").expect("label shown");
    assert!(label.bottom() <= save.y + 0.5 || label.y >= save.bottom() - 0.5, "{label:?} next to {save:?}");
    // Clicking pins it, and doesn't reach the app.
    h.click(save.x + 4.0, save.center().y);
    h.settle();
    assert_eq!(h.rt.app.clicks, 0);
    assert!(h.rt.rect_of("__inspector_title").is_some());
    assert_eq!(h.rt.inspected().as_deref(), Some("button.button.button-primary #save"), "role, classes and id");
    assert!(texts(&h, "button.button.button-primary #save"), "the panel's title");
    assert!(texts(&h, "handlers"));
    assert!(texts(&h, "click"), "its handlers are listed");
    // Moving away keeps the pinned element described.
    let card = h.rt.rect_of("card").unwrap();
    h.move_to(card.center().x, card.center().y);
    h.settle();
    assert!(texts(&h, "button.button.button-primary #save"));
    // Escape closes; clicks reach the app again.
    key(&mut h, Key::Escape, Modifiers::default());
    assert!(!h.rt.inspector_open());
    h.click(save.center().x, save.center().y);
    h.settle();
    assert_eq!(h.rt.app.clicks, 1);
    // Ctrl+Shift+I toggles too; disabled, it doesn't open.
    key(&mut h, Key::Char('I'), Modifiers { ctrl: true, shift: true, ..Default::default() });
    assert!(h.rt.inspector_open());
    key(&mut h, Key::F(12), Modifiers::default());
    h.rt.set_inspector_enabled(false);
    key(&mut h, Key::F(12), Modifiers::default());
    assert!(!h.rt.inspector_open());
}

#[test]
fn stylesheet_restyles_classes() {
    let mut h = app();
    let card = h.rt.rect_of("card").unwrap();
    let before = h.pixel(card.x + 100.0, card.y + 60.0);
    let (sheet, errors) = Stylesheet::parse(
        ".card { background: #ff0000; radius: 0; shadow: none; }\n.button-primary:hover { background: rgb(0, 255, 0); }\n.card { colour: red; }",
    );
    assert_eq!(errors.len(), 1, "{errors:?}");
    h.rt.set_stylesheet(Some(sheet));
    h.settle();
    let after = h.pixel(card.x + 100.0, card.y + 60.0);
    assert_ne!(before, after);
    assert_eq!(after[..3], [255, 0, 0]);
    assert_eq!(h.pixel(card.x + 1.0, card.y + 1.0)[..3], [255, 0, 0], "square corners now");
    // A state rule: hovering the primary button turns it green.
    let save = h.rt.rect_of("save").unwrap();
    h.move_to(save.x + 4.0, save.center().y);
    h.settle();
    assert_eq!(h.pixel(save.x + 4.0, save.center().y)[..3], [0, 255, 0]);
    // Theme colors by name, with alpha.
    let (sheet, errors) = Stylesheet::parse(".card { background: accent; }");
    assert!(errors.is_empty());
    h.rt.set_stylesheet(Some(sheet));
    h.settle();
    let accent = h.pixel(card.x + 100.0, card.y + 60.0);
    h.rt.set_stylesheet(None);
    h.settle();
    assert_ne!(h.pixel(card.x + 100.0, card.y + 60.0), accent, "removing it restores the theme");
}

#[test]
fn stylesheet_files() {
    let dir = std::env::temp_dir().join(format!("rui-styles-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("app.css");
    std::fs::write(&path, "/* app styles */\n.card {\n  padding: 4 8;\n  bogus: 1;\n}\n").unwrap();
    let (sheet, errors) = Stylesheet::load(&path).unwrap();
    assert_eq!(sheet.classes(), ["card"]);
    assert_eq!((errors.len(), errors[0].line), (1, 4));
    let _ = std::fs::remove_dir_all(dir);
}
