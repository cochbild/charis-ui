//! Platform polish that can be checked headlessly: DPI changes re-render
//! crisply, fractional scales snap lines to device pixels, window state
//! (native buttons, full screen, backdrop) shapes the title bar and the
//! background, frameless edge resizing, macOS editing keys, the system
//! UI font.

use rust_ui::prelude::*;
use rust_ui::{Event, MouseButton, WindowInfo, WindowRequest};

struct Chrome {
    text: String,
}

#[derive(Clone)]
enum Msg {
    Text(String),
}

impl App for Chrome {
    type Msg = Msg;
    fn update(&mut self, msg: Msg, _: &mut Cx<Msg>) {
        let Msg::Text(t) = msg;
        self.text = t;
    }
    fn view(&self) -> Element<Msg> {
        let th = theme();
        col()
            .size_full()
            .child(titlebar("Title", text("left"), text("right"), window_info().maximized))
            .child(
                div()
                    .id("box")
                    .ml(10.3)
                    .mt(7.7)
                    .w(40.4)
                    .h(20.0)
                    .border(1.0, th.colors.accent)
                    .child(text("Crisp").font_size(12.0)),
            )
            .child(text_area(self.text.clone(), Msg::Text).id("field").w(300.0))
    }
}

fn app() -> Chrome {
    Chrome { text: String::new() }
}

fn pixels(h: &mut Headless<Chrome>) -> Vec<u8> {
    h.rt.render().data().to_vec()
}

#[test]
fn dpi_changes_rerender_like_a_fresh_window() {
    for (from, to) in [(1.0, 2.0), (2.0, 1.0), (1.0, 1.5), (1.25, 1.75)] {
        let mut h = Headless::new(app(), 400.0, 300.0, from);
        h.settle();
        let _ = pixels(&mut h);
        // Moved to a monitor with another scale: same logical size.
        h.rt.resize(Size::new(400.0, 300.0), to);
        h.settle();
        let moved = pixels(&mut h);
        let mut fresh = Headless::new(app(), 400.0, 300.0, to);
        fresh.settle();
        assert!(moved == pixels(&mut fresh), "{from} -> {to}: differs from a window opened at {to}");
    }
}

#[test]
fn fractional_scales_keep_lines_crisp() {
    for scale in [1.25f32, 1.5, 1.75] {
        let mut h = Headless::new(app(), 400.0, 300.0, scale);
        h.settle();
        let r = h.rt.rect_of("box").unwrap();
        // Down a column through the top border: exactly two colors (the
        // background and the border), never a blend of both, and the 1 px
        // line is a whole number of device pixels.
        let pm = h.rt.render().clone();
        // Near the box's right side, clear of its text.
        let x = ((r.right() - 4.0) * scale) as u32;
        let (y0, y1) = (((r.y - 3.0) * scale) as u32, ((r.y + 4.0) * scale) as u32);
        let col: Vec<[u8; 4]> = (y0..y1)
            .map(|y| {
                let p = pm.pixel(x, y).unwrap();
                [p.red(), p.green(), p.blue(), p.alpha()]
            })
            .collect();
        let bg = col[0];
        let mut colors: Vec<[u8; 4]> = col.clone();
        colors.dedup();
        assert_eq!(colors.len(), 3, "scale {scale}: background, line, background: {col:?}");
        assert_eq!(colors[2], bg);
        let line_px = col.iter().filter(|c| **c == colors[1]).count();
        assert_eq!(line_px, scale.round() as usize, "scale {scale}: {col:?}");
    }
}

#[test]
fn native_buttons_and_full_screen_shape_the_title_bar() {
    let mut h = Headless::new(app(), 600.0, 300.0, 1.0);
    h.settle();
    assert!(h.rt.rect_of("window-control/close").is_some());
    assert!(h.rt.rect_of("titlebar-native-buttons").is_none());
    // A macOS frameless window: traffic lights drawn by the OS.
    h.rt.set_window_state(WindowInfo { native_buttons: true, buttons_inset: 76.0, ..Default::default() });
    h.settle();
    assert!(h.rt.rect_of("window-control/close").is_none(), "no buttons of our own");
    let gap = h.rt.rect_of("titlebar-native-buttons").unwrap();
    assert_eq!((gap.x, gap.w), (0.0, 76.0));
    assert!(h.rt.rect_of_text("left").unwrap().x >= 76.0);
    // Full screen: macOS hides the traffic lights, the room goes away.
    h.rt.set_window_state(WindowInfo {
        native_buttons: true,
        buttons_inset: 76.0,
        fullscreen: true,
        ..Default::default()
    });
    h.settle();
    assert!(h.rt.rect_of("titlebar-native-buttons").is_none());
    assert!(h.rt.window_info().fullscreen);
}

