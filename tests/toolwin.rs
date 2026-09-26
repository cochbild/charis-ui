//! Tool windows: stripes, pinned panels that take space, auto-hide panels
//! that slide over the content and hide again.

use rust_ui::prelude::*;
use rust_ui::toolwin::{Side, ToolMode, ToolMsg, ToolWindows};

#[derive(Clone, Copy, PartialEq, Debug)]
enum Tool {
    Files,
    Search,
    Terminal,
    Outline,
}

struct Ide {
    tools: ToolWindows<Tool>,
}

#[derive(Clone, Debug)]
enum Msg {
    Tool(ToolMsg<Tool>),
}

impl App for Ide {
    type Msg = Msg;
    fn update(&mut self, m: Msg, _: &mut Cx<Msg>) {
        let Msg::Tool(m) = m;
        self.tools.update(m);
    }
    fn on_key(&self, k: &KeyEvent) -> Option<Msg> {
        self.tools.key(k).map(Msg::Tool)
    }
    fn view(&self) -> Element<Msg> {
        self.tools.view(
            div().id("center").size_full(),
            |t| format!("{t:?}"),
            |t| match t {
                Tool::Files => Icon::Files,
                Tool::Search => Icon::Search,
                Tool::Terminal => Icon::Terminal,
                Tool::Outline => Icon::Layers,
            },
            |t| text(format!("{t:?} content")),
            Msg::Tool,
        )
    }
}

fn harness() -> Headless<Ide> {
    let tools = ToolWindows::new()
        .add(Tool::Files, Side::Left, ToolMode::Pinned)
        .add(Tool::Search, Side::Left, ToolMode::AutoHide)
        .add(Tool::Terminal, Side::Bottom, ToolMode::AutoHide)
        .add(Tool::Outline, Side::Right, ToolMode::Pinned);
    let mut h = Headless::new(Ide { tools }, 1000.0, 700.0, 1.0);
    h.settle();
    h
}

fn click_id(h: &mut Headless<Ide>, id: &str) {
    let r = h.rt.rect_of(id).unwrap_or_else(|| panic!("{id} missing"));
    h.click(r.center().x, r.center().y);
    h.settle();
}

fn center(h: &Headless<Ide>) -> Rect {
    h.rt.rect_of("center").unwrap()
}

/// Is the panel on screen (inside the content area) rather than slid out?
fn visible(h: &Headless<Ide>, name: &str) -> bool {
    h.rt.rect_of(&format!("tool/{name}")).is_some_and(|r| {
        let area = Rect::new(36.0, 0.0, 1000.0 - 72.0, 700.0 - 30.0);
        area.intersect(&r).w > r.w * 0.9 && area.intersect(&r).h > r.h * 0.9
    })
}

#[test]
fn stripes_and_pinned_panels() {
    let mut h = harness();
    for t in ["Files", "Search", "Terminal", "Outline"] {
        assert!(h.rt.rect_of(&format!("tool-stripe/{t}")).is_some(), "stripe button for {t}");
    }
    let w0 = center(&h).w;
    click_id(&mut h, "tool-stripe/Files");
    assert!(visible(&h, "Files"));
    let w1 = center(&h).w;
    assert!(w0 - w1 >= 255.0, "pinned panel takes space: {w0} -> {w1}");
    click_id(&mut h, "tool-stripe/Outline");
    assert!(w1 - center(&h).w >= 255.0, "right side too");
    // The stripe button closes it again.
    click_id(&mut h, "tool-stripe/Files");
    assert!(!h.rt.app.tools.is_open(&Tool::Files));
}

#[test]
fn auto_hide_slides_over_and_hides_on_outside_click() {
    let mut h = harness();
    let w0 = center(&h).w;
    click_id(&mut h, "tool-stripe/Terminal");
    assert!(visible(&h, "Terminal"), "slid in");
    assert_eq!(center(&h).w, w0, "overlays without taking space");
    assert!(center(&h).h > 600.0);
    // Clicking the content hides it (and slides it out of view).
    h.click(500.0, 150.0);
    h.settle();
    assert!(!h.rt.app.tools.is_open(&Tool::Terminal));
    assert!(!visible(&h, "Terminal"));
    // Escape hides too.
    click_id(&mut h, "tool-stripe/Terminal");
    assert!(visible(&h, "Terminal"));
    h.event(rust_ui::Event::Key(KeyEvent { key: Key::Escape, mods: Modifiers::default(), repeat: false }));
    h.settle();
    assert!(!visible(&h, "Terminal"));
}

