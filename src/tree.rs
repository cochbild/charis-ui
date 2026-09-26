//! A virtualized tree view: only the rows on screen are built, so trees
//! with hundreds of thousands of visible rows scroll smoothly.
//!
//! Your data implements [`TreeModel`]; children are asked for only when a
//! node is expanded, so the model can load them lazily (a file system, a
//! database). [`TreeState`] keeps what's expanded and selected, and the
//! flattened list of visible rows, rebuilt only when something expands,
//! collapses, or you call [`TreeState::refresh`] after the data changed.
//!
//! Keyboard (like VS Code's explorer): ↑/↓ move, → expands or goes to the
//! first child, ← collapses or goes to the parent, Home/End, PageUp/Down,
//! Enter activates, Space toggles, and typing jumps to a matching label.
//!
//! ```no_run
//! use std::rc::Rc;
//! use rust_ui::prelude::*;
//! use rust_ui::tree::{TreeEvent, TreeModel, TreeMsg, TreeState};
//!
//! struct Files; // e.g. a file system
//! impl TreeModel for Files {
//!     type Id = String;
//!     fn children(&self, parent: Option<&String>) -> Vec<String> {
//!         match parent { None => vec!["src".into()], Some(_) => vec!["src/main.rs".into()] }
//!     }
//!     fn has_children(&self, id: &String) -> bool { id == "src" }
//!     fn label(&self, id: &String) -> String { id.rsplit('/').next().unwrap_or(id).into() }
//! }
//!
//! struct App1 { files: Rc<Files>, tree: TreeState<String> }
//! #[derive(Clone)] enum Msg { Tree(TreeMsg<String>) }
//!
//! impl App for App1 {
//!     type Msg = Msg;
//!     fn update(&mut self, msg: Msg, cx: &mut Cx<Msg>) {
//!         let Msg::Tree(m) = msg;
//!         if let Some(TreeEvent::Activated(path)) = self.tree.update(m, &*self.files, cx) {
//!             println!("open {path}");
//!         }
//!     }
//!     fn view(&self) -> Element<Msg> {
//!         self.tree.view(&self.files, Msg::Tree)
//!     }
//! }
//! # let _ = App1 { files: Rc::new(Files), tree: TreeState::new("files") };
//! ```

use std::cell::RefCell;
use std::collections::HashSet;
use std::hash::Hash;
use std::rc::Rc;

use crate::element::*;
use crate::icons::Icon;
use crate::runtime::Cx;
use crate::semantics::Role;
use crate::style::*;
use crate::theme::theme;

/// Tree data. Only expanded nodes' children are asked for.
pub trait TreeModel: 'static {
    /// Identifies a node (a path, a database key…).
    type Id: Clone + Eq + Hash + 'static;

    /// The children of `parent`, or the roots for `None`, in display order.
    fn children(&self, parent: Option<&Self::Id>) -> Vec<Self::Id>;

    /// Whether the node can expand (shows a chevron). May be true before
    /// its children are loaded.
    fn has_children(&self, id: &Self::Id) -> bool;

    /// The text shown for the node.
    fn label(&self, id: &Self::Id) -> String;

    /// An icon before the label (`expanded` tells a folder's state); none by default.
    fn icon(&self, _id: &Self::Id, _expanded: bool) -> Option<Icon> {
        None
    }
}

/// A visible row of the flattened tree.
#[derive(Debug, Clone)]
pub struct TreeRow<Id> {
    /// The node shown in this row.
    pub id: Id,
    /// Nesting level (0 for roots).
    pub depth: usize,
    /// Index of the parent's row.
    pub parent: Option<usize>,
    /// Whether the node can expand.
    pub has_children: bool,
    /// Whether the node is expanded.
    pub expanded: bool,
}

/// Messages of a tree view. Forward them to [`TreeState::update`].
#[derive(Debug, Clone)]
pub enum TreeMsg<Id> {
    /// Expand or collapse the node (chevron click).
    Toggle(Id),
    /// Select the node (click).
    Select(Id),
    /// Double-click (or Enter): open the node.
    Activate(Id),
    /// A navigation key pressed in the focused tree.
    Key(KeyEvent),
}

