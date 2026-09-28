//! Theme style classes: restyle every built-in widget from the theme, with
//! CSS-like precedence (widget defaults < theme class < the app's own styling).

use charis_ui::prelude::*;

struct Demo {
    themed: bool,
}

#[derive(Clone, Debug)]
enum Msg {
    Go,
}

impl App for Demo {
    type Msg = Msg;
    fn theme(&self) -> Theme {
        let t = Theme::dark();
        if !self.themed {
            return t;
        }
        t.style_class("button", |e| e.pill().px(40.0))
            .style_class("button-primary", |e| e.bg(hex("#ff0000")).hover(|s| s.bg(hex("#00ff00"))))
            .style_class("sidebar", |e| e.w(123.0).bg(hex("#0000ff")))
            // Changing a knob keeps the classes.
            .with_accent(hex("#22aa88"))
    }
    fn update(&mut self, _: Msg, _: &mut Cx<Msg>) {}
    fn view(&self) -> Element<Msg> {
        row()
            .items(Align::Start)
            .gap(10.0)
            .p(10.0)
            .child(div().id("side").h(50.0).class("sidebar").class("no-such-class"))
            .child(primary_button("Go").id("primary").on_click(Msg::Go))
            .child(button("Plain").id("plain"))
            .child(primary_button("Go").id("tight").px(4.0))
    }
}

fn px(h: &Headless<Demo>, id: &str) -> [u8; 4] {
    let r = h.rt.rect_of(id).unwrap();
    h.pixel(r.x + r.w / 2.0, r.y + 3.0)
}

#[test]
fn theme_classes_restyle_built_in_widgets() {
    let mut plain = Headless::new(Demo { themed: false }, 600.0, 120.0, 1.0);
    plain.settle();
    let mut themed = Headless::new(Demo { themed: true }, 600.0, 120.0, 1.0);
    themed.settle();

    // `button` applies to every kind; `button-primary` only to primary.
    let w = |h: &Headless<Demo>, id| h.rt.rect_of(id).unwrap().w;
    assert!(w(&themed, "primary") > w(&plain, "primary") + 40.0);
    assert!(w(&themed, "plain") > w(&plain, "plain") + 40.0);
    assert_eq!(px(&themed, "primary")[..3], [255, 0, 0]);
    assert_ne!(px(&themed, "plain")[..3], [255, 0, 0]);

    // The app's own styling on an instance still wins over the class.
    assert!(w(&themed, "tight") < w(&themed, "primary") - 60.0);

    // App-defined classes work on any element; unknown names are ignored.
    assert_eq!(themed.rt.rect_of("side").unwrap().w, 123.0);
    assert_eq!(px(&themed, "side")[..3], [0, 0, 255]);
}

#[test]
fn class_state_styles_apply() {
    let mut h = Headless::new(Demo { themed: true }, 600.0, 120.0, 1.0);
    h.settle();
    let r = h.rt.rect_of("primary").unwrap();
    h.move_to(r.center().x, r.center().y);
    h.settle();
    assert_eq!(px(&h, "primary")[..3], [0, 255, 0], "hover style from the class");
}