#[test]
fn one_per_side_and_switching_modes() {
    let mut h = harness();
    click_id(&mut h, "tool-stripe/Files");
    click_id(&mut h, "tool-stripe/Search");
    assert_eq!(h.rt.app.tools.open_on(Side::Left), Some(&Tool::Search), "replaced Files");
    let w_auto = center(&h).w;
    // Header pin button: dock it.
    click_id(&mut h, "tool-pin/Search");
    assert_eq!(h.rt.app.tools.windows[1].mode, ToolMode::Pinned);
    assert!(w_auto - center(&h).w >= 255.0, "now takes space");
    // Hide button.
    click_id(&mut h, "tool-hide/Search");
    assert!(!h.rt.app.tools.is_open(&Tool::Search));
}

#[test]
fn resizing() {
    let mut h = harness();
    // Auto-hide: drag the panel's inner edge.
    click_id(&mut h, "tool-stripe/Search");
    let r = h.rt.rect_of("tool/Search").unwrap();
    h.drag((r.right() - 2.0, r.center().y), (r.right() + 78.0, r.center().y), 8);
    h.settle();
    let size = h.rt.app.tools.windows[1].size;
    assert!((size - 340.0).abs() < 2.0, "auto-hide resized to {size}");
    assert!((h.rt.rect_of("tool/Search").unwrap().w - 340.0).abs() < 2.0);
    // Pinned: drag the splitter; the size is written back.
    h.rt.app.tools.update(ToolMsg::Hide(Tool::Search));
    h.rt.invalidate();
    h.settle();
    click_id(&mut h, "tool-stripe/Files");
    let r = h.rt.rect_of("tool/Files").unwrap();
    h.drag((r.right() + 1.0, r.center().y), (r.right() + 61.0, r.center().y), 8);
    h.settle();
    let size = h.rt.app.tools.windows[0].size;
    assert!((size - 320.0).abs() < 3.0, "pinned resized to {size}");
}

fn right_click(h: &mut Headless<Ide>, p: Point) {
    h.event(rust_ui::Event::PointerDown(p, rust_ui::MouseButton::Right));
    h.event(rust_ui::Event::PointerUp(p, rust_ui::MouseButton::Right));
    h.settle();
}

fn click_text(h: &mut Headless<Ide>, s: &str) {
    let r = h.rt.rect_of_text(s).unwrap_or_else(|| panic!("no {s:?}"));
    h.click(r.center().x, r.center().y);
    h.settle();
}

#[test]
fn commands_menu() {
    let mut h = harness();
    // Right-click a stripe button: move the tool window to the right edge.
    let b = h.rt.rect_of("tool-stripe/Search").unwrap().center();
    right_click(&mut h, b);
    assert!(h.rt.rect_of_text("View Mode").is_some());
    let sub = h.rt.rect_of_text("Move to").unwrap().center();
    h.move_to(sub.x, sub.y);
    h.settle();
    click_text(&mut h, "Right");
    assert_eq!(h.rt.app.tools.windows[1].side, Side::Right);
    assert!(h.rt.rect_of_text("View Mode").is_none(), "menu closed");
    let b = h.rt.rect_of("tool-stripe/Search").unwrap();
    assert!(b.x > 900.0, "button moved to the right stripe: {b:?}");

    // The header's Options button: switch the view mode.
    click_id(&mut h, "tool-stripe/Terminal");
    click_id(&mut h, "tool-options/Terminal");
    click_text(&mut h, "Pinned");
    assert_eq!(h.rt.app.tools.windows[2].mode, ToolMode::Pinned);
    assert!(h.rt.app.tools.is_open(&Tool::Terminal), "still open, now docked");
    assert!(center(&h).h < 500.0, "takes space now");

    // Escape closes the menu (focus is in it).
    click_id(&mut h, "tool-options/Terminal");
    assert!(h.rt.rect_of_text("View Mode").is_some());
    h.event(rust_ui::Event::Key(KeyEvent { key: Key::Escape, mods: Modifiers::default(), repeat: false }));
    h.settle();
    assert!(h.rt.rect_of_text("View Mode").is_none());
}
