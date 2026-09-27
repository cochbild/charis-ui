//! Named layouts ("workspaces"): save the current arrangement of panels
//! under a name and switch between arrangements, like JetBrains layouts
//! or Visual Studio window layouts.
//!
//! A [`Layouts`] store holds snapshots of any cloneable state `S`, usually
//! a struct with your [`DockSpace`](crate::dock::DockSpace) and
//! [`ToolWindows`](crate::toolwin::ToolWindows). Layouts are explicit
//! snapshots: applying one replaces the arrangement, and changes made
//! afterwards are kept only when saved ("Save Changes to …").
//!
//! It comes with the usual commands as menu items ([`Layouts::menu_items`],
//! e.g. for a Window ▸ Layouts submenu) and a "Save Layout As" dialog
//! ([`Layouts::dialog`]). With the `serde` feature the store serializes, so
//! layouts can persist across runs.
//!
//! ```no_run
//! use charis_ui::prelude::*;
//! use charis_ui::layouts::{LayoutMsg, Layouts};
//!
//! #[derive(Clone)]
//! struct Arrangement { sidebar: bool }
//!
//! struct Ide { now: Arrangement, layouts: Layouts<Arrangement> }
//! #[derive(Clone)] enum Msg { Layout(LayoutMsg) }
//!
//! impl App for Ide {
//!     type Msg = Msg;
//!     fn update(&mut self, msg: Msg, _: &mut Cx<Msg>) {
//!         let Msg::Layout(m) = msg;
//!         if let Some(a) = self.layouts.update(m, &self.now) {
//!             self.now = a; // a layout was applied
//!         }
//!     }
//!     fn menu(&self) -> Vec<Menu<Msg>> {
//!         vec![Menu::new("Window", vec![MenuItem::submenu("Layouts", self.layouts.menu_items(&Msg::Layout))])]
//!     }
//!     fn view(&self) -> Element<Msg> {
//!         div().size_full().children(self.layouts.dialog(Msg::Layout))
//!     }
//! }
//! let layouts = Layouts::new().with("Default", Arrangement { sidebar: true });
//! # let _ = Ide { now: Arrangement { sidebar: true }, layouts };
//! ```

use crate::element::*;
use crate::theme::theme;
use crate::widgets::{button, modal, primary_button, text_input, MenuItem};

/// Messages of the layout commands. Forward them to [`Layouts::update`].
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutMsg {
    /// Switch to this layout.
    Apply(String),
    /// Save the current arrangement under this name (replacing a layout
    /// with that name).
    Save(String),
    /// Delete the layout with this name.
    Delete(String),
    /// Rename a layout (from, to).
    Rename(String, String),
    /// Open the "Save Layout As" dialog.
    SaveAs,
    /// The dialog's name field changed.
    Draft(String),
    /// Confirm the dialog.
    Confirm,
    /// Close the dialog.
    Cancel,
}

/// Named snapshots of an arrangement; see the [module docs](self).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Layouts<S> {
    entries: Vec<(String, S)>,
    /// The layout last applied or saved.
    current: Option<String>,
    /// The "Save Layout As" dialog's name, while it's open.
    #[cfg_attr(feature = "serde", serde(skip))]
    draft: Option<String>,
}

impl<S> Default for Layouts<S> {
    fn default() -> Self {
        Self { entries: Vec::new(), current: None, draft: None }
    }
}

impl<S: Clone> Layouts<S> {
    /// No layouts, none current.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a layout (builder style), e.g. built-in defaults. The first one
    /// becomes current.
    pub fn with(mut self, name: impl Into<String>, state: S) -> Self {
        let name = name.into();
        if self.current.is_none() {
            self.current = Some(name.clone());
        }
        self.put(name, state);
        self
    }

    fn put(&mut self, name: String, state: S) {
        match self.entries.iter_mut().find(|e| e.0 == name) {
            Some(e) => e.1 = state,
            None => self.entries.push((name, state)),
        }
    }

