//! pick_list / combo_box.

use rust_ui::prelude::*;
use rust_ui::Event;

#[derive(Default)]
struct D {
    model: Option<usize>,
    mode: Option<usize>,
}

#[derive(Clone, Debug)]
enum Msg {
    Model(usize),
    Mode(usize),
}

const MODELS: [&str; 5] = ["llama-3.1-8b", "qwen2.5-7b", "qwen2.5-coder-32b", "mistral-small", "phi-4"];

impl App for D {
    type Msg = Msg;
    fn update(&mut self, msg: Msg, _: &mut Cx<Msg>) {
        match msg {
            Msg::Model(i) => self.model = Some(i),
            Msg::Mode(i) => self.mode = Some(i),
        }
    }
    fn view(&self) -> Element<Msg> {
        col()
            .p(20.0)
            .gap(12.0)
            .child(
                // A clipping container: the popup must still escape it.
                div().h(40.0).clip().child(combo_box(MODELS, self.model, Msg::Model).id("model").w(240.0)),
            )
            .child(pick_list(["auto", "on", "off"], self.mode, Msg::Mode).id("mode").w(160.0))
    }
}

fn key(h: &mut Headless<D>, k: Key) {
    h.event(Event::Key(KeyEvent { key: k, mods: Modifiers::default(), repeat: false }));
}

fn setup() -> Headless<D> {
    let mut h = Headless::new(D::default(), 500.0, 500.0, 1.0);
    h.settle();
    h
}

#[test]
fn click_to_pick() {
    let mut h = setup();
    let r = h.rt.rect_of("mode").unwrap();
    assert!(h.rt.rect_of_text("off").is_none(), "closed initially");
    h.click(r.center().x, r.center().y);
    let off = h.rt.rect_of_text("off").expect("options shown");
    h.click(off.center().x, off.center().y);
    assert_eq!(h.rt.app.mode, Some(2));
    assert!(h.rt.rect_of_text("auto").is_none(), "closed after picking");
    assert!(h.rt.rect_of_text("off").is_some(), "trigger shows the selection");
}

#[test]
fn keyboard_navigation() {
    let mut h = setup();
    let r = h.rt.rect_of("mode").unwrap();
    h.click(r.center().x, r.center().y); // opens and focuses
    key(&mut h, Key::Escape);
    assert!(h.rt.rect_of_text("auto").is_none(), "escape closes");
    key(&mut h, Key::Down); // reopens (focused)
    key(&mut h, Key::Down);
    key(&mut h, Key::Enter);
    assert_eq!(h.rt.app.mode, Some(1));
    // Type-ahead.
    key(&mut h, Key::Enter);
    h.type_text("o");
    key(&mut h, Key::Enter);
    assert_eq!(h.rt.app.mode, Some(1), "first option starting with 'o' is 'on'");
}

#[test]
fn combo_filters_and_escapes_clipping() {
    let mut h = setup();
    let r = h.rt.rect_of("model").unwrap();
    h.click(r.center().x, r.center().y);
    // The popup extends below the 40px clipping container and is clickable.
    let last = h.rt.rect_of_text("phi-4").expect("all options listed");
    assert!(last.y > 60.0 + 40.0, "popup outside the clip: {last:?}");
    h.type_text("coder");
    assert!(h.rt.rect_of_text("phi-4").is_none(), "filtered out");
    assert!(h.rt.rect_of_text("qwen2.5-coder-32b").is_some());
    key(&mut h, Key::Enter);
    assert_eq!(h.rt.app.model, Some(2), "maps back to the original index");
    // Click outside closes without picking.
    h.click(r.center().x, r.center().y);
    h.click(450.0, 450.0);
    assert!(h.rt.rect_of_text("phi-4").is_none());
    assert_eq!(h.rt.app.model, Some(2));
}
