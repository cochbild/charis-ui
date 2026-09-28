//! Screenshot of IME composition inside inputs (headless).
//!
//! cargo run --example ime_demo -- out.png

use charis_ui::prelude::*;
use charis_ui::Event;

#[derive(Default)]
struct Demo {
    a: String,
    b: String,
}

#[derive(Clone, Debug)]
enum Msg {
    A(String),
    B(String),
}

impl App for Demo {
    type Msg = Msg;
    fn update(&mut self, msg: Msg, _: &mut Cx<Msg>) {
        match msg {
            Msg::A(s) => self.a = s,
            Msg::B(s) => self.b = s,
        }
    }
    fn view(&self) -> Element<Msg> {
        col()
            .p(24.0)
            .gap(14.0)
            .child(
                text("Composition shows inline, underlined, before the IME commits").color(theme().colors.text_muted),
            )
            .child(text_input(self.a.clone(), Msg::A).id("a").w(360.0).placeholder("Search"))
            .child(text_area(self.b.clone(), Msg::B).id("b").w(360.0).rows(3, 3))
    }
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "ime.png".into());
    let mut h = Headless::new(Demo::default(), 420.0, 230.0, 2.0);
    h.settle();
    let r = h.rt.rect_of("b").unwrap();
    h.click(r.center().x, r.center().y);
    h.type_text("Meeting notes: ");
    let t = "とうきょう";
    h.event(Event::Preedit { text: t.into(), cursor: Some((t.len(), t.len())) });
    h.settle();
    h.save_png(&out).expect("save");
    println!("saved {out}");
}
