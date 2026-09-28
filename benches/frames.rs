//! Frame-time benchmarks against the roadmap's budgets.
//!
//!     cargo bench --bench frames            # table on stdout
//!     cargo bench --bench frames -- --json out.json
//!
//! Each scenario renders frames in a headless runtime and reports the CPU
//! time per frame (median and 95th percentile). "scene" is the work the GPU
//! backend needs (view, layout, paint recording); "cpu" adds rasterizing on
//! the CPU backend (with damage tracking, so unchanged pixels are skipped).

#[allow(dead_code)]
#[path = "../examples/showcase.rs"]
mod showcase;

use std::rc::Rc;
use std::time::{Duration, Instant};

use charis_ui::prelude::*;
use charis_ui::tree::{TreeModel, TreeMsg, TreeState};
use charis_ui::Event;

struct Result {
    name: &'static str,
    median: Duration,
    p95: Duration,
    budget: Option<Duration>,
}

fn stats(name: &'static str, mut t: Vec<Duration>, budget: Option<Duration>) -> Result {
    t.sort();
    let median = t[t.len() / 2];
    let p95 = t[(t.len() * 95 / 100).min(t.len() - 1)];
    Result { name, median, p95, budget }
}

/// Time `frames` frames, each prepared by `step`, rendered by `render`.
fn run<A: App>(
    h: &mut Headless<A>,
    frames: usize,
    mut step: impl FnMut(&mut Headless<A>, usize),
    cpu: bool,
) -> Vec<Duration> {
    // Warm up caches (shaping, rasters, layout).
    for i in 0..10 {
        step(h, i);
        let _ = h.rt.render();
    }
    (0..frames)
        .map(|i| {
            step(h, i);
            let t0 = Instant::now();
            if cpu {
                let _ = h.rt.render();
            } else {
                let _ = h.rt.render_scene();
            }
            t0.elapsed()
        })
        .collect()
}

const MS: fn(f64) -> Option<Duration> = |ms| Some(Duration::from_secs_f64(ms / 1000.0));

fn showcase_benches(out: &mut Vec<Result>) {
    let mut h = Headless::new(showcase::Nebula::new(), 1280.0, 800.0, 1.0);
    h.settle();
    // Unchanged frames (an animation elsewhere, a timer tick).
    let idle = |h: &mut Headless<showcase::Nebula>, _: usize| h.rt.invalidate();
    out.push(stats("showcase: unchanged frame, scene", run(&mut h, 200, idle, false), MS(4.0)));
    out.push(stats("showcase: unchanged frame, cpu", run(&mut h, 200, idle, true), MS(4.0)));
    // The pointer moving over the file tree (hover changes each frame).
    let hover = |h: &mut Headless<showcase::Nebula>, i: usize| {
        h.move_to(120.0, 110.0 + (i % 12) as f32 * 26.0);
        h.rt.set_time(h.rt.time() + 0.016);
    };
    out.push(stats("showcase: hovering, scene", run(&mut h, 200, hover, false), MS(4.0)));
    out.push(stats("showcase: hovering, cpu", run(&mut h, 200, hover, true), MS(8.0)));
    // Everything redrawn (no damage tracking).
    h.rt.set_damage_tracking(false);
    out.push(stats("showcase: full redraw, cpu", run(&mut h, 100, idle, true), None));
}

struct BigTable {
    rows: Rc<Vec<(String, u32)>>,
}

#[derive(Clone)]
enum Nothing {}

impl App for BigTable {
    type Msg = Nothing;
    fn update(&mut self, _: Nothing, _: &mut Cx<Nothing>) {}
    fn view(&self) -> Element<Nothing> {
        let d = self.rows.clone();
        table(
            "big",
            vec![Column::new("Name").weight(3.0), Column::new("Kind").weight(1.0), Column::new("Size").fixed(90.0)],
            d.len(),
            move |r, c| match c {
                0 => cell_text(d[r].0.clone()),
                1 => cell_text(if r % 3 == 0 { "folder" } else { "file" }),
                _ => cell_text(format!("{} KB", d[r].1)),
            },
        )
        .into()
    }
}

