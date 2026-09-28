//! The chrome map drives native title-bar behaviour (Windows snap, Snap
//! Layouts, system menu). Verify it classifies regions correctly.

use charis_ui::prelude::*;
use charis_ui::ChromeHit;

struct App1 {
    menu: Option<usize>,
}

#[derive(Clone)]
enum Msg {
    Menu(Option<usize>),
    Noop,
}

impl App for App1 {
    type Msg = Msg;
    fn update(&mut self, msg: Msg, _: &mut Cx<Msg>) {
        if let Msg::Menu(m) = msg {
            self.menu = m;
        }
    }
    fn view(&self) -> Element<Msg> {
        let left = row().h_full().items_center().child(menu_bar(
            vec![Menu::new("File", vec![MenuItem::action("Quit", Msg::Noop)])],
            self.menu,
            Msg::Menu,
        ));
        let right = row().pr(4.0).child(icon_button(Icon::Search).id("search").on_click(Msg::Noop));
        col().size_full().child(titlebar("Title", left, right, false)).child(div().grow(1.0).id("body"))
    }
}

#[test]
fn chrome_regions() {
    let mut h = Headless::new(App1 { menu: None }, 800.0, 400.0, 1.0);
    h.settle();
    let map = h.rt.chrome_map();
    // Empty title-bar space drags the window.
    assert_eq!(map.hit(Point::new(400.0, 15.0)), ChromeHit::Caption);
    // Menu titles and toolbar buttons stay clickable.
    assert_eq!(map.hit(Point::new(20.0, 15.0)), ChromeHit::Client);
    let s = h.rt.rect_of("search").unwrap().center();
    assert_eq!(map.hit(s), ChromeHit::Client);
    // The maximize button (second of the three window controls) maps to the OS maximize button.
    assert_eq!(map.hit(Point::new(800.0 - 46.0 - 23.0, 15.0)), ChromeHit::Maximize);
    assert_eq!(map.hit(Point::new(800.0 - 23.0, 15.0)), ChromeHit::Client, "close is handled by the app");
    // Content is client.
    assert_eq!(map.hit(Point::new(400.0, 200.0)), ChromeHit::Client);
    // An open menu's backdrop covers the title bar: clicks close the menu instead of dragging.
    h.click(20.0, 15.0);
    assert_eq!(h.rt.app.menu, Some(0));
    assert_eq!(h.rt.chrome_map().hit(Point::new(400.0, 15.0)), ChromeHit::Client);
}