#[test]
fn backdrop_makes_the_window_background_transparent() {
    let mut h = Headless::new(app(), 400.0, 300.0, 1.0);
    h.settle();
    assert_eq!(h.pixel(380.0, 280.0)[3], 255);
    h.rt.set_window_state(WindowInfo { backdrop: true, ..Default::default() });
    h.settle();
    assert_eq!(h.pixel(380.0, 280.0)[3], 0, "the backdrop shows through");
    assert!(h.rt.window_info().backdrop);
    // The title bar shows the material too; surfaces with their own color
    // (a text field) stay opaque.
    assert_eq!(h.pixel(200.0, 5.0)[3], 0);
    let f = h.rt.rect_of("field").unwrap();
    assert_eq!(h.pixel(f.x + 20.0, f.y + 10.0)[3], 255);
}

#[test]
fn frameless_edges_resize_except_in_full_screen() {
    let mut h = Headless::new(app(), 400.0, 300.0, 1.0);
    h.rt.frameless = true;
    h.settle();
    h.event(Event::PointerDown(Point::new(399.0, 299.0), MouseButton::Left));
    h.event(Event::PointerUp(Point::new(399.0, 299.0), MouseButton::Left));
    let reqs = h.rt.take_requests();
    assert!(reqs.iter().any(|r| matches!(r, WindowRequest::DragResize(rust_ui::runtime::ResizeEdge::SE))), "{reqs:?}");
    h.rt.set_window_state(WindowInfo { fullscreen: true, ..Default::default() });
    h.settle();
    h.event(Event::PointerDown(Point::new(399.0, 299.0), MouseButton::Left));
    h.event(Event::PointerUp(Point::new(399.0, 299.0), MouseButton::Left));
    assert!(!h.rt.take_requests().iter().any(|r| matches!(r, WindowRequest::DragResize(_))));
}

fn key(h: &mut Headless<Chrome>, key: Key, mods: Modifiers) {
    h.event(Event::Key(KeyEvent { key, mods, repeat: false }));
    h.settle();
}

#[test]
fn macos_editing_keys() {
    let mut h = Headless::new(app(), 400.0, 300.0, 1.0);
    h.rt.set_mac_keys(true);
    h.settle();
    let f = h.rt.rect_of("field").unwrap().center();
    h.click(f.x, f.y);
    h.type_text("first line");
    key(&mut h, Key::Enter, Modifiers::default());
    h.type_text("second word");
    h.settle();
    let cmd = Modifiers { meta: true, ..Default::default() };
    let opt = Modifiers { alt: true, ..Default::default() };
    // Option+⌫ deletes a word; Cmd+← goes to the line start.
    key(&mut h, Key::Backspace, opt);
    assert_eq!(h.rt.app.text, "first line\nsecond ");
    key(&mut h, Key::Left, cmd);
    h.type_text("> ");
    h.settle();
    assert_eq!(h.rt.app.text, "first line\n> second ");
    // Cmd+⌫ deletes to the line start; Cmd+↑ goes to the start of the text.
    key(&mut h, Key::Backspace, cmd);
    assert_eq!(h.rt.app.text, "first line\nsecond ");
    key(&mut h, Key::Up, cmd);
    h.type_text("^");
    h.settle();
    assert_eq!(h.rt.app.text, "^first line\nsecond ");
    // Ctrl isn't the command key on macOS: Ctrl+A doesn't select all.
    key(&mut h, Key::Char('a'), Modifiers { ctrl: true, ..Default::default() });
    h.type_text("!");
    h.settle();
    assert!(h.rt.app.text.len() > 3);
    key(&mut h, Key::Char('a'), cmd);
    h.type_text("all");
    h.settle();
    assert_eq!(h.rt.app.text, "all");
}

#[test]
fn system_ui_font() {
    let mut h = Headless::new(app(), 400.0, 300.0, 1.0);
    h.settle();
    let st = rust_ui::text::TextStyle::default();
    let inter = h.rt.text.measure("Crisp system font", &st, None, 1.0).w;
    match h.rt.use_system_font() {
        // This machine has a platform font (on Linux: the desktop's, else a
        // common sans like DejaVu Sans): text now uses it.
        Some(family) => {
            h.settle();
            let w = h.rt.text.measure("Crisp system font", &st, None, 1.0).w;
            assert!((w - inter).abs() > 0.5, "{family} ({w}) measures differently from Inter ({inter})");
        }
        None => eprintln!("no platform UI font installed; staying on Inter"),
    }
}