fn table_benches(out: &mut Vec<Result>) {
    let rows: Vec<(String, u32)> = (0..100_000).map(|i| (format!("row_{i:06}.dat"), (i * 37 % 9000) as u32)).collect();
    let mut h = Headless::new(BigTable { rows: Rc::new(rows) }, 1000.0, 700.0, 1.0);
    h.settle();
    let scroll = |h: &mut Headless<BigTable>, i: usize| {
        h.event(Event::Wheel(
            Point::new(500.0, 400.0),
            Point::new(0.0, if (i / 100).is_multiple_of(2) { 90.0 } else { -90.0 }),
        ));
        h.rt.set_time(h.rt.time() + 0.008);
    };
    out.push(stats("100k-row table: scrolling, scene", run(&mut h, 300, scroll, false), MS(8.3)));
    out.push(stats("100k-row table: scrolling, cpu", run(&mut h, 300, scroll, true), MS(8.3)));
}

struct Files;

impl TreeModel for Files {
    type Id = u64;
    fn children(&self, p: Option<&u64>) -> Vec<u64> {
        match p {
            None => (1..=100).collect(),
            Some(&p) if p < 1_000_000 => (0..1000).map(|j| p * 1_000_000 + j).collect(),
            _ => Vec::new(),
        }
    }
    fn has_children(&self, id: &u64) -> bool {
        *id < 1_000_000
    }
    fn label(&self, id: &u64) -> String {
        format!("item {id}")
    }
}

struct TreeApp {
    model: Rc<Files>,
    tree: TreeState<u64>,
}

#[derive(Clone)]
enum TMsg {
    T(TreeMsg<u64>),
}

impl App for TreeApp {
    type Msg = TMsg;
    fn update(&mut self, m: TMsg, cx: &mut Cx<TMsg>) {
        let TMsg::T(m) = m;
        self.tree.update(m, &*self.model, cx);
    }
    fn view(&self) -> Element<TMsg> {
        col().size_full().child(self.tree.view(&self.model, TMsg::T))
    }
}

fn tree_benches(out: &mut Vec<Result>) {
    let mut tree = TreeState::new("t");
    for r in 1..=100 {
        tree.expand(r);
    }
    let mut h = Headless::new(TreeApp { model: Rc::new(Files), tree }, 600.0, 800.0, 1.0);
    h.settle();
    let scroll = |h: &mut Headless<TreeApp>, _: usize| {
        h.event(Event::Wheel(Point::new(300.0, 400.0), Point::new(0.0, 120.0)));
        h.rt.set_time(h.rt.time() + 0.008);
    };
    out.push(stats("100k-row tree: scrolling, scene", run(&mut h, 300, scroll, false), MS(8.3)));
}

struct Editor {
    text: String,
}

#[derive(Clone)]
enum EMsg {
    Edit(String),
}

impl App for Editor {
    type Msg = EMsg;
    fn update(&mut self, m: EMsg, _: &mut Cx<EMsg>) {
        let EMsg::Edit(t) = m;
        self.text = t;
    }
    fn view(&self) -> Element<EMsg> {
        col().size_full().child(text_area(self.text.clone(), EMsg::Edit).id("editor").mono().size_full())
    }
}

/// A 100k-line document, like a large source file.
pub fn big_document(lines: usize) -> String {
    let mut s = String::new();
    for i in 0..lines {
        match i % 4 {
            0 => s.push_str(&format!("fn function_{i}(arg: u32) -> u32 {{")),
            1 => s.push_str(&format!("    let value = arg * {i} + compute(\"line {i}\");")),
            2 => s.push_str("    value.wrapping_add(1)"),
            _ => s.push('}'),
        }
        s.push('\n');
    }
    s
}