/// What happened, for the app.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum TreeEvent<Id> {
    /// The selection moved to this node.
    Selected(Id),
    /// Enter or double-click on a node without children.
    Activated(Id),
    /// The node was expanded.
    Expanded(Id),
    /// The node was collapsed.
    Collapsed(Id),
}

/// Expansion, selection and the visible rows of a tree view.
pub struct TreeState<Id> {
    id: String,
    expanded: HashSet<Id>,
    selected: Option<Id>,
    rows: RefCell<Option<Rc<Vec<TreeRow<Id>>>>>,
    /// Type-ahead: the letters typed recently and when.
    typed: String,
    typed_at: Option<std::time::Instant>,
    /// Indentation per level, in px.
    pub indent: f32,
}

impl<Id: Clone> Clone for TreeState<Id> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            expanded: self.expanded.clone(),
            selected: self.selected.clone(),
            rows: RefCell::new(self.rows.borrow().clone()),
            typed: self.typed.clone(),
            typed_at: self.typed_at,
            indent: self.indent,
        }
    }
}

/// Rows moved by PageUp / PageDown.
const PAGE: usize = 20;

impl<Id: Clone + Eq + Hash + 'static> TreeState<Id> {
    /// `id` is the tree's element id (for focus and scrolling).
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            expanded: HashSet::new(),
            selected: None,
            rows: RefCell::new(None),
            typed: String::new(),
            typed_at: None,
            indent: 14.0,
        }
    }

    /// The tree's element id (e.g. for [`Cx::focus`]).
    pub fn element_id(&self) -> &str {
        &self.id
    }

    /// Whether node `id` is expanded.
    pub fn is_expanded(&self, id: &Id) -> bool {
        self.expanded.contains(id)
    }

    /// Expand node `id` (its children become visible).
    pub fn expand(&mut self, id: Id) {
        if self.expanded.insert(id) {
            self.refresh();
        }
    }

    /// Collapse node `id`.
    pub fn collapse(&mut self, id: &Id) {
        if self.expanded.remove(id) {
            self.refresh();
        }
    }

    /// The selected node.
    pub fn selected(&self) -> Option<&Id> {
        self.selected.as_ref()
    }

    /// Set the selected node (`None` clears it); sends no event.
    pub fn select(&mut self, id: Option<Id>) {
        self.selected = id;
    }

    /// Rebuild the visible rows on the next use: call it after the data
    /// changed (children added, removed or renamed).
    pub fn refresh(&mut self) {
        self.rows.borrow_mut().take();
    }

    /// The visible rows, in order (built on first use after a change).
    pub fn rows<T: TreeModel<Id = Id>>(&self, model: &T) -> Rc<Vec<TreeRow<Id>>> {
        if let Some(r) = self.rows.borrow().as_ref() {
            return r.clone();
        }
        let rows = Rc::new(self.flatten(model));
        *self.rows.borrow_mut() = Some(rows.clone());
        rows
    }

    fn flatten<T: TreeModel<Id = Id>>(&self, model: &T) -> Vec<TreeRow<Id>> {
        let mut out = Vec::new();
        // Depth-first without recursion (deep trees can't overflow the stack).
        let mut stack: Vec<(Id, usize, Option<usize>)> =
            model.children(None).into_iter().rev().map(|id| (id, 0, None)).collect();
        while let Some((id, depth, parent)) = stack.pop() {
            let has_children = model.has_children(&id);
            let expanded = has_children && self.expanded.contains(&id);
            let index = out.len();
            if expanded {
                for c in model.children(Some(&id)).into_iter().rev() {
                    stack.push((c, depth + 1, Some(index)));
                }
            }
            out.push(TreeRow { id, depth, parent, has_children, expanded });
        }
        out
    }

    /// The row index of a visible node.
    pub fn index_of<T: TreeModel<Id = Id>>(&self, model: &T, id: &Id) -> Option<usize> {
        self.rows(model).iter().position(|r| &r.id == id)
    }

    fn set_expanded(&mut self, id: &Id, on: bool) -> Option<TreeEvent<Id>> {
        if on {
            self.expanded.insert(id.clone()).then(|| TreeEvent::Expanded(id.clone()))
        } else {
            self.expanded.remove(id).then(|| TreeEvent::Collapsed(id.clone()))
        }
        .inspect(|_| self.refresh())
    }

    /// Select row `i`, scrolling it into view.
    fn select_row<M: 'static>(&mut self, rows: &[TreeRow<Id>], i: usize, cx: &mut Cx<M>) -> Option<TreeEvent<Id>> {
        let r = rows.get(i)?;
        cx.scroll_item_into_view(&self.id, i);
        if self.selected.as_ref() == Some(&r.id) {
            return None;
        }
        self.selected = Some(r.id.clone());
        Some(TreeEvent::Selected(r.id.clone()))
    }

    /// Apply a message.
    pub fn update<T: TreeModel<Id = Id>, M: 'static>(
        &mut self,
        msg: TreeMsg<Id>,
        model: &T,
        cx: &mut Cx<M>,
    ) -> Option<TreeEvent<Id>> {
        match msg {
            TreeMsg::Toggle(id) => {
                let on = !self.expanded.contains(&id);
                self.set_expanded(&id, on)
            }
            TreeMsg::Select(id) => {
                if self.selected.as_ref() == Some(&id) {
                    return None;
                }
                self.selected = Some(id.clone());
                Some(TreeEvent::Selected(id))
            }
            TreeMsg::Activate(id) => {
                self.selected = Some(id.clone());
                if model.has_children(&id) {
                    let on = !self.expanded.contains(&id);
                    self.set_expanded(&id, on)
                } else {
                    Some(TreeEvent::Activated(id))
                }
            }
            TreeMsg::Key(k) => self.key(&k, model, cx),
        }
    }

    fn key<T: TreeModel<Id = Id>, M: 'static>(
        &mut self,
        k: &KeyEvent,
        model: &T,
        cx: &mut Cx<M>,
    ) -> Option<TreeEvent<Id>> {
        let rows = self.rows(model);
        if rows.is_empty() {
            return None;
        }
        let cur = self.selected.as_ref().and_then(|s| rows.iter().position(|r| &r.id == s));
        let last = rows.len() - 1;
        let row = cur.map(|i| &rows[i]);
        match &k.key {
            Key::Down => self.select_row(&rows, cur.map_or(0, |i| (i + 1).min(last)), cx),
            Key::Up => self.select_row(&rows, cur.map_or(0, |i| i.saturating_sub(1)), cx),
            Key::Home => self.select_row(&rows, 0, cx),
            Key::End => self.select_row(&rows, last, cx),
            Key::PageDown => self.select_row(&rows, cur.map_or(0, |i| (i + PAGE).min(last)), cx),
            Key::PageUp => self.select_row(&rows, cur.map_or(0, |i| i.saturating_sub(PAGE)), cx),
            Key::Right => {
                let (i, r) = (cur?, row?);
                match (r.has_children, r.expanded) {
                    (true, false) => self.set_expanded(&r.id.clone(), true),
                    (true, true) if rows.get(i + 1).is_some_and(|c| c.parent == Some(i)) => {
                        self.select_row(&rows, i + 1, cx)
                    }
                    _ => None,
                }
            }
            Key::Left => {
                let r = row?;
                if r.expanded {
                    self.set_expanded(&r.id.clone(), false)
                } else {
                    let p = r.parent?;
                    self.select_row(&rows, p, cx)
                }
            }
            Key::Enter => {
                let r = row?;
                if r.has_children {
                    self.set_expanded(&r.id.clone(), !r.expanded)
                } else {
                    Some(TreeEvent::Activated(r.id.clone()))
                }
            }
            Key::Space => {
                let r = row?;
                r.has_children.then(|| r.id.clone()).and_then(|id| self.set_expanded(&id, !r.expanded))
            }
            Key::Char(c) if !k.mods.ctrl && !k.mods.meta && !k.mods.alt => {
                // Type-ahead: letters typed within a second form a prefix.
                let now = std::time::Instant::now();
                if self.typed_at.is_none_or(|t| now.duration_since(t).as_secs_f32() > 1.0) {
                    self.typed.clear();
                }
                self.typed_at = Some(now);
                self.typed.extend(c.to_lowercase());
                // Search from the current row (from the next one for a new
                // single letter, so repeating it cycles through matches).
                let start = match cur {
                    Some(i) if self.typed.chars().count() == 1 => i + 1,
                    Some(i) => i,
                    None => 0,
                };
                let n = rows.len();
                let hit = (0..n)
                    .map(|j| (start + j) % n)
                    .find(|&j| model.label(&rows[j].id).to_lowercase().starts_with(&self.typed))?;
                self.select_row(&rows, hit, cx)
            }
            _ => None,
        }
    }

    /// The tree view: a focusable, virtualized list of the visible rows.
    pub fn view<T: TreeModel<Id = Id>, M: Clone + 'static>(
        &self,
        model: &Rc<T>,
        map: impl Fn(TreeMsg<Id>) -> M + 'static,
    ) -> Element<M> {
        let th = theme();
        let rows = self.rows(&**model);
        let map: Rc<dyn Fn(TreeMsg<Id>) -> M> = Rc::new(map);
        let (model, selected, indent) = (model.clone(), self.selected.clone(), self.indent);
        let row_h = th.row_height;
        let (m, rows2) = (map.clone(), rows.clone());
        let list = virtual_list(rows.len(), move |i| {
            let r = &rows2[i];
            tree_row_view(&*model, r, selected.as_ref() == Some(&r.id), indent, &m)
        })
        .item_height(row_h);
        let km = map.clone();
        list.id(&self.id)
            .grow(1.0)
            .min_h(0.0)
            .py(4.0)
            .focusable()
            .role(Role::Tree)
            .on_key(move |k| {
                let nav = matches!(
                    k.key,
                    Key::Up
                        | Key::Down
                        | Key::Left
                        | Key::Right
                        | Key::Home
                        | Key::End
                        | Key::PageUp
                        | Key::PageDown
                        | Key::Enter
                        | Key::Space
                ) || matches!(k.key, Key::Char(_)) && !k.mods.ctrl && !k.mods.meta && !k.mods.alt;
                nav.then(|| km(TreeMsg::Key(k.clone())))
            })
            .class("tree")
    }
}

