//! The widget gallery example: every page builds, lays out and responds.

#[path = "../examples/gallery.rs"]
#[allow(dead_code)]
mod gallery;

use gallery::{Gallery, Msg, Page};
use rust_ui::prelude::*;
use rust_ui::MouseButton;

fn open() -> Headless<Gallery> {
    let mut h = Headless::new(Gallery::default(), 1180.0, 820.0, 1.0);
    h.settle();
    h
}

fn click_id(h: &mut Headless<Gallery>, id: &str) {
    let r = h.rt.rect_of(id).unwrap_or_else(|| panic!("no element {id:?}"));
    h.click(r.center().x, r.center().y);
    h.settle();
}

fn click_text(h: &mut Headless<Gallery>, s: &str) {
    let r = h.rt.rect_of_text(s).unwrap_or_else(|| panic!("no text {s:?}"));
    h.click(r.center().x, r.center().y);
    h.settle();
}

#[test]
fn every_page_renders_from_the_sidebar() {
    let mut h = open();
    let dir = std::env::var_os("GALLERY_SHOTS");
    for p in Page::ALL {
        click_id(&mut h, &format!("page-{p:?}").to_lowercase());
        assert_eq!(h.rt.app.page, p);
        // The page title shows twice: in the sidebar and as the heading.
        assert!(h.rt.rect_of_text(p.title()).is_some(), "{p:?} has no title");
        h.rt.render();
        if let Some(dir) = &dir {
            h.save_png(std::path::Path::new(dir).join(format!("{p:?}.png"))).unwrap();
        }
    }
}

#[test]
fn widgets_respond() {
    let mut h = open();
    click_id(&mut h, "counter");
    click_id(&mut h, "counter");
    assert_eq!(h.rt.app.clicks, 2);
    assert!(h.rt.rect_of_text("Pressed 2 times").is_some());

    click_id(&mut h, "page-selection");
    let before = h.rt.app.checked;
    click_id(&mut h, "check");
    assert_eq!(h.rt.app.checked, !before);
    click_text(&mut h, "Large");
    assert_eq!(h.rt.app.radio, Some(2));

    click_id(&mut h, "page-inputs");
    click_id(&mut h, "name");
    h.type_text("!");
    assert!(h.rt.app.name.ends_with('!'), "typed into the name field: {:?}", h.rt.app.name);
}

#[test]
fn table_sorts_and_selects() {
    let mut h = open();
    click_id(&mut h, "page-data");
    click_text(&mut h, "Age");
    assert_eq!(h.rt.app.sort.0, 2);
    click_text(&mut h, "Grace Hopper");
    assert!(h.rt.app.selected_row.is_some());
}

#[test]
fn overlays_open_and_close() {
    let mut h = open();
    click_id(&mut h, "page-overlays");

    // Context menu.
    let r = h.rt.rect_of("context-area").unwrap();
    let at = r.center();
    h.event(rust_ui::Event::PointerDown(at, MouseButton::Right));
    h.event(rust_ui::Event::PointerUp(at, MouseButton::Right));
    h.settle();
    assert!(h.rt.app.context.is_some());
    click_text(&mut h, "Copy");
    assert_eq!(h.rt.app.last_action, "Copy");
    assert!(h.rt.app.context.is_none());

    // Dialog.
    click_id(&mut h, "open-dialog");
    assert!(h.rt.app.dialog);
    click_id(&mut h, "discard");
    assert!(!h.rt.app.dialog);
    assert_eq!(h.rt.app.last_action, "Discard");
}

#[test]
fn theme_knobs_apply_live() {
    let mut h = open();
    click_id(&mut h, "page-theme");
    let bg_dark = h.pixel(600.0, 5.0);
    click_id(&mut h, "dark-switch");
    assert!(!h.rt.app.dark);
    let bg_light = h.pixel(600.0, 5.0);
    assert!(bg_light[0] > bg_dark[0] + 100, "light background {bg_light:?} vs dark {bg_dark:?}");

    click_id(&mut h, "high-contrast");
    assert!(h.rt.app.high_contrast);
    h.rt.send(Msg::Accent(5));
    h.settle();
    assert!(h.rt.rect_of_text(&format!("accent {}", Accent::ALL[5].name())).is_some());
}

#[test]
fn pasting_an_image_shows_it() {
    let mut h = open();
    click_id(&mut h, "page-media");
    click_id(&mut h, "paste-target");
    let img = rust_ui::image::Image::from_rgba(4, 3, &[200u8; 48]).unwrap();
    h.event(rust_ui::Event::PasteImage(img));
    h.settle();
    assert!(h.rt.app.pasted.is_some());
    assert!(h.rt.rect_of_text("Pasted a 4×3 image").is_some());
}
