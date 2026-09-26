//! Several windows of one app. The main window opens "inspector" windows;
//! every window shows and edits the same state.
//!
//! cargo run --example multiwindow
//! (`--smoke` runs a scripted open / update / close sequence and exits.)

use std::time::Duration;

use rust_ui::prelude::*;

struct Studio {
    count: i32,
    inspectors: Vec<u32>,
    next: u32,
    smoke: Option<u32>,
}

#[derive(Clone, Debug)]
enum Msg {
    Add(i32),
    Open,
    Close(u32),
    Smoke,
}

impl App for Studio {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, cx: &mut Cx<Msg>) {
        match msg {
            Msg::Add(d) => self.count += d,
            Msg::Open => {
                self.next += 1;
                self.inspectors.push(self.next);
            }
            Msg::Close(i) => self.inspectors.retain(|x| *x != i),
            Msg::Smoke => {
                let step = self.smoke.map_or(1, |s| s + 1);
                self.smoke = Some(step);
                match step {
                    1 | 2 => self.update(Msg::Open, cx),
                    3 => self.count += 5,
                    4 => self.inspectors.retain(|x| *x != 1),
                    _ => {
                        println!("smoke: count {} open {:?}", self.count, self.inspectors);
                        cx.close_window();
                    }
                }
                println!("smoke step {step}: windows {:?}", self.inspectors);
            }
        }
    }

    fn view(&self) -> Element<Msg> {
        let th = theme();
        col()
            .size_full()
            .p(24.0)
            .gap(16.0)
            .child(text("Main window").font_size(20.0).bold())
            .child(text(format!("Shared count: {}", self.count)).font_size(16.0))
            .child(
                row()
                    .gap(8.0)
                    .child(button("−").on_click(Msg::Add(-1)))
                    .child(button("+").on_click(Msg::Add(1)))
                    .child(primary_button("Open inspector").with_icon(Icon::Plus).on_click(Msg::Open)),
            )
            .child(
                text(format!(
                    "{} inspector window(s) open. Each has its own hover, focus and scroll state.",
                    self.inspectors.len()
                ))
                .color(th.colors.text_muted),
            )
    }

    fn windows(&self) -> Vec<WindowSpec<Msg>> {
        self.inspectors
            .iter()
            .enumerate()
            .map(|(n, &i)| {
                WindowSpec::new(format!("inspector-{i}"), format!("Inspector {i}"), Msg::Close(i))
                    .size(320.0, 220.0)
                    .at(120.0 + 40.0 * n as f32, 120.0 + 40.0 * n as f32)
            })
            .collect()
    }

    fn window_view(&self, key: &str) -> Element<Msg> {
        let th = theme();
        col()
            .size_full()
            .p(16.0)
            .gap(12.0)
            .bg(th.colors.panel)
            .child(text(key).bold())
            .child(text(format!("Shared count: {}", self.count)))
            .child(row().gap(8.0).child(button("−").on_click(Msg::Add(-1))).child(button("+").on_click(Msg::Add(1))))
    }

    fn subscriptions(&self) -> Subscriptions<Msg> {
        Subscriptions::none().every_if(self.smoke.is_some(), Duration::from_millis(300), Msg::Smoke)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let smoke = std::env::args().any(|a| a == "--smoke");
    let app = Studio { count: 0, inspectors: Vec::new(), next: 0, smoke: smoke.then_some(0) };
    rust_ui::run(app, WindowOptions::new("Multi-window").size(520.0, 300.0))
}
