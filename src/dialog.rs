//! Native file dialogs.
//!
//! Ask from `update` through [`Cx`](crate::Cx); the answer arrives as a
//! message, so the UI never blocks:
//!
//! ```no_run
//! # use rust_ui::prelude::*;
//! # use std::path::PathBuf;
//! # #[derive(Clone)] enum Msg { Open, Opened(Option<PathBuf>) }
//! # struct Editor;
//! # impl Editor {
//! fn update(&mut self, msg: Msg, cx: &mut Cx<Msg>) {
//!     match msg {
//!         Msg::Open => cx.open_file(
//!             FileDialog::new().title("Open model").filter("GGUF models", &["gguf"]),
//!             Msg::Opened,
//!         ),
//!         Msg::Opened(Some(path)) => { /* load it */ }
//!         Msg::Opened(None) => {} // cancelled
//!     }
//! }
//! # }
//! ```
//!
//! The window shell shows the platform dialog (Windows' common item dialog,
//! macOS panels, the XDG desktop portal on Linux), modal to the window that
//! asked. Headless runtimes answer with a responder you install
//! ([`Runtime::set_dialog_responder`](crate::Runtime::set_dialog_responder)),
//! so tests can script the user's choice.

use std::path::PathBuf;

/// What kind of dialog to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DialogKind {
    OpenFile,
    OpenFiles,
    PickFolder,
    SaveFile,
}

/// A file dialog's options.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FileDialog {
    pub title: Option<String>,
    /// Folder to start in.
    pub directory: Option<PathBuf>,
    /// Suggested file name (save dialogs).
    pub file_name: Option<String>,
    /// (name, extensions without dots), e.g. `("Images", ["png", "jpg"])`.
    pub filters: Vec<(String, Vec<String>)>,
}

impl FileDialog {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = Some(t.into());
        self
    }
    pub fn directory(mut self, d: impl Into<PathBuf>) -> Self {
        self.directory = Some(d.into());
        self
    }
    pub fn file_name(mut self, n: impl Into<String>) -> Self {
        self.file_name = Some(n.into());
        self
    }
    /// Add a file type filter (extensions without the dot).
    pub fn filter(mut self, name: impl Into<String>, extensions: &[&str]) -> Self {
        self.filters.push((name.into(), extensions.iter().map(|e| e.to_string()).collect()));
        self
    }
}

/// A dialog the app asked for, waiting to be shown.
#[derive(Debug, Clone, PartialEq)]
pub struct DialogRequest {
    pub kind: DialogKind,
    pub dialog: FileDialog,
}

/// Show a request with the platform dialog (blocking until the user
/// answers). Returns the chosen paths; empty when cancelled.
#[cfg(feature = "dialogs")]
pub(crate) async fn show(req: DialogRequest, parent: Option<rfd::AsyncFileDialog>) -> Vec<PathBuf> {
    let d = &req.dialog;
    let mut f = parent.unwrap_or_default();
    if let Some(t) = &d.title {
        f = f.set_title(t);
    }
    if let Some(dir) = &d.directory {
        f = f.set_directory(dir);
    }
    if let Some(n) = &d.file_name {
        f = f.set_file_name(n);
    }
    for (name, exts) in &d.filters {
        f = f.add_filter(name, exts);
    }
    let one = |h: Option<rfd::FileHandle>| h.map(|h| h.path().to_path_buf()).into_iter().collect();
    match req.kind {
        DialogKind::OpenFile => one(f.pick_file().await),
        DialogKind::OpenFiles => {
            f.pick_files().await.unwrap_or_default().iter().map(|h| h.path().to_path_buf()).collect()
        }
        DialogKind::PickFolder => one(f.pick_folder().await),
        DialogKind::SaveFile => one(f.save_file().await),
    }
}