fn editor_benches(out: &mut Vec<Result>) {
    let t0 = Instant::now();
    let mut h = Headless::new(
        Editor {
            text: big_document(std::env::var("EDITOR_LINES").ok().and_then(|v| v.parse().ok()).unwrap_or(100_000)),
        },
        1000.0,
        800.0,
        1.0,
    );
    let _ = h.rt.render_scene();
    out.push(stats("100k-line editor: open", vec![t0.elapsed()], MS(250.0)));
    let r = h.rt.rect_of("editor").expect("editor");
    h.click(r.x + 40.0, r.y + 30.0);
    h.settle();
    let scroll = |h: &mut Headless<Editor>, i: usize| {
        h.event(Event::Wheel(
            Point::new(500.0, 400.0),
            Point::new(0.0, if (i / 50).is_multiple_of(2) { 400.0 } else { -300.0 }),
        ));
        h.rt.set_time(h.rt.time() + 0.008);
    };
    out.push(stats("100k-line editor: scrolling, scene", run(&mut h, 200, scroll, false), MS(8.3)));
    let typing = |h: &mut Headless<Editor>, _: usize| {
        h.type_text("x");
        h.rt.set_time(h.rt.time() + 0.008);
    };
    out.push(stats("100k-line editor: typing, scene", run(&mut h, 100, typing, false), MS(8.3)));
    // The whole keystroke: event handling, the app's update and view, the frame.
    let keystrokes = (0..100)
        .map(|_| {
            let t0 = Instant::now();
            // Not h.type_text: that also draws a CPU frame, which a real keystroke doesn't.
            h.rt.handle(Event::Text("y".into()));
            let _ = h.rt.render_scene();
            t0.elapsed()
        })
        .collect();
    out.push(stats("100k-line editor: keystroke to frame, scene", keystrokes, MS(8.3)));
    let down = |h: &mut Headless<Editor>, _: usize| {
        h.event(Event::Key(KeyEvent { key: Key::Down, mods: Modifiers::default(), repeat: true }));
        h.rt.set_time(h.rt.time() + 0.008);
    };
    out.push(stats("100k-line editor: arrow down, scene", run(&mut h, 200, down, false), MS(8.3)));
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--list") {
        return;
    }
    // `cargo bench --bench frames -- editor` runs only the scenarios whose group matches.
    let only: Vec<&String> = args.iter().skip(1).filter(|a| !a.starts_with('-') && !a.ends_with(".json")).collect();
    let want = |g: &str| only.is_empty() || only.iter().any(|o| g.contains(o.as_str()));
    let mut out = Vec::new();
    if want("showcase") {
        showcase_benches(&mut out);
    }
    if want("table") {
        table_benches(&mut out);
    }
    if want("tree") {
        tree_benches(&mut out);
    }
    if want("editor") {
        editor_benches(&mut out);
    }
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    println!("| Scenario | Median | p95 | Budget |");
    println!("|---|---|---|---|");
    for r in &out {
        let budget = match r.budget {
            Some(b) => format!("{:.1} ms {}", ms(b), if r.p95 <= b { "✓" } else { "✗" }),
            None => "—".into(),
        };
        println!("| {} | {:.2} ms | {:.2} ms | {budget} |", r.name, ms(r.median), ms(r.p95));
    }
    if let Some(i) = args.iter().position(|a| a == "--json") {
        let path = args.get(i + 1).map(String::as_str).unwrap_or("frames.json");
        let rows: Vec<String> = out
            .iter()
            .map(|r| {
                format!(
                    "  {{\"name\": {:?}, \"median_ms\": {:.4}, \"p95_ms\": {:.4}, \"budget_ms\": {}}}",
                    r.name,
                    ms(r.median),
                    ms(r.p95),
                    r.budget.map_or("null".into(), |b| format!("{:.3}", ms(b)))
                )
            })
            .collect();
        let _ = std::fs::write(path, format!("[\n{}\n]\n", rows.join(",\n")));
    }
}
