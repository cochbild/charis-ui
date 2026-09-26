//! Several windows of one app: declared from state, shared updates,
//! per-window UI state, and app-level subscriptions running once.

use std::time::Duration;

use rust_ui::headless::HeadlessApp;
use rust_ui::prelude::*;

#[derive(Default)]
struct Studio {
    count: u32,
    ticks: u32,
    note: String,
    inspectors: Vec<u32>,
}

#[derive(Clone, Debug)]
enum Msg {
    Inc,
    Tick,
    Note(String),
    OpenInspector(u32),
    CloseInspector(u32),
}

impl App for Studio {
    type Msg = Msg;
    fn update(&mut self, m: Msg, _: &mut Cx<Msg>) {
        match m {
            Msg::Inc => self.count += 1,
            Msg::Tick => self.ticks += 1,
            Msg::Note(s) => self.note = s,
            Msg::OpenInspector(i) => {
                if !self.inspectors.contains(&i) {
                    self.inspectors.push(i)
                }
            }
            Msg::CloseInspector(i) => self.inspectors.retain(|x| *x != i),
        }
    }
    fn view(&self) -> Element<Msg> {
        col()
            .items(Align::Start)
            .child(text(format!("count {}", self.count)))
            .child(button("Open 1").id("open1").on_click(Msg::OpenInspector(1)))
            .child(button("Open 2").id("open2").on_click(Msg::OpenInspector(2)))
            .child(text_input(self.note.clone(), Msg::Note).id("note").w(200.0))
    }
    fn subscriptions(&self) -> Subscriptions<Msg> {
        Subscriptions::none().every(Duration::from_millis(100), Msg::Tick)
    }
    fn windows(&self) -> Vec<WindowSpec<Msg>> {
        self.inspectors
            .iter()
            .map(|&i| {
                WindowSpec::new(format!("inspector-{i}"), format!("Inspector {i}"), Msg::CloseInspector(i))
                    .size(300.0, 200.0)
            })
            .collect()
    }
    fn window_view(&self, key: &str) -> Element<Msg> {
        col()
            .items(Align::Start)
            .child(text(format!("{key}: count {}", self.count)))
            .child(button("Increment").id("inc").on_click(Msg::Inc))
            .child(text_input(self.note.clone(), Msg::Note).id("note").w(200.0))
    }
}

fn click(h: &mut HeadlessApp<Studio>, window: Option<&str>, id: &str) {
    let hw = match window {
        None => h.main(),
        Some(k) => h.window(k).unwrap(),
    };
    let r = hw.rt.rect_of(id).unwrap_or_else(|| panic!("{id} not in {window:?}"));
    h.click(window, r.center().x, r.center().y);
}

#[test]
fn windows_follow_app_state() {
    let mut h = HeadlessApp::new(Studio::default(), 400.0, 300.0);
    assert!(h.window_keys().is_empty());
    click(&mut h, None, "open1");
    click(&mut h, None, "open2");
    assert_eq!(h.window_keys(), ["inspector-1", "inspector-2"]);
    assert!(h.window("inspector-2").unwrap().rt.rect_of_text("inspector-2: count 0").is_some());
    // Closing sends on_close; the app drops it from windows().
    h.close("inspector-1");
    assert_eq!(h.window_keys(), ["inspector-2"]);
    assert_eq!(h.app().inspectors, [2]);
}

#[test]
fn updates_from_any_window_reach_every_window() {
    let mut h = HeadlessApp::new(Studio::default(), 400.0, 300.0);
    click(&mut h, None, "open1");
    click(&mut h, None, "open2");
    click(&mut h, Some("inspector-1"), "inc");
    click(&mut h, Some("inspector-1"), "inc");
    assert_eq!(h.app().count, 2);
    assert!(h.main().rt.rect_of_text("count 2").is_some());
    assert!(h.window("inspector-2").unwrap().rt.rect_of_text("inspector-2: count 2").is_some());
}

#[test]
fn each_window_has_its_own_focus() {
    let mut h = HeadlessApp::new(Studio::default(), 400.0, 300.0);
    click(&mut h, None, "open1");
    click(&mut h, Some("inspector-1"), "note");
    h.type_text(Some("inspector-1"), "from inspector");
    assert_eq!(h.app().note, "from inspector");
    // The main window's input isn't focused: typing there does nothing.
    h.type_text(None, "!");
    assert_eq!(h.app().note, "from inspector");
    click(&mut h, None, "note");
    h.type_text(None, "?");
    assert_eq!(h.app().note, "from inspector?");
}

#[test]
fn app_subscriptions_run_once_not_per_window() {
    let mut h = HeadlessApp::new(Studio::default(), 400.0, 300.0);
    click(&mut h, None, "open1");
    click(&mut h, None, "open2");
    let t0 = h.app().ticks;
    for _ in 0..10 {
        h.advance(0.1);
    }
    let n = h.app().ticks - t0;
    assert!((9..=11).contains(&n), "one timer, not three: {n} ticks");
}
