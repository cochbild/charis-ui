//! Frame-cost scaling benchmark: a non-virtualized UI with N rows of mixed
//! widgets (the worst case), measured per phase.
//!
//! cargo run --release --example stress [-- rows...]

use rust_ui::prelude::*;

struct Stress {
    rows: usize,
    tick: u32,
}

#[derive(Clone, Debug)]
enum Msg {
    Noop,
}

impl App for Stress {
    type Msg = Msg;
    fn update(&mut self, _: Msg, _: &mut Cx<Msg>) {}
    fn view(&self) -> Element<Msg> {
        let c = theme().colors.clone();
        let list = col().gap(2.0).children((0..self.rows).map(|i| {
            row()
                .items_center()
                .gap(8.0)
                .h(28.0)
                .px(8.0)
                .rounded(4.0)
                .hover(|s| s.bg(c.hover))
                .child(icon(Icon::File).color(c.text_faint))
                .child(text(format!("Item {i} · tick {}", self.tick)).grow(1.0))
                .child(badge(format!("{}", i % 97)))
                .child(button("Open").on_click(Msg::Noop))
        }));
        col().size_full().child(list.scroll_y().grow(1.0))
    }
}

fn main() {
    if std::env::var_os("SIZES").is_some() {
        for (n, s) in rust_ui::Runtime::<Stress>::struct_sizes() {
            println!("{n}: {s} bytes");
        }
    }
    let args: Vec<usize> = std::env::args().skip(1).filter_map(|a| a.parse().ok()).collect();
    let sizes = if args.is_empty() { vec![100, 1_000, 5_000, 20_000] } else { args };
    println!("{:>7} {:>7} {:>10} {:>10}", "rows", "nodes", "frame ms", "per node µs");
    for rows in sizes {
        let mut h = Headless::new(Stress { rows, tick: 0 }, 1280.0, 800.0, 1.0);
        h.settle();
        let frames = 10;
        let t = std::time::Instant::now();
        for i in 0..frames {
            if std::env::var_os("STATIC").is_none() {
                h.rt.app.tick = i;
            }
            h.rt.invalidate();
            h.rt.render_scene();
        }
        let ms = t.elapsed().as_secs_f64() * 1000.0 / frames as f64;
        let nodes = h.rt.node_count();
        println!("{rows:>7} {nodes:>7} {ms:>10.2} {:>10.2}", ms * 1000.0 / nodes as f64);
    }
}
