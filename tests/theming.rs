//! Runtime theme switching: the app's `theme()` is re-read every frame, so a
//! message that changes appearance state repaints with the new tokens.

use rust_ui::prelude::*;

struct Themed {
    dark: bool,
    accent: Accent,
}

#[derive(Clone, Debug)]
enum Msg {
    Pick(Accent),
    Light,
}

impl App for Themed {
    type Msg = Msg;
    fn theme(&self) -> Theme {
        Theme::from_config(ThemeConfig { dark: self.dark, accent: self.accent.color(self.dark), ..ThemeConfig::dark() })
    }
    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        match msg {
            Msg::Pick(a) => self.accent = a,
            Msg::Light => self.dark = false,
        }
    }
    fn view(&self) -> Element<Msg> {
        let c = theme().colors.clone();
        col()
            .size_full()
            .bg(c.background)
            .child(div().w(100.0).h(40.0).bg(c.accent).on_click(Msg::Pick(Accent::Teal)))
            .child(div().w(100.0).h(40.0).on_click(Msg::Light))
    }
}

fn close(px: [u8; 4], c: Color) -> bool {
    let want = [(c.r * 255.0).round(), (c.g * 255.0).round(), (c.b * 255.0).round()];
    (0..3).all(|i| (px[i] as f32 - want[i]).abs() <= 2.0)
}

#[test]
fn switching_accent_and_mode_repaints() {
    let mut h = Headless::new(Themed { dark: true, accent: Accent::Violet }, 200.0, 120.0, 1.0);
    h.settle();
    assert!(close(h.pixel(50.0, 20.0), Accent::Violet.color(true)));

    h.click(50.0, 20.0);
    h.settle();
    assert!(close(h.pixel(50.0, 20.0), Accent::Teal.color(true)), "accent swatch did not repaint");

    let dark_bg = h.pixel(150.0, 100.0);
    h.click(50.0, 60.0);
    h.settle();
    let light_bg = h.pixel(150.0, 100.0);
    assert!(light_bg[0] > 200 && dark_bg[0] < 60, "mode switch: {dark_bg:?} -> {light_bg:?}");
    assert!(close(h.pixel(50.0, 20.0), Accent::Teal.color(false)));
}