    /// The layouts' names, in the order they were added.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|e| e.0.as_str())
    }

    /// Number of layouts.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether there are no layouts.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The layout last applied or saved.
    pub fn current(&self) -> Option<&str> {
        self.current.as_deref()
    }

    /// The state saved under `name` (doesn't change the current layout).
    pub fn get(&self, name: &str) -> Option<&S> {
        self.entries.iter().find(|e| e.0 == name).map(|e| &e.1)
    }

    /// Save `state` under `name` (replacing a layout with that name); it
    /// becomes current.
    pub fn save(&mut self, name: impl Into<String>, state: S) {
        let name = name.into();
        self.put(name.clone(), state);
        self.current = Some(name);
    }

    /// The state of layout `name`, which becomes current.
    pub fn apply(&mut self, name: &str) -> Option<S> {
        let s = self.get(name)?.clone();
        self.current = Some(name.to_string());
        Some(s)
    }

    /// Delete layout `name` and return its state; if it was current, none is.
    pub fn remove(&mut self, name: &str) -> Option<S> {
        let i = self.entries.iter().position(|e| e.0 == name)?;
        if self.current.as_deref() == Some(name) {
            self.current = None;
        }
        Some(self.entries.remove(i).1)
    }

    /// Rename a layout; false if `from` doesn't exist or `to` is taken.
    pub fn rename(&mut self, from: &str, to: impl Into<String>) -> bool {
        let to = to.into();
        if to.trim().is_empty() || self.get(&to).is_some() {
            return false;
        }
        let Some(e) = self.entries.iter_mut().find(|e| e.0 == from) else { return false };
        e.0 = to.clone();
        if self.current.as_deref() == Some(from) {
            self.current = Some(to);
        }
        true
    }

    /// Is the "Save Layout As" dialog open?
    pub fn dialog_open(&self) -> bool {
        self.draft.is_some()
    }

    /// The name in the "Save Layout As" dialog, while it's open.
    pub fn draft(&self) -> Option<&str> {
        self.draft.as_deref()
    }

    /// Apply a message. `now` is the current arrangement (saved by the
    /// save commands); returns the arrangement to switch to when a layout
    /// is applied.
    pub fn update(&mut self, msg: LayoutMsg, now: &S) -> Option<S> {
        match msg {
            LayoutMsg::Apply(name) => return self.apply(&name),
            LayoutMsg::Save(name) => self.save(name, now.clone()),
            LayoutMsg::Delete(name) => {
                self.remove(&name);
            }
            LayoutMsg::Rename(from, to) => {
                self.rename(&from, to);
            }
            LayoutMsg::SaveAs => {
                // Suggest a fresh name.
                let mut n = self.entries.len() + 1;
                while self.get(&format!("Layout {n}")).is_some() {
                    n += 1;
                }
                self.draft = Some(format!("Layout {n}"));
            }
            LayoutMsg::Draft(s) => {
                if self.draft.is_some() {
                    self.draft = Some(s);
                }
            }
            LayoutMsg::Confirm => {
                if let Some(name) = self.draft.take() {
                    let name = name.trim().to_string();
                    if name.is_empty() {
                        self.draft = Some(name);
                    } else {
                        self.save(name, now.clone());
                    }
                }
            }
            LayoutMsg::Cancel => self.draft = None,
        }
        None
    }

    /// The layout commands, for a menu (e.g. Window ▸ Layouts): each
    /// layout (checked when current), Save Changes to the current one,
    /// Save Layout As…, and Delete ▸.
    pub fn menu_items<M>(&self, map: &dyn Fn(LayoutMsg) -> M) -> Vec<MenuItem<M>> {
        let mut items: Vec<MenuItem<M>> = self
            .entries
            .iter()
            .map(|(n, _)| MenuItem::check(n.clone(), self.current() == Some(n), map(LayoutMsg::Apply(n.clone()))))
            .collect();
        if !items.is_empty() {
            items.push(MenuItem::Separator);
        }
        if let Some(cur) = self.current() {
            items.push(MenuItem::action(format!("Save Changes to “{cur}”"), map(LayoutMsg::Save(cur.to_string()))));
        }
        items.push(MenuItem::action("Save Layout As…", map(LayoutMsg::SaveAs)));
        if !self.entries.is_empty() {
            items.push(MenuItem::submenu(
                "Delete Layout",
                self.entries
                    .iter()
                    .map(|(n, _)| MenuItem::action(n.clone(), map(LayoutMsg::Delete(n.clone()))))
                    .collect(),
            ));
        }
        items
    }

    /// The "Save Layout As" dialog while it's open: a name field (Enter
    /// saves, Escape cancels), noting when the name replaces a layout.
    pub fn dialog<M: Clone + 'static>(&self, map: impl Fn(LayoutMsg) -> M + 'static) -> Option<Element<M>> {
        let draft = self.draft.as_ref()?;
        let th = theme();
        let replaces = self.get(draft.trim()).is_some();
        let empty = draft.trim().is_empty();
        let input = std::rc::Rc::new(map);
        let m = input.clone();
        let body = col()
            .gap(8.0)
            .child(text("Name").font_size(th.font_size_sm))
            .child(
                text_input(draft.clone(), move |s| m(LayoutMsg::Draft(s)))
                    .id("layout-name")
                    .aria_label("Layout name")
                    .autofocus()
                    .on_submit(input(LayoutMsg::Confirm)),
            )
            .child_if(replaces, || {
                text(format!("Replaces the layout “{}”.", draft.trim()))
                    .font_size(th.font_size_sm)
                    .color(th.colors.warning)
            });
        let save = primary_button(if replaces { "Replace" } else { "Save" }).id("layout-save").disabled(empty);
        let save = if empty { save } else { save.on_click(input(LayoutMsg::Confirm)) };
        Some(modal(
            "Save Layout As",
            body,
            vec![button("Cancel").on_click(input(LayoutMsg::Cancel)), save],
            input(LayoutMsg::Cancel),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store() {
        let mut l = Layouts::new().with("A", 1).with("B", 2);
        assert_eq!(l.current(), Some("A"));
        assert_eq!(l.update(LayoutMsg::Apply("B".into()), &0), Some(2));
        assert_eq!(l.current(), Some("B"));
        // Save changes to the current layout.
        l.update(LayoutMsg::Save("B".into()), &5);
        assert_eq!(l.get("B"), Some(&5));
        // Save As: a suggested name, edited, confirmed.
        l.update(LayoutMsg::SaveAs, &7);
        assert!(l.dialog_open());
        l.update(LayoutMsg::Draft("  Debug ".into()), &7);
        l.update(LayoutMsg::Confirm, &7);
        assert!(!l.dialog_open());
        assert_eq!(l.names().collect::<Vec<_>>(), ["A", "B", "Debug"]);
        assert_eq!(l.current(), Some("Debug"));
        // An empty name keeps the dialog open.
        l.update(LayoutMsg::SaveAs, &7);
        l.update(LayoutMsg::Draft(" ".into()), &7);
        l.update(LayoutMsg::Confirm, &7);
        assert!(l.dialog_open());
        l.update(LayoutMsg::Cancel, &7);
        assert!(!l.dialog_open());
        assert!(!l.rename("A", "B"), "name taken");
        assert!(l.rename("Debug", "Debugging"));
        assert_eq!(l.current(), Some("Debugging"));
        l.update(LayoutMsg::Delete("Debugging".into()), &0);
        assert_eq!(l.current(), None);
        assert_eq!(l.len(), 2);
    }
}
