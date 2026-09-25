//! The smallest rust-ui app.
//!
//! Run: cargo run --example counter

use rust_ui::prelude::*;

struct Counter {
    n: i32,
}

#[derive(Clone)]
enum Msg {
    Inc,
    Dec,
    Reset,
}

impl App for Counter {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _cx: &mut Cx) {
        match msg {
            Msg::Inc => self.n += 1,
            Msg::Dec => self.n -= 1,
            Msg::Reset => self.n = 0,
        }
    }

    fn view(&self) -> Element<Msg> {
        let th = theme();
        col().size_full().center().gap(20.0).child(
            card()
                .w(320.0)
                .items_center()
                .gap(16.0)
                .p(28.0)
                .child(text("Counter").color(th.colors.text_muted).medium())
                .child(text(self.n.to_string()).font_size(56.0).bold())
                .child(
                    row()
                        .gap(8.0)
                        .child(button("−").w(44.0).on_click(Msg::Dec))
                        .child(ghost_button("Reset").on_click(Msg::Reset))
                        .child(primary_button("+").w(44.0).on_click(Msg::Inc)),
                ),
        )
    }
}

fn main() {
    rust_ui::run(Counter { n: 0 }, WindowOptions::new("Counter").size(480.0, 360.0)).unwrap();
}
