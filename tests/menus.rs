//! App menus: shortcuts from the declaration (with the right precedence),
//! and the self-managing in-window menu bar with submenus.

use charis_ui::headless::HeadlessApp;
use charis_ui::prelude::*;

#[derive(Default)]
struct Ed {
    log: Vec<String>,
    text: String,
    wrap: bool,
    exporting: bool,
    secondary: bool,
}

#[derive(Clone, Debug)]
enum Msg {
    Do(&'static str),
    Wrap(bool),
    Input(String),
    Close,
}

impl App for Ed {
    type Msg = Msg;
    fn update(&mut self, m: Msg, _: &mut Cx<Msg>) {
        match m {
            Msg::Do(s) => self.log.push(s.into()),
            Msg::Wrap(w) => self.wrap = w,
            Msg::Input(t) => self.text = t,
            Msg::Close => self.secondary = false,
        }
    }
    fn menu(&self) -> Vec<Menu<Msg>> {
        vec![
            Menu::new(
                "File",
                vec![
                    MenuItem::action("Save", Msg::Do("save")).shortcut("Mod+S"),
                    MenuItem::submenu(
                        "Export",
                        vec![
                            MenuItem::action("HTML", Msg::Do("html")).shortcut("Mod+Shift+E"),
                            MenuItem::action("PDF", Msg::Do("pdf")).disabled(!self.exporting).shortcut("Mod+Shift+P"),
                        ],
                    ),
                    MenuItem::Separator,
                    MenuItem::action("Quit", Msg::Do("quit")).shortcut("Mod+Q"),
                ],
            ),
            Menu::new(
                "Edit",
                vec![
                    MenuItem::action("Copy", Msg::Do("copy")).shortcut("Mod+C"),
                    MenuItem::check("Word wrap", self.wrap, Msg::Wrap(!self.wrap)).shortcut("Alt+Z"),
                ],
            ),
        ]
    }
    fn view(&self) -> Element<Msg> {
        col()
            .size_full()
            .child(row().h(30.0).child(menubar(self.menu())))
            .child(text_input(self.text.clone(), Msg::Input).id("input").w(200.0))
            .child(text("Body text").id("body"))
    }
    fn windows(&self) -> Vec<WindowSpec<Msg>> {
        if self.secondary {
            vec![WindowSpec::new("aux", "Aux", Msg::Close).size(300.0, 200.0)]
        } else {
            vec![]
        }
    }
    fn window_view(&self, _: &str) -> Element<Msg> {
        text("aux window")
    }
}

fn key(c: char, ctrl: bool, shift: bool, alt: bool) -> charis_ui::Event {
    charis_ui::Event::Key(KeyEvent {
        key: Key::Char(c),
        mods: Modifiers { ctrl, shift, alt, meta: false },
        repeat: false,
    })
}

fn harness() -> Headless<Ed> {
    let mut h = Headless::new(Ed::default(), 600.0, 400.0, 1.0);
    h.settle();
    h
}

#[test]
fn shortcuts_follow_the_declaration() {
    let mut h = harness();
    h.event(key('s', true, false, false));
    h.event(key('E', true, true, false)); // in a submenu
    h.event(key('P', true, true, false)); // disabled: nothing
    h.event(key('z', false, false, true)); // check item
    assert_eq!(h.rt.app.log, ["save", "html"]);
    assert!(h.rt.app.wrap);
    h.rt.app.exporting = true;
    h.rt.invalidate();
    h.event(key('P', true, true, false));
    assert_eq!(h.rt.app.log, ["save", "html", "pdf"]);
}

#[test]
fn focused_inputs_keep_their_editing_keys() {
    let mut h = harness();
    let r = h.rt.rect_of("input").unwrap();
    h.click(r.center().x, r.center().y);
    h.type_text("hello");
    // Ctrl+A / Ctrl+C in the input copy the text, not the menu's "Copy"...
    h.event(key('a', true, false, false));
    h.event(key('c', true, false, false));
    assert!(h.rt.app.log.is_empty(), "input handled Ctrl+C: {:?}", h.rt.app.log);
    // ...but shortcuts the input doesn't use still reach the menu.
    h.event(key('s', true, false, false));
    assert_eq!(h.rt.app.log, ["save"]);
    // Without focus in an input, Ctrl+C is the menu's.
    h.click(500.0, 380.0);
    h.event(key('c', true, false, false));
    assert_eq!(h.rt.app.log, ["save", "copy"]);
}

#[test]
fn menubar_opens_picks_and_closes() {
    let mut h = harness();
    let file = h.rt.rect_of_text("File").unwrap().center();
    h.click(file.x, file.y);
    let save = h.rt.rect_of_text("Save").expect("File menu open").center();
    // The shortcut hint is shown.
    assert!(h.rt.rect_of_text("Ctrl+S").is_some() || h.rt.rect_of_text("⌘S").is_some());
    h.click(save.x, save.y);
    assert_eq!(h.rt.app.log, ["save"]);
    assert!(h.rt.rect_of_text("Save").is_none(), "picking closes the menu");

    // Submenu: hover "Export", pick "HTML" in the flyout.
    h.click(file.x, file.y);
    let export = h.rt.rect_of_text("Export").unwrap().center();
    h.move_to(export.x, export.y);
    h.settle();
    let html = h.rt.rect_of_text("HTML").expect("submenu open on hover").center();
    assert!(html.x > export.x + 50.0, "flyout to the right");
    h.move_to(html.x, export.y);
    h.move_to(html.x, html.y);
    h.click(html.x, html.y);
    assert_eq!(h.rt.app.log, ["save", "html"]);
    assert!(h.rt.rect_of_text("HTML").is_none());

    // Clicking outside closes without picking.
    h.click(file.x, file.y);
    assert!(h.rt.rect_of_text("Save").is_some());
    h.click(500.0, 380.0);
    assert!(h.rt.rect_of_text("Save").is_none());
    assert_eq!(h.rt.app.log.len(), 2);
}

#[test]
fn shortcuts_work_in_every_window() {
    let mut h = HeadlessApp::new(Ed { secondary: true, ..Default::default() }, 600.0, 400.0);
    h.event(Some("aux"), key('s', true, false, false));
    assert_eq!(h.app().log, ["save"]);
}
