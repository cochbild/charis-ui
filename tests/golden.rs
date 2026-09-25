//! Visual regression tests: render widget galleries and compare them with
//! checked-in reference images. Run with `UPDATE_GOLDEN=1` to accept changes.
//! Uses only the bundled Inter font so results don't depend on system fonts.
#![cfg(feature = "bundled-fonts")]

use rust_ui::prelude::*;

struct Gallery {
    dark: bool,
}

#[derive(Clone)]
enum Msg {
    Noop,
}

impl App for Gallery {
    type Msg = Msg;
    fn theme(&self) -> Theme {
        if self.dark {
            Theme::dark()
        } else {
            Theme::light()
        }
    }
    fn update(&mut self, _: Msg, _: &mut Cx) {}
    fn view(&self) -> Element<Msg> {
        let th = theme();
        let c = &th.colors;
        row()
            .size_full()
            .child(
                col()
                    .w(220.0)
                    .bg(c.panel)
                    .border_r(1.0, c.border)
                    .child(section_header("Explorer"))
                    .child(tree_row(0, Some(true), Some(Icon::Folder), "src", false))
                    .child(tree_row(1, None, Some(Icon::File), "main.rs", true))
                    .child(tree_row(1, None, Some(Icon::File), "a_very_long_file_name_that_truncates.rs", false))
                    .child(list_item(Some(Icon::Settings), "Settings", false)),
            )
            .child(
                col()
                    .grow(1.0)
                    .child(tab_bar(vec![
                        Tab::new("main.rs", true, Msg::Noop).icon(Icon::Code).closable(Msg::Noop),
                        Tab::new("lib.rs", false, Msg::Noop).icon(Icon::Code).closable(Msg::Noop).modified(true),
                    ]))
                    .child(
                        col()
                            .p(20.0)
                            .gap(14.0)
                            .child(
                                row()
                                    .gap(8.0)
                                    .items_center()
                                    .child(primary_button("Primary").with_icon(Icon::Check))
                                    .child(button("Secondary"))
                                    .child(ghost_button("Ghost"))
                                    .child(danger_button("Danger"))
                                    .child(icon_button(Icon::More)),
                            )
                            .child(
                                row()
                                    .gap(16.0)
                                    .items_center()
                                    .child(checkbox("Checked", true))
                                    .child(checkbox("Unchecked", false))
                                    .child(switch(true))
                                    .child(switch(false))
                                    .child(badge("12"))
                                    .child(tag("tag", c.success))
                                    .child(kbd("Ctrl+P")),
                            )
                            .child(
                                row()
                                    .gap(12.0)
                                    .child(text_input("Hello", |_| Msg::Noop).w(220.0))
                                    .child(text_input("", |_| Msg::Noop).placeholder("Placeholder").w(220.0)),
                            )
                            .child(
                                row()
                                    .gap(12.0)
                                    .items_center()
                                    .child(slider(40.0, 0.0, 100.0).w(200.0))
                                    .child(progress(0.6).w(200.0)),
                            )
                            .child(segmented(vec![
                                ("One".into(), true, Msg::Noop),
                                ("Two".into(), false, Msg::Noop),
                                ("Three".into(), false, Msg::Noop),
                            ]))
                            .child(
                                row()
                                    .gap(16.0)
                                    .items(Align::Start)
                                    .child(
                                        card().w(240.0).child(text("Card title").semibold()).child(
                                            text("Body text that wraps across multiple lines inside the card.")
                                                .color(c.text_muted),
                                        ),
                                    )
                                    .child(menu_panel(vec![
                                        MenuItem::action("Copy", Msg::Noop).icon(Icon::Files).shortcut("Ctrl+C"),
                                        MenuItem::action("Disabled", Msg::Noop).disabled(true),
                                        MenuItem::Separator,
                                        MenuItem::check("Word wrap", true, Msg::Noop),
                                    ])),
                            ),
                    ),
            )
    }
}

fn check(name: &str, dark: bool) {
    let mut h = Headless::new(Gallery { dark }, 960.0, 560.0, 1.0);
    h.settle();
    let got = h.rt.render().clone();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden").join(format!("{name}.png"));
    if std::env::var("UPDATE_GOLDEN").is_ok() || !path.exists() {
        got.save_png(&path).expect("write golden");
        eprintln!("updated {}", path.display());
        return;
    }
    let want = tiny_skia::Pixmap::load_png(&path).expect("read golden");
    assert_eq!((got.width(), got.height()), (want.width(), want.height()), "size changed");
    let mut bad = 0usize;
    let mut worst = 0u8;
    for (a, b) in got.data().chunks_exact(4).zip(want.data().chunks_exact(4)) {
        let d = (0..3).map(|i| a[i].abs_diff(b[i])).max().unwrap_or(0);
        worst = worst.max(d);
        if d > 24 {
            bad += 1;
        }
    }
    if bad > 20 {
        let out = path.with_extension("actual.png");
        got.save_png(&out).ok();
        panic!("{name}: {bad} pixels differ (worst {worst}); actual written to {}", out.display());
    }
}

#[test]
fn gallery_dark() {
    check("gallery-dark", true);
}

#[test]
fn gallery_light() {
    check("gallery-light", false);
}
