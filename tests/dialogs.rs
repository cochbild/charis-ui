//! File dialogs: asked for from `update`, answered as messages.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use charis_ui::dialog::{DialogKind, DialogRequest};
use charis_ui::prelude::*;

#[derive(Default)]
struct Ed {
    opened: Vec<Option<PathBuf>>,
    many: Vec<PathBuf>,
    saved: Option<PathBuf>,
}

#[derive(Clone, Debug)]
enum Msg {
    Open,
    OpenMany,
    Save,
    Opened(Option<PathBuf>),
    OpenedMany(Vec<PathBuf>),
    Saved(Option<PathBuf>),
}

impl App for Ed {
    type Msg = Msg;
    fn update(&mut self, m: Msg, cx: &mut Cx<Msg>) {
        match m {
            Msg::Open => cx.open_file(FileDialog::new().title("Open model").filter("GGUF", &["gguf"]), Msg::Opened),
            Msg::OpenMany => cx.open_files(FileDialog::new(), Msg::OpenedMany),
            Msg::Save => cx.save_file(FileDialog::new().file_name("chat.html").directory("/tmp"), Msg::Saved),
            Msg::Opened(p) => self.opened.push(p),
            Msg::OpenedMany(v) => self.many = v,
            Msg::Saved(p) => self.saved = p,
        }
    }
    fn view(&self) -> Element<Msg> {
        div()
    }
}

#[test]
fn responder_answers_like_the_user() {
    let mut h = Headless::new(Ed::default(), 200.0, 100.0, 1.0);
    let seen: Rc<RefCell<Vec<DialogRequest>>> = Default::default();
    let log = seen.clone();
    let mut answers = vec![vec![], vec![PathBuf::from("/models/q4.gguf")]];
    h.rt.set_dialog_responder(move |req| {
        log.borrow_mut().push(req.clone());
        match req.kind {
            DialogKind::OpenFile => answers.pop().unwrap_or_default(),
            DialogKind::OpenFiles => vec!["/a".into(), "/b".into()],
            DialogKind::SaveFile => vec![PathBuf::from("/tmp").join(req.dialog.file_name.clone().unwrap())],
            DialogKind::PickFolder => vec![],
            _ => vec![],
        }
    });
    h.rt.send(Msg::Open);
    h.rt.send(Msg::Open);
    h.rt.send(Msg::OpenMany);
    h.rt.send(Msg::Save);
    h.advance(0.016);
    let app = &h.rt.app;
    assert_eq!(app.opened, [Some(PathBuf::from("/models/q4.gguf")), None], "chosen, then cancelled");
    assert_eq!(app.many, [PathBuf::from("/a"), PathBuf::from("/b")]);
    assert_eq!(app.saved, Some(PathBuf::from("/tmp/chat.html")));
    let seen = seen.borrow();
    assert_eq!(seen[0].dialog.title.as_deref(), Some("Open model"));
    assert_eq!(seen[0].dialog.filters, [("GGUF".to_string(), vec!["gguf".to_string()])]);
}

#[test]
fn shells_take_requests_and_answer_later() {
    let mut h = Headless::new(Ed::default(), 200.0, 100.0, 1.0);
    h.rt.send(Msg::Save);
    h.advance(0.016);
    let reqs = h.rt.take_dialog_requests();
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].1.kind, DialogKind::SaveFile);
    assert!(h.rt.app.saved.is_none(), "waiting for the user");
    h.rt.dialog_done(reqs[0].0, vec!["/home/me/chat.html".into()]);
    assert_eq!(h.rt.app.saved, Some(PathBuf::from("/home/me/chat.html")));
    // Unknown or repeated ids are ignored.
    h.rt.dialog_done(reqs[0].0, vec!["/other".into()]);
    assert_eq!(h.rt.app.saved, Some(PathBuf::from("/home/me/chat.html")));
}
