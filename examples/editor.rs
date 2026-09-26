//! A plain text editor for large files: opens a file, or generates one.
//!
//! ```sh
//! cargo run --release --example editor -- path/to/file.txt
//! cargo run --release --example editor -- --lines 100000
//! ```

use rust_ui::prelude::*;

struct Editor {
    title: String,
    text: String,
    edits: u32,
}

#[derive(Clone)]
enum Msg {
    Edit(String),
}

impl App for Editor {
    type Msg = Msg;
    fn update(&mut self, m: Msg, _: &mut Cx<Msg>) {
        let Msg::Edit(t) = m;
        self.text = t;
        self.edits += 1;
    }
    fn view(&self) -> Element<Msg> {
        let th = theme();
        let lines = self.text.as_bytes().iter().filter(|&&b| b == b'\n').count() + 1;
        col()
            .size_full()
            .child(text_area(self.text.clone(), Msg::Edit).id("editor").mono().size_full().rounded(0.0).grow(1.0))
            .child(
                status_bar()
                    .child(status_item(Some(Icon::File), self.title.clone()))
                    .child(spacer())
                    .child(status_item(None, format!("{lines} lines · {} KB", self.text.len() / 1024)))
                    .child(status_item(None, format!("{} edits", self.edits)))
                    .color(th.colors.text_muted),
            )
    }
}

fn generated(lines: usize) -> String {
    let mut s = String::with_capacity(lines * 40);
    for i in 0..lines {
        match i % 4 {
            0 => s.push_str(&format!("fn function_{i}(arg: u32) -> u32 {{\n")),
            1 => s.push_str(&format!("    let value = arg * {i} + compute(\"line {i}\");\n")),
            2 => s.push_str("    value.wrapping_add(1)\n"),
            _ => s.push_str("}\n"),
        }
    }
    s
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (title, text) = match args.iter().position(|a| a == "--lines") {
        Some(i) => {
            let n = args.get(i + 1).and_then(|n| n.parse().ok()).unwrap_or(100_000);
            (format!("generated ({n} lines)"), generated(n))
        }
        None => match args.first() {
            Some(path) => match std::fs::read_to_string(path) {
                Ok(t) => (path.clone(), t),
                Err(e) => {
                    eprintln!("can't read {path}: {e}");
                    std::process::exit(1);
                }
            },
            None => ("generated (100000 lines)".into(), generated(100_000)),
        },
    };
    let app = Editor { title: title.clone(), text, edits: 0 };
    rust_ui::run(app, WindowOptions::new(format!("Editor — {title}")).size(1000.0, 760.0)).expect("run");
}
