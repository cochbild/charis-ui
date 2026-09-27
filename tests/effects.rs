//! Async tasks, streams, proxies, timers, window subscriptions and scroll control.

use std::time::Duration;

use charis_ui::prelude::*;
use charis_ui::TaskHandle;

#[derive(Default)]
struct Fx {
    log: Vec<String>,
    tokens: String,
    lines: usize,
    ticking: bool,
    ticks: u32,
    handle: Option<TaskHandle>,
    close_intercept: bool,
    last_scroll: Option<ScrollInfo>,
    sizes: Vec<Size>,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
enum Msg {
    Fetch,
    Fetched(u32),
    Stream(usize),
    Token(String),
    Abort,
    Blocking,
    BlockingDone(u64),
    FromThread(&'static str),
    Tick,
    CloseRequested,
    AddLines(usize),
    Scrolled(ScrollInfo),
    ScrollEnd,
    ScrollTop,
    Resized(Size),
}

impl App for Fx {
    type Msg = Msg;
    fn update(&mut self, msg: Msg, cx: &mut Cx<Msg>) {
        self.log.push(format!("{msg:?}"));
        match msg {
            Msg::Fetch => {
                cx.spawn(async { 40 + 2 }, Msg::Fetched);
            }
            Msg::Fetched(_) => {}
            Msg::Stream(n) => {
                let s = futures::stream::iter((0..n).map(|i| format!("{i},")));
                self.handle = Some(cx.run(s, Msg::Token));
            }
            Msg::Token(t) => self.tokens.push_str(&t),
            Msg::Abort => {
                if let Some(h) = &self.handle {
                    h.abort();
                }
            }
            Msg::Blocking => {
                cx.spawn_blocking(|| Msg::BlockingDone((1..=10u64).product()));
            }
            Msg::BlockingDone(_) => {}
            Msg::FromThread(_) => {}
            Msg::Tick => self.ticks += 1,
            Msg::CloseRequested => {}
            Msg::AddLines(n) => self.lines += n,
            Msg::Scrolled(s) => self.last_scroll = Some(s),
            Msg::ScrollEnd => cx.scroll_to_end("log"),
            Msg::ScrollTop => cx.scroll_to("log", 0.0),
            Msg::Resized(s) => self.sizes.push(s),
        }
    }
    fn subscriptions(&self) -> Subscriptions<Msg> {
        let s = Subscriptions::none().every_if(self.ticking, Duration::from_secs(1), Msg::Tick).on_resize(Msg::Resized);
        if self.close_intercept {
            s.on_close_request(Msg::CloseRequested)
        } else {
            s
        }
    }
    fn view(&self) -> Element<Msg> {
        col().size_full().child(
            col()
                .id("log")
                .h(200.0)
                .scroll_y()
                .follow_end()
                .on_scroll(Msg::Scrolled)
                .children((0..self.lines).map(|i| text(format!("line {i}")).h(20.0))),
        )
    }
}

fn h() -> Headless<Fx> {
    let mut h = Headless::new(Fx::default(), 400.0, 300.0, 1.0);
    h.settle();
    h
}

const T: Duration = Duration::from_secs(5);

#[test]
fn spawn_delivers_result() {
    let mut h = h();
    h.rt.send(Msg::Fetch);
    assert!(h.wait_until(T, |a| a.log.iter().any(|l| l == "Fetched(42)")));
}

#[test]
fn stream_delivers_in_order_and_aborts() {
    let mut h = h();
    h.rt.send(Msg::Stream(200));
    assert!(h.wait_until(T, |a| a.tokens.matches(',').count() == 200));
    let expected: String = (0..200).map(|i| format!("{i},")).collect();
    assert_eq!(h.rt.app.tokens, expected);

    // An endless stream stops delivering after abort.
    let mut h2 = self::h();
    h2.rt.send(Msg::Stream(usize::MAX));
    assert!(h2.wait_until(T, |a| a.tokens.len() > 100));
    h2.rt.send(Msg::Abort);
    std::thread::sleep(Duration::from_millis(50));
    h2.rt.poll();
    let n = h2.rt.app.tokens.len();
    std::thread::sleep(Duration::from_millis(100));
    h2.rt.poll();
    assert_eq!(h2.rt.app.tokens.len(), n, "no tokens after abort");
}

#[test]
fn blocking_and_proxy() {
    let mut h = h();
    h.rt.send(Msg::Blocking);
    assert!(h.wait_until(T, |a| a.log.iter().any(|l| l == "BlockingDone(3628800)")));
    let proxy = h.rt.proxy();
    std::thread::spawn(move || {
        proxy.send(Msg::FromThread("hi"));
    });
    assert!(h.wait_until(T, |a| a.log.iter().any(|l| l.contains("FromThread"))));
}

#[test]
fn conditional_timer() {
    let mut h = h();
    for _ in 0..30 {
        h.advance(0.1);
    }
    assert_eq!(h.rt.app.ticks, 0, "no timer while not subscribed");
    h.rt.app.ticking = true;
    h.rt.send(Msg::AddLines(0)); // any update re-evaluates subscriptions
    for _ in 0..35 {
        h.advance(0.1);
    }
    assert_eq!(h.rt.app.ticks, 3, "ticks after 3.5s");
    h.rt.app.ticking = false;
    h.rt.send(Msg::AddLines(0));
    for _ in 0..30 {
        h.advance(0.1);
    }
    assert_eq!(h.rt.app.ticks, 3, "timer stops when unsubscribed");
}

#[test]
fn close_request_interception() {
    let mut h = h();
    assert!(h.rt.request_close(), "closes immediately without a subscription");
    h.rt.app.close_intercept = true;
    h.rt.send(Msg::AddLines(0));
    assert!(!h.rt.request_close(), "intercepted");
    assert!(h.rt.app.log.iter().any(|l| l == "CloseRequested"));
}

#[test]
fn resize_subscription() {
    let mut h = h();
    h.rt.resize(Size::new(500.0, 300.0), 1.0);
    assert_eq!(h.rt.app.sizes, vec![Size::new(500.0, 300.0)]);
}

#[test]
fn follow_end_and_scroll_commands() {
    let mut h = h();
    h.rt.send(Msg::AddLines(50));
    h.settle();
    let s = h.rt.app.last_scroll.expect("scroll reported");
    assert!(s.at_end && s.max.y > 0.0, "pinned to end: {s:?}");
    // Content grows: stays pinned.
    h.rt.send(Msg::AddLines(20));
    h.settle();
    let s = h.rt.app.last_scroll.unwrap();
    assert!(s.at_end, "still at end after growth: {s:?}");
    // User scrolls up: unpinned, growth doesn't move it.
    let r = h.rt.rect_of("log").unwrap();
    h.event(charis_ui::Event::Wheel(r.center(), Point::new(0.0, -300.0)));
    h.settle();
    let up = h.rt.app.last_scroll.unwrap();
    assert!(!up.at_end);
    h.rt.send(Msg::AddLines(10));
    h.settle();
    assert_eq!(h.rt.app.last_scroll.unwrap().offset, up.offset, "unpinned view stays put");
    // Programmatic scroll to end re-pins.
    h.rt.send(Msg::ScrollEnd);
    h.settle();
    assert!(h.rt.app.last_scroll.unwrap().at_end);
    h.rt.send(Msg::AddLines(10));
    h.settle();
    assert!(h.rt.app.last_scroll.unwrap().at_end);
    h.rt.send(Msg::ScrollTop);
    h.settle();
    assert_eq!(h.rt.app.last_scroll.unwrap().offset.y, 0.0);
}