/// One row: indent guides, a chevron that toggles, icon, label.
fn tree_row_view<T: TreeModel, M: Clone + 'static>(
    model: &T,
    r: &TreeRow<T::Id>,
    selected: bool,
    indent: f32,
    map: &Rc<dyn Fn(TreeMsg<T::Id>) -> M>,
) -> Element<M> {
    let th = theme();
    let c = &th.colors;
    let label = model.label(&r.id);
    let mut row_el = row()
        .items_center()
        .h(th.row_height)
        .pl(8.0 + r.depth as f32 * indent)
        .pr(8.0)
        .gap(6.0)
        .mx(6.0)
        .rounded(th.radius_sm)
        .shrink(0.0)
        .color(if selected { c.text } else { c.text_muted })
        .hover(|s| s.bg(if selected { c.accent_soft } else { c.hover }).color(c.text))
        .cursor(Cursor::Default)
        .role(Role::TreeItem)
        .aria_selected(selected)
        .aria_label(label.clone())
        .on_click(map(TreeMsg::Select(r.id.clone())))
        .on_double_click(map(TreeMsg::Activate(r.id.clone())))
        .class("tree-row");
    if r.has_children {
        row_el = row_el.aria_expanded(r.expanded);
    }
    if selected {
        row_el = row_el.bg(c.accent_soft).class("tree-row-selected");
    }
    // Indent guides, one per ancestor level.
    for d in 0..r.depth {
        row_el = row_el.child(
            div().absolute().top(0.0).bottom(0.0).left(8.0 + d as f32 * indent + 6.5).w(1.0).bg(c.border).aria_hidden(),
        );
    }
    row_el = if r.has_children {
        row_el.child(
            div().center().square(14.0).shrink(0.0).on_click(map(TreeMsg::Toggle(r.id.clone()))).aria_hidden().child(
                icon(if r.expanded { Icon::ChevronDown } else { Icon::ChevronRight })
                    .font_size(14.0)
                    .color(c.text_faint),
            ),
        )
    } else {
        row_el.child(div().w(14.0).shrink(0.0))
    };
    if let Some(i) = model.icon(&r.id, r.expanded) {
        row_el = row_el.child(icon(i).font_size(15.0));
    }
    row_el.child(text(label).ellipsis().grow(1.0).min_w(0.0))
}
