//! Named layouts end to end: switching from the menu bar, and saving the
//! current arrangement through the "Save Layout As" dialog.

use rust_ui::layouts::{LayoutMsg, Layouts};
use rust_ui::prelude::*;
use rust_ui::Event;

#[derive(Clone)]
struct Arr {
    dock: Dock<&'static str>,
}

struct Ide {
    now: Arr,
    layouts: Layouts<Arr>,
}

#[derive(Clone)]
enum Msg {
    Dock(DockMsg),
    Layout(LayoutMsg),
}

impl App for Ide {
    type Msg = Msg;
    fn update(&mut self, msg: Msg, _: &mut Cx<Msg>) {
        match msg {
            Msg::Dock(m) => self.now.dock.update(m),
            Msg::Layout(m) => {
                if let Some(a) = self.layouts.update(m, &self.now) {
                    self.now = a;
                }
            }
        }
    }
    fn menu(&self) -> Vec<Menu<Msg>> {
        vec![Menu::new("Window", vec![MenuItem::submenu("Layouts", self.layouts.menu_items(&Msg::Layout))])]
    }
    fn view(&self) -> Element<Msg> {
        col()
            .size_full()
            .child(menubar(self.menu()))
            .child(self.now.dock.view("d", |t| t.to_string(), |t| text(format!("content {t}")), Msg::Dock))
            .children(self.layouts.dialog(Msg::Layout))
    }
}

fn tabs(h: &Headless<Ide>) -> Vec<Vec<&'static str>> {
    h.rt.app.now.dock.groups().iter().map(|g| g.tabs.clone()).collect()
}

fn setup() -> Headless<Ide> {
    let default = Arr {
        dock: Dock::new(DockNode::hsplit(vec![
            (1.0, DockNode::tabs(vec!["a", "b"])),
            (1.0, DockNode::tabs(vec!["c"])),
        ])),
    };
    let wide = Arr { dock: Dock::new(DockNode::tabs(vec!["a", "b", "c"])) };
    let layouts = Layouts::new().with("Default", default.clone()).with("Wide", wide);
    let mut h = Headless::new(Ide { now: default, layouts }, 900.0, 600.0, 1.0);
    h.settle();
    h
}

fn click_text(h: &mut Headless<Ide>, s: &str) {
    let r = h.rt.rect_of_text(s).unwrap_or_else(|| panic!("no {s:?}"));
    h.click(r.center().x, r.center().y);
    h.settle();
}

fn layouts_menu(h: &mut Headless<Ide>) {
    click_text(h, "Window");
    let r = h.rt.rect_of_text("Layouts").unwrap().center();
    h.move_to(r.x, r.y);
    h.settle();
}

fn key(h: &mut Headless<Ide>, key: Key, ctrl: bool) {
    h.event(Event::Key(KeyEvent { key, mods: Modifiers { ctrl, meta: ctrl, ..Default::default() }, repeat: false }));
    h.settle();
}

#[test]
fn switch_from_the_menu() {
    let mut h = setup();
    layouts_menu(&mut h);
    click_text(&mut h, "Wide");
    assert_eq!(tabs(&h), vec![vec!["a", "b", "c"]]);
    assert_eq!(h.rt.app.layouts.current(), Some("Wide"));
    // Changes aren't saved unless asked: switching back and forth restores the snapshot.
    let g = h.rt.app.now.dock.groups()[0].id;
    h.rt.send(Msg::Dock(DockMsg::Close(g, 0)));
    h.settle();
    layouts_menu(&mut h);
    click_text(&mut h, "Default");
    layouts_menu(&mut h);
    click_text(&mut h, "Wide");
    assert_eq!(tabs(&h), vec![vec!["a", "b", "c"]]);
    // "Save Changes to “Wide”" keeps them.
    h.rt.send(Msg::Dock(DockMsg::Close(g, 0)));
    h.settle();
    layouts_menu(&mut h);
    click_text(&mut h, "Save Changes to “Wide”");
    layouts_menu(&mut h);
    click_text(&mut h, "Default");
    layouts_menu(&mut h);
    click_text(&mut h, "Wide");
    assert_eq!(tabs(&h), vec![vec!["b", "c"]]);
}

#[test]
fn save_as_dialog() {
    let mut h = setup();
    let g = h.rt.app.now.dock.groups()[1].id;
    h.rt.send(Msg::Dock(DockMsg::Close(g, 0)));
    h.settle();
    layouts_menu(&mut h);
    click_text(&mut h, "Save Layout As…");
    assert!(h.rt.app.layouts.dialog_open());
    // The name field has focus with a suggested name selected: type over it, press Enter.
    assert_eq!(h.rt.app.layouts.draft(), Some("Layout 3"));
    h.type_text("Mine");
    h.settle();
    key(&mut h, Key::Enter, false);
    assert!(!h.rt.app.layouts.dialog_open());
    assert_eq!(h.rt.app.layouts.names().collect::<Vec<_>>(), ["Default", "Wide", "Mine"]);
    assert_eq!(h.rt.app.layouts.current(), Some("Mine"));

    // Escape cancels; an existing name offers "Replace" (clicked).
    layouts_menu(&mut h);
    click_text(&mut h, "Save Layout As…");
    key(&mut h, Key::Escape, false);
    assert!(!h.rt.app.layouts.dialog_open());
    layouts_menu(&mut h);
    click_text(&mut h, "Save Layout As…");
    key(&mut h, Key::Char('a'), true);
    h.type_text("Wide");
    h.settle();
    assert!(h.rt.rect_of_text("Replaces the layout “Wide”.").is_some());
    click_text(&mut h, "Replace");
    assert_eq!(h.rt.app.layouts.len(), 3);
    layouts_menu(&mut h);
    click_text(&mut h, "Default");
    layouts_menu(&mut h);
    click_text(&mut h, "Wide");
    assert_eq!(tabs(&h), vec![vec!["a", "b"]]);
}
