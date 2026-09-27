//! Clipboard: copying text and images, reading the clipboard back, and
//! pasting an image into a focused element.

use charis_ui::image::Image;
use charis_ui::prelude::*;
use charis_ui::{ClipboardContent, Event, WindowRequest};

struct Clip {
    got: Vec<ClipboardContent>,
    pasted: Option<Image>,
}

#[derive(Clone)]
enum Msg {
    CopyText,
    CopyImage,
    Read,
    Got(ClipboardContent),
    Pasted(Image),
}

fn pixel_image() -> Image {
    Image::from_rgba(2, 1, &[10, 20, 30, 255, 40, 50, 60, 128]).unwrap()
}

impl App for Clip {
    type Msg = Msg;
    fn update(&mut self, m: Msg, cx: &mut Cx<Msg>) {
        match m {
            Msg::CopyText => cx.copy_to_clipboard("hello"),
            Msg::CopyImage => cx.copy_image(pixel_image()),
            Msg::Read => cx.read_clipboard(Msg::Got),
            Msg::Got(c) => self.got.push(c),
            Msg::Pasted(i) => self.pasted = Some(i),
        }
    }
    fn view(&self) -> Element<Msg> {
        col().child(div().id("drop").size(200.0, 100.0).focusable().on_paste_image(Msg::Pasted))
    }
}

fn app() -> Headless<Clip> {
    let mut h = Headless::new(Clip { got: Vec::new(), pasted: None }, 300.0, 200.0, 1.0);
    h.settle();
    h
}

#[test]
fn copy_and_read_back() {
    let mut h = app();
    h.rt.send(Msg::Read);
    h.rt.send(Msg::CopyText);
    h.rt.send(Msg::Read);
    h.rt.send(Msg::CopyImage);
    h.rt.send(Msg::Read);
    h.settle();
    let got = &h.rt.app.got;
    assert_eq!(got[0], ClipboardContent::Empty);
    assert_eq!(got[1], ClipboardContent::Text("hello".into()));
    match &got[2] {
        ClipboardContent::Image(i) => assert_eq!(i.to_rgba(), pixel_image().to_rgba()),
        other => panic!("{other:?}"),
    }
    // The shell gets the requests to write the system clipboard.
    let reqs = h.rt.take_requests();
    assert!(reqs.iter().any(|r| matches!(r, WindowRequest::SetClipboard(t) if t == "hello")));
    assert!(reqs.iter().any(|r| matches!(r, WindowRequest::SetClipboardImage(_))));
}

#[test]
fn external_clipboard_reads_go_to_the_shell() {
    let mut h = app();
    h.rt.set_external_clipboard(true);
    h.rt.send(Msg::Read);
    h.settle();
    assert!(h.rt.app.got.is_empty());
    let ids = h.rt.take_clipboard_requests();
    assert_eq!(ids.len(), 1);
    h.rt.clipboard_done(ids[0], ClipboardContent::Text("from the OS".into()));
    assert_eq!(h.rt.app.got, vec![ClipboardContent::Text("from the OS".into())]);
}

#[test]
fn paste_image_into_the_focused_element() {
    let mut h = app();
    h.event(Event::PasteImage(pixel_image()));
    assert!(h.rt.app.pasted.is_none(), "nothing focused");
    let r = h.rt.rect_of("drop").unwrap().center();
    h.click(r.x, r.y);
    h.event(Event::PasteImage(pixel_image()));
    h.settle();
    assert_eq!(h.rt.app.pasted.as_ref().map(|i| (i.width(), i.height())), Some((2, 1)));
}
