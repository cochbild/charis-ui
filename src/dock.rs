//! Dockable, rearrangeable tabbed panels (VS Code / JetBrains style).
//!
//! A [`Dock`] owns a tree of splits and tab groups. Users can resize every
//! split, switch and close tabs, and drag a tab onto another group — onto its
//! center to join it, or onto an edge to split that group left/right/top/bottom.
//!
//! ```no_run
//! use charis_ui::prelude::*;
//! use charis_ui::dock::{Dock, DockMsg, DockNode};
//!
//! struct Ide { dock: Dock<&'static str> }
//! #[derive(Clone)] enum Msg { Dock(DockMsg) }
//!
//! impl App for Ide {
//!     type Msg = Msg;
//!     fn update(&mut self, msg: Msg, _: &mut Cx<Msg>) {
//!         match msg { Msg::Dock(m) => self.dock.update(m) }
//!     }
//!     fn view(&self) -> Element<Msg> {
//!         self.dock.view("dock", |t| t.to_string(), |t| text(format!("Content of {t}")), Msg::Dock)
//!     }
//! }
//!
//! let dock = Dock::new(DockNode::hsplit(vec![
//!     (1.0, DockNode::tabs(vec!["Explorer", "Search"])),
//!     (3.0, DockNode::vsplit(vec![
//!         (3.0, DockNode::tabs(vec!["main.rs", "lib.rs"])),
//!         (1.0, DockNode::tabs(vec!["Terminal"])),
//!     ])),
//! ]));
//! ```

use std::rc::Rc;

use crate::element::*;
use crate::geometry::{Axis, Point, Rect};
use crate::icons::Icon;
use crate::semantics::Role;
use crate::style::*;
use crate::theme::theme;
use crate::widgets::{backdrop, context_menu, menu_panel, MenuItem};

/// Identifies a tab group inside a [`Dock`].
pub type GroupId = u64;

/// Drag source marker for drags that started in another dock.
const REMOTE: GroupId = GroupId::MAX;

/// Drop target meaning "the dock's outer edge" (with a side zone): the tab
/// gets a full-height or full-width panel along that edge.
pub const DOCK_EDGE: GroupId = GroupId::MAX - 1;

/// Where a dragged tab will land relative to a group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DropZone {
    /// Join the group (appended).
    Center,
    /// Split the group, new tab on the left.
    Left,
    /// Split the group, new tab on the right.
    Right,
    /// Split the group, new tab above.
    Top,
    /// Split the group, new tab below.
    Bottom,
    /// Join the group at this tab position (reordering within a group).
    Insert(usize),
}

/// A node of the dock layout tree.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DockNode<T> {
    /// Children laid out along `axis` with flex weights.
    Split {
        /// Direction the children are laid out in.
        axis: Axis,
        /// Each child with its flex weight.
        children: Vec<(f32, DockNode<T>)>,
    },
    /// A leaf holding a group of tabs.
    Group(TabGroup<T>),
}

/// A stack of tabs, one of which is visible.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TabGroup<T> {
    /// Identifies the group in [`DockMsg`]s; assigned by the dock.
    pub id: GroupId,
    /// The group's tabs, in order.
    pub tabs: Vec<T>,
    /// Index of the visible tab.
    pub active: usize,
}

impl<T> DockNode<T> {
    /// A tab group.
    pub fn tabs(tabs: Vec<T>) -> Self {
        DockNode::Group(TabGroup { id: 0, tabs, active: 0 })
    }
    /// Children side by side, with flex weights.
    pub fn hsplit(children: Vec<(f32, DockNode<T>)>) -> Self {
        DockNode::Split { axis: Axis::Horizontal, children }
    }
    /// Children stacked vertically, with flex weights.
    pub fn vsplit(children: Vec<(f32, DockNode<T>)>) -> Self {
        DockNode::Split { axis: Axis::Vertical, children }
    }

    fn first_group(&self) -> GroupId {
        match self {
            DockNode::Group(g) => g.id,
            DockNode::Split { children, .. } => children.first().map(|c| c.1.first_group()).unwrap_or(0),
        }
    }

    fn group_mut(&mut self, id: GroupId) -> Option<&mut TabGroup<T>> {
        match self {
            DockNode::Group(g) => (g.id == id).then_some(g),
            DockNode::Split { children, .. } => children.iter_mut().find_map(|c| c.1.group_mut(id)),
        }
    }

    fn group(&self, id: GroupId) -> Option<&TabGroup<T>> {
        match self {
            DockNode::Group(g) => (g.id == id).then_some(g),
            DockNode::Split { children, .. } => children.iter().find_map(|c| c.1.group(id)),
        }
    }

    fn assign_ids(&mut self, next: &mut GroupId) {
        match self {
            DockNode::Group(g) => {
                *next += 1;
                g.id = *next;
            }
            DockNode::Split { children, .. } => children.iter_mut().for_each(|c| c.1.assign_ids(next)),
        }
    }

    fn count_groups(&self) -> usize {
        match self {
            DockNode::Group(_) => 1,
            DockNode::Split { children, .. } => children.iter().map(|c| c.1.count_groups()).sum(),
        }
    }

    /// Remove empty groups and flatten single-child splits.
    fn prune(self) -> Option<DockNode<T>> {
        match self {
            DockNode::Group(g) => (!g.tabs.is_empty()).then_some(DockNode::Group(g)),
            DockNode::Split { axis, children } => {
                let mut kept: Vec<(f32, DockNode<T>)> = Vec::new();
                for (w, c) in children {
                    match c.prune() {
                        // Merge nested splits along the same axis.
                        Some(DockNode::Split { axis: a, children: sub }) if a == axis => {
                            let total: f32 = sub.iter().map(|s| s.0).sum::<f32>().max(0.0001);
                            kept.extend(sub.into_iter().map(|(sw, sc)| (w * sw / total, sc)));
                        }
                        Some(c) => kept.push((w, c)),
                        None => {}
                    }
                }
                match kept.len() {
                    0 => None,
                    1 => kept.pop().map(|k| k.1),
                    _ => Some(DockNode::Split { axis, children: kept }),
                }
            }
        }
    }

    /// Split the group `target` by inserting `new` on the `zone` side.
    fn insert_beside(&mut self, target: GroupId, new: DockNode<T>, zone: DropZone) -> Result<(), DockNode<T>> {
        let axis = match zone {
            DropZone::Left | DropZone::Right => Axis::Horizontal,
            _ => Axis::Vertical,
        };
        let before = matches!(zone, DropZone::Left | DropZone::Top);
        match self {
            DockNode::Group(g) if g.id == target => {
                let old = std::mem::replace(self, DockNode::Split { axis, children: Vec::new() });
                let children = if before { vec![(1.0, new), (1.0, old)] } else { vec![(1.0, old), (1.0, new)] };
                *self = DockNode::Split { axis, children };
                Ok(())
            }
            DockNode::Group(_) => Err(new),
            DockNode::Split { axis: a, children } => {
                // Same axis: insert as a sibling, splitting the target's share.
                if *a == axis {
                    if let Some(pos) =
                        children.iter().position(|c| matches!(&c.1, DockNode::Group(g) if g.id == target))
                    {
                        let half = children[pos].0 / 2.0;
                        children[pos].0 = half;
                        let at = if before { pos } else { pos + 1 };
                        children.insert(at, (half, new));
                        return Ok(());
                    }
                }
                let mut new = new;
                for c in children.iter_mut() {
                    match c.1.insert_beside(target, new, zone) {
                        Ok(()) => return Ok(()),
                        Err(n) => new = n,
                    }
                }
                Err(new)
            }
        }
    }
}

/// Messages produced by a dock's UI. Forward them to [`Dock::update`].
#[derive(Debug, Clone)]
pub enum DockMsg {
    /// Show tab `usize` of the group.
    Activate(GroupId, usize),
    /// Close tab `usize` of the group.
    Close(GroupId, usize),
    /// A tab of the group is being dragged.
    Drag(GroupId, usize, DragEvent),
    /// A drag moved over a group body.
    Target(GroupId, DropEvent),
    /// A drag moved over a tab (reordering).
    TabTarget(GroupId, usize, DropEvent),
    /// Maximize a group to fill the dock, or restore it.
    ToggleMaximize(GroupId),
    /// The user resized a split (identified by its first group and axis).
    Resized(GroupId, Axis, Vec<f32>),
    /// Move the group's active tab into a new window ([`DockSpace`] only).
    PopOut(GroupId),
    /// Move the group's tabs back into the main window ([`DockSpace`] only).
    DockBack(GroupId),
    /// A drag moved over one of the dock-edge guides (zone: the side).
    EdgeTarget(DropZone, DropEvent),
    /// Open a tab's context menu, at a window position or (from the
    /// keyboard) under the tab.
    TabMenu(GroupId, usize, Option<Point>),
    /// Close the tab context menu.
    CloseMenu,
    /// Close every other tab of the group.
    CloseOthers(GroupId, usize),
    /// Close every tab of the group.
    CloseGroup(GroupId),
    /// Move a tab into a new group beside its own (zone: the side).
    Split(GroupId, usize, DropZone),
    /// Move a tab to a new panel along the dock's outer edge (zone: the side).
    MoveToEdge(GroupId, usize, DropZone),
    /// Move a tab into a new window ([`DockSpace`] only).
    FloatTab(GroupId, usize),
    /// Move a floating window's tab back into the main window ([`DockSpace`] only).
    DockBackTab(GroupId, usize),
}

#[derive(Debug, Clone)]
struct DockDrag {
    from: (GroupId, usize),
    pos: Point,
    over: Option<(GroupId, DropZone)>,
    /// The hovered group's rect (sizes the compass).
    over_rect: Rect,
}

#[cfg(feature = "serde")]
fn yes() -> bool {
    true
}

/// A dock layout: splits of tab groups that users can rearrange.
///
/// With the `serde` feature the whole dock (layout tree, split weights,
/// tabs and maximized state) can be saved and restored.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Dock<T> {
    /// Layout format version, for forward-compatible persistence.
    #[cfg_attr(feature = "serde", serde(default))]
    pub version: u32,
    root: Option<DockNode<T>>,
    next_id: GroupId,
    #[cfg_attr(feature = "serde", serde(skip))]
    drag: Option<DockDrag>,
    maximized: Option<GroupId>,
    /// Height of each group's tab strip (0 = use the theme's `tab_height`).
    pub tab_height: f32,
    /// Show close buttons on tabs.
    pub closable: bool,
    /// While dragging a tab, show a compass of drop targets in the hovered
    /// group and guides at the dock's outer edges (Visual Studio style).
    /// Edge bands work either way.
    #[cfg_attr(feature = "serde", serde(default = "yes"))]
    pub compass: bool,
    /// The open tab context menu.
    #[cfg_attr(feature = "serde", serde(skip))]
    menu: Option<(GroupId, usize, Option<Point>)>,
}

impl<T> Dock<T> {
    /// A dock with this layout; group ids are assigned here.
    ///
    /// ```
    /// use charis_ui::prelude::*;
    ///
    /// #[derive(Clone)]
    /// enum Msg {
    ///     Dock(DockMsg),
    /// }
    ///
    /// let dock = Dock::new(DockNode::hsplit(vec![
    ///     (1.0, DockNode::tabs(vec!["Files"])),
    ///     (3.0, DockNode::tabs(vec!["main.rs", "lib.rs"])),
    /// ]));
    /// let view: Element<Msg> = dock.view("dock", |t| t.to_string(), |t| text(*t), Msg::Dock);
    /// ```
    pub fn new(mut root: DockNode<T>) -> Self {
        let mut next = 0;
        root.assign_ids(&mut next);
        Self {
            version: 1,
            root: Some(root),
            next_id: next,
            drag: None,
            maximized: None,
            tab_height: 0.0,
            closable: true,
            compass: true,
            menu: None,
        }
    }

    /// The layout tree (for persistence or inspection).
    pub fn root(&self) -> Option<&DockNode<T>> {
        self.root.as_ref()
    }

    /// All groups' tabs, in layout order.
    pub fn groups(&self) -> Vec<&TabGroup<T>> {
        fn walk<'a, T>(n: &'a DockNode<T>, out: &mut Vec<&'a TabGroup<T>>) {
            match n {
                DockNode::Group(g) => out.push(g),
                DockNode::Split { children, .. } => children.iter().for_each(|c| walk(&c.1, out)),
            }
        }
        let mut out = Vec::new();
        if let Some(r) = &self.root {
            walk(r, &mut out);
        }
        out
    }

    /// Open a tab in the first group (or the group containing the active drag target).
    pub fn open(&mut self, tab: T) {
        match &mut self.root {
            Some(root) => {
                let id = root.first_group();
                if let Some(g) = root.group_mut(id) {
                    g.tabs.push(tab);
                    g.active = g.tabs.len() - 1;
                }
            }
            None => {
                self.next_id += 1;
                self.root = Some(DockNode::Group(TabGroup { id: self.next_id, tabs: vec![tab], active: 0 }));
            }
        }
    }

    /// The maximized group, if any.
    pub fn maximized(&self) -> Option<GroupId> {
        self.maximized
    }

    /// Whether a tab drag is in progress.
    pub fn dragging(&self) -> bool {
        self.drag.as_ref().is_some_and(|d| d.pos != Point::ZERO)
    }

    /// Apply a UI message.
    pub fn update(&mut self, msg: DockMsg) {
        // Any action closes the tab menu.
        if !matches!(msg, DockMsg::TabMenu(..)) {
            self.menu = None;
        }
        match msg {
            DockMsg::Activate(g, i) => {
                if let Some(g) = self.root.as_mut().and_then(|r| r.group_mut(g)) {
                    if i < g.tabs.len() {
                        g.active = i;
                    }
                }
            }
            DockMsg::Close(g, i) => {
                if let Some(grp) = self.root.as_mut().and_then(|r| r.group_mut(g)) {
                    if i < grp.tabs.len() {
                        grp.tabs.remove(i);
                        if grp.active >= grp.tabs.len() {
                            grp.active = grp.tabs.len().saturating_sub(1);
                        } else if i < grp.active {
                            grp.active -= 1;
                        }
                    }
                }
                self.prune();
            }
            DockMsg::Drag(g, i, ev) => match ev.phase {
                DragPhase::Start => {
                    self.drag = Some(DockDrag { from: (g, i), pos: ev.pos, over: None, over_rect: Rect::default() })
                }
                DragPhase::Move => {
                    if let Some(d) = &mut self.drag {
                        d.pos = ev.pos;
                    }
                }
                DragPhase::End => {
                    if let Some(d) = self.drag.take() {
                        if let Some((to, zone)) = d.over {
                            self.move_tab(d.from.0, d.from.1, to, zone);
                        }
                    }
                }
            },
            DockMsg::TabTarget(g, i, ev) => {
                if let Some(d) = &mut self.drag {
                    match ev.phase {
                        DropPhase::Over | DropPhase::Drop => {
                            let at = if ev.pos.x < ev.rect.center().x { i } else { i + 1 };
                            d.over = Some((g, DropZone::Insert(at)));
                        }
                        DropPhase::Leave => {
                            if d.over.is_some_and(|o| o.0 == g) {
                                d.over = None;
                            }
                        }
                    }
                }
            }
            DockMsg::PopOut(_) | DockMsg::DockBack(_) | DockMsg::FloatTab(..) | DockMsg::DockBackTab(..) => {}
            DockMsg::TabMenu(g, i, at) => self.menu = Some((g, i, at)),
            DockMsg::CloseMenu => {}
            DockMsg::CloseOthers(g, i) => self.close_others(g, i),
            DockMsg::CloseGroup(g) => self.close_group(g),
            DockMsg::Split(g, i, zone) => self.move_tab(g, i, g, zone),
            DockMsg::MoveToEdge(g, i, zone) => self.move_tab(g, i, DOCK_EDGE, zone),
            DockMsg::EdgeTarget(zone, ev) => {
                if let Some(d) = &mut self.drag {
                    match ev.phase {
                        DropPhase::Over | DropPhase::Drop => d.over = Some((DOCK_EDGE, zone)),
                        DropPhase::Leave => {
                            if d.over == Some((DOCK_EDGE, zone)) {
                                d.over = None;
                            }
                        }
                    }
                }
            }
            DockMsg::ToggleMaximize(g) => {
                self.maximized = if self.maximized == Some(g) { None } else { Some(g) };
            }
            DockMsg::Resized(g, axis, weights) => {
                fn find<T>(n: &mut DockNode<T>, g: GroupId, axis: Axis, w: &[f32]) -> bool {
                    match n {
                        DockNode::Split { axis: a, children } => {
                            if *a == axis && n_first(children) == g && children.len() == w.len() {
                                for (c, w) in children.iter_mut().zip(w) {
                                    c.0 = *w;
                                }
                                return true;
                            }
                            children.iter_mut().any(|c| find(&mut c.1, g, axis, w))
                        }
                        DockNode::Group(_) => false,
                    }
                }
                fn n_first<T>(c: &[(f32, DockNode<T>)]) -> GroupId {
                    c.first().map(|c| c.1.first_group()).unwrap_or(0)
                }
                if let Some(r) = &mut self.root {
                    find(r, g, axis, &weights);
                }
            }
            DockMsg::Target(g, ev) => {
                let tab_h = if self.tab_height > 0.0 { self.tab_height } else { theme().tab_height };
                let compass = self.compass;
                if let Some(d) = &mut self.drag {
                    match ev.phase {
                        DropPhase::Over | DropPhase::Drop => {
                            d.over = zone_for(ev.pos, ev.rect, tab_h, compass).map(|z| (g, z));
                            d.over_rect = ev.rect;
                        }
                        DropPhase::Leave => {
                            if d.over.is_some_and(|o| o.0 == g) {
                                d.over = None;
                            }
                        }
                    }
                }
            }
        }
    }

    fn prune(&mut self) {
        let keep_one = self.root.as_ref().map(|r| r.count_groups() == 1).unwrap_or(false);
        if !keep_one {
            // Keep the last group even when empty so there's a place to drop tabs.
            self.root = self.root.take().and_then(|r| r.prune());
        }
        self.fix_maximized();
    }

    fn fix_maximized(&mut self) {
        if let Some(m) = self.maximized {
            if self.root.as_ref().and_then(|r| r.group(m)).is_none_or(|g| g.tabs.is_empty()) {
                self.maximized = None;
            }
        }
    }

    /// Remove tab `index` of group `g` and return it (empty groups are
    /// pruned, except the last one).
    pub fn take_tab(&mut self, g: GroupId, index: usize) -> Option<T> {
        let grp = self.root.as_mut()?.group_mut(g)?;
        if index >= grp.tabs.len() {
            return None;
        }
        let t = grp.tabs.remove(index);
        if grp.active >= grp.tabs.len() {
            grp.active = grp.tabs.len().saturating_sub(1);
        } else if index < grp.active {
            grp.active -= 1;
        }
        self.prune();
        Some(t)
    }

    /// Insert a tab relative to group `g` (joining it, at a tab position, or
    /// splitting it at an edge). Opens it in the first group when `g` is gone.
    pub fn insert_tab(&mut self, tab: T, g: GroupId, zone: DropZone) {
        if g == DOCK_EDGE {
            self.insert_at_edge(tab, zone);
            self.prune();
            return;
        }
        let Some(root) = self.root.as_mut() else {
            self.open(tab);
            return;
        };
        match zone {
            DropZone::Center | DropZone::Insert(_) => match root.group_mut(g) {
                Some(grp) => {
                    let at = match zone {
                        DropZone::Insert(k) => k.min(grp.tabs.len()),
                        _ => grp.tabs.len(),
                    };
                    grp.tabs.insert(at, tab);
                    grp.active = at;
                }
                None => self.open(tab),
            },
            _ => {
                self.next_id += 1;
                let new = DockNode::Group(TabGroup { id: self.next_id, tabs: vec![tab], active: 0 });
                if let Err(DockNode::Group(grp)) = root.insert_beside(g, new, zone) {
                    for t in grp.tabs {
                        self.open(t);
                    }
                }
            }
        }
        self.prune();
    }

    /// Close every tab of group `g` except tab `keep`.
    pub fn close_others(&mut self, g: GroupId, keep: usize) {
        if let Some(grp) = self.root.as_mut().and_then(|r| r.group_mut(g)) {
            if keep < grp.tabs.len() {
                let t = grp.tabs.swap_remove(keep);
                grp.tabs = vec![t];
                grp.active = 0;
            }
        }
    }

    /// Close every tab of group `g` (the group goes away, unless it's the
    /// last one).
    pub fn close_group(&mut self, g: GroupId) {
        if let Some(grp) = self.root.as_mut().and_then(|r| r.group_mut(g)) {
            grp.tabs.clear();
            grp.active = 0;
        }
        self.prune();
    }

    /// Add a new group holding `tab` along the dock's outer edge (`zone`:
    /// the side), a quarter of the dock's size.
    fn insert_at_edge(&mut self, tab: T, zone: DropZone) {
        self.next_id += 1;
        let new = DockNode::Group(TabGroup { id: self.next_id, tabs: vec![tab], active: 0 });
        let axis = match zone {
            DropZone::Left | DropZone::Right => Axis::Horizontal,
            DropZone::Top | DropZone::Bottom => Axis::Vertical,
            _ => {
                self.root = Some(match self.root.take() {
                    None => new,
                    Some(old) => DockNode::Split { axis: Axis::Horizontal, children: vec![(3.0, old), (1.0, new)] },
                });
                return;
            }
        };
        let before = matches!(zone, DropZone::Left | DropZone::Top);
        self.root = Some(match self.root.take() {
            None => new,
            Some(DockNode::Split { axis: a, mut children }) if a == axis => {
                let w = children.iter().map(|c| c.0).sum::<f32>() / 3.0;
                if before {
                    children.insert(0, (w, new));
                } else {
                    children.push((w, new));
                }
                DockNode::Split { axis, children }
            }
            Some(old) => {
                let children = if before { vec![(1.0, new), (3.0, old)] } else { vec![(3.0, old), (1.0, new)] };
                DockNode::Split { axis, children }
            }
        });
    }

    /// The element id of group `n`'s active tab (in layout order, from 0),
    /// for keyboard commands like "Focus Panel 1": pass it to
    /// [`Cx::focus`](crate::Cx::focus). `id` is the one given to
    /// [`Dock::view`].
    pub fn focus_id(&self, id: &str, n: usize) -> Option<String> {
        let g = *self.groups().get(n)?;
        (!g.tabs.is_empty()).then(|| tab_id(id, g.id, g.active))
    }

    /// Is the dock empty (no tabs anywhere)?
    pub fn is_empty(&self) -> bool {
        self.groups().iter().all(|g| g.tabs.is_empty())
    }

    /// A tab from another dock is being dragged: show drop previews here.
    fn begin_remote_drag(&mut self) {
        if self.drag.is_none() {
            self.drag = Some(DockDrag {
                from: (REMOTE, 0),
                pos: Point::new(-1.0, -1.0),
                over: None,
                over_rect: Rect::default(),
            });
        }
    }

    /// End a remote drag; returns where the tab would land in this dock.
    fn end_remote_drag(&mut self) -> Option<(GroupId, DropZone)> {
        match &self.drag {
            Some(d) if d.from.0 == REMOTE => self.drag.take().and_then(|d| d.over),
            _ => None,
        }
    }

    /// Move tab `index` of group `from` to group `to` at `zone`.
    /// `to` can be [`DOCK_EDGE`] (with a side zone) for the dock's outer edge.
    pub fn move_tab(&mut self, from: GroupId, index: usize, to: GroupId, zone: DropZone) {
        if to == DOCK_EDGE {
            // Moving a lone group's only tab to the edge changes nothing.
            if self.groups().len() == 1 && self.groups()[0].tabs.len() == 1 {
                return;
            }
            let Some(g) = self.root.as_mut().and_then(|r| r.group_mut(from)) else { return };
            if index >= g.tabs.len() {
                return;
            }
            let t = g.tabs.remove(index);
            if g.active >= g.tabs.len() {
                g.active = g.tabs.len().saturating_sub(1);
            } else if index < g.active {
                g.active -= 1;
            }
            // Drop the emptied source first so the new panel spans the whole edge.
            self.root = self.root.take().and_then(|r| r.prune());
            self.insert_at_edge(t, zone);
            self.fix_maximized();
            return;
        }
        let Some(root) = self.root.as_mut() else { return };
        let Some(src) = root.group(from) else { return };
        if index >= src.tabs.len() {
            return;
        }
        if let DropZone::Insert(k) = zone {
            if from == to {
                let Some(g) = root.group_mut(from) else { return };
                let k = k.min(g.tabs.len());
                if k == index || k == index + 1 {
                    g.active = index;
                    return;
                }
                let t = g.tabs.remove(index);
                let at = if index < k { k - 1 } else { k };
                g.tabs.insert(at, t);
                g.active = at;
                return;
            }
        }
        if from == to && (zone == DropZone::Center || src.tabs.len() == 1) {
            if let Some(g) = root.group_mut(from) {
                g.active = index;
            }
            return;
        }
        let tab = {
            let Some(g) = root.group_mut(from) else { return };
            let t = g.tabs.remove(index);
            if g.active >= g.tabs.len() {
                g.active = g.tabs.len().saturating_sub(1);
            } else if index < g.active {
                g.active -= 1;
            }
            t
        };
        if let DropZone::Insert(k) = zone {
            if let Some(g) = root.group_mut(to) {
                let at = k.min(g.tabs.len());
                g.tabs.insert(at, tab);
                g.active = at;
            }
        } else if zone == DropZone::Center {
            if let Some(g) = root.group_mut(to) {
                g.tabs.push(tab);
                g.active = g.tabs.len() - 1;
            }
        } else {
            self.next_id += 1;
            let new = DockNode::Group(TabGroup { id: self.next_id, tabs: vec![tab], active: 0 });
            if let Err(DockNode::Group(g)) = root.insert_beside(to, new, zone) {
                // Target vanished: put the tab back.
                if let Some(src) = root.group_mut(from) {
                    src.tabs.extend(g.tabs);
                }
            }
        }
        // Remove the source group if it became empty.
        self.root = self.root.take().and_then(|r| r.prune());
        self.fix_maximized();
    }

    /// Render the dock.
    ///
    /// * `title` — the tab label for a tab value.
    /// * `content` — the panel content for a tab value.
    /// * `map` — wraps [`DockMsg`] into your app's message type.
    pub fn view<M: Clone + 'static>(
        &self,
        id: &str,
        title: impl Fn(&T) -> String,
        content: impl Fn(&T) -> Element<M>,
        map: impl Fn(DockMsg) -> M + 'static,
    ) -> Element<M> {
        self.view_with_icons(id, title, |_| None, content, map)
    }

    /// Like [`Dock::view`], with an icon per tab.
    pub fn view_with_icons<M: Clone + 'static>(
        &self,
        id: &str,
        title: impl Fn(&T) -> String,
        tab_icon: impl Fn(&T) -> Option<Icon>,
        content: impl Fn(&T) -> Element<M>,
        map: impl Fn(DockMsg) -> M + 'static,
    ) -> Element<M> {
        self.view_full(id, &title, &tab_icon, &content, Rc::new(map), None)
    }

    fn view_full<M: Clone + 'static>(
        &self,
        id: &str,
        title: &dyn Fn(&T) -> String,
        tab_icon: &dyn Fn(&T) -> Option<Icon>,
        content: &dyn Fn(&T) -> Element<M>,
        map: Rc<dyn Fn(DockMsg) -> M>,
        action: Option<HeaderAction>,
    ) -> Element<M> {
        let ctx = ViewCtx { id, title, icon: tab_icon, content, map, dock: self, action };
        let mut root = div().grow(1.0).min_w(0.0).min_h(0.0).flex_col();
        let maximized = self.maximized.and_then(|m| self.root.as_ref().and_then(|r| r.group(m)));
        root = match (&self.root, maximized) {
            (_, Some(g)) => root.child(ctx.group(g).grow(1.0)),
            (Some(r), None) => root.child(ctx.node(r).grow(1.0)),
            (None, None) => root,
        };
        // Tab menu opened with the pointer.
        if let Some((g, i, Some(at))) = self.menu {
            if let Some(items) = ctx.tab_menu(g, i) {
                root = root.child(context_menu(at, items, (ctx.map)(DockMsg::CloseMenu)));
            }
        }
        // Guides for docking along the dock's outer edges.
        let drag = self.drag.as_ref().filter(|d| d.pos != Point::ZERO);
        if let (Some(d), true, None) = (drag, self.compass && self.groups().len() > 1, maximized) {
            let th = theme();
            let c = &th.colors;
            let tab_h = if self.tab_height > 0.0 { self.tab_height } else { th.tab_height };
            if let Some((DOCK_EDGE, zone)) = d.over {
                let p = div()
                    .absolute()
                    .pointer_events(false)
                    .z_index(55)
                    .bg(c.accent.with_alpha(0.16))
                    .border(2.0, c.accent.with_alpha(0.8))
                    .rounded(th.radius)
                    .top(4.0)
                    .left(4.0)
                    .right(4.0)
                    .bottom(4.0);
                root = root.child(match zone {
                    DropZone::Left => p.right(pct(75.0)),
                    DropZone::Right => p.left(pct(75.0)),
                    DropZone::Top => p.bottom(pct(75.0)),
                    _ => p.top(pct(75.0)),
                });
            }
            for zone in [DropZone::Left, DropZone::Right, DropZone::Top, DropZone::Bottom] {
                let m = ctx.map.clone();
                let on = d.over == Some((DOCK_EDGE, zone));
                let button = compass_button(zone, on)
                    .id(&format!("{id}/edge-guide/{zone:?}"))
                    .pointer_events(true)
                    .on_drop_target(move |ev| m(DockMsg::EdgeTarget(zone, ev)));
                let lane = div().absolute().z_index(60).pointer_events(false).center();
                root = root.child(
                    match zone {
                        DropZone::Left => lane.left(6.0).top(0.0).bottom(0.0),
                        DropZone::Right => lane.right(6.0).top(0.0).bottom(0.0),
                        // Below the tab strips, which are drop targets too.
                        DropZone::Top => lane.top(tab_h + 6.0).left(0.0).right(0.0),
                        _ => lane.bottom(6.0).left(0.0).right(0.0),
                    }
                    .child(button),
                );
            }
        }
        // Drag ghost following the pointer.
        if let Some(d) = &self.drag {
            if d.pos != Point::ZERO {
                if let Some(t) = self.root.as_ref().and_then(|r| r.group(d.from.0)).and_then(|g| g.tabs.get(d.from.1)) {
                    let th = theme();
                    root = root.child(
                        row()
                            .fixed()
                            .left(d.pos.x + 14.0)
                            .top(d.pos.y + 10.0)
                            .z_index(300)
                            .pointer_events(false)
                            .items_center()
                            .gap(6.0)
                            .h(28.0)
                            .px(12.0)
                            .rounded(th.radius)
                            .bg(th.colors.elevated)
                            .border(1.0, th.colors.accent)
                            .shadows(th.shadow_popover.clone())
                            .color(th.colors.text)
                            .child(text(title(t)).nowrap()),
                    );
                }
            }
        }
        root
    }
}

/// Identifies a dock inside a [`DockSpace`]: `0` is the main window's.
pub type DockId = u64;

/// Messages of a [`DockSpace`]'s UI. Forward them to [`DockSpace::update`].
#[derive(Debug, Clone)]
pub enum DockSpaceMsg {
    /// A message from one of the docks.
    Dock(DockId, DockMsg),
    /// The user closed a floating window: its tabs move back to the main dock.
    CloseWindow(DockId),
}

/// A dock in its own window.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FloatingDock<T> {
    /// Identifies the window (see [`DockSpace::window_key`]).
    pub id: DockId,
    /// The dock shown in the window.
    pub dock: Dock<T>,
    /// Where the window opened (screen, logical px), if known.
    pub position: Option<(f32, f32)>,
    /// Window size (logical px).
    pub size: (f32, f32),
}

/// A main [`Dock`] plus docks in floating windows, with tabs that can be
/// dragged between all of them (JetBrains / VS Code style):
/// - drag a tab out of its window to open it in a new window there;
/// - drag a tab onto a group in another window to dock it there, with the
///   usual edge and center previews;
/// - or use the header buttons: "Open in new window" in the main window,
///   "Dock back" in floating ones (also for keyboard users, and Wayland,
///   where windows can't learn their screen position).
///
/// Closing a floating window docks its tabs back into the main window.
///
/// ```no_run
/// use charis_ui::prelude::*;
/// use charis_ui::dock::{Dock, DockNode, DockSpace, DockSpaceMsg};
///
/// struct Ide { dock: DockSpace<&'static str> }
/// #[derive(Clone)] enum Msg { Dock(DockSpaceMsg) }
///
/// impl App for Ide {
///     type Msg = Msg;
///     fn update(&mut self, msg: Msg, _: &mut Cx<Msg>) {
///         let Msg::Dock(m) = msg;
///         self.dock.update(m);
///     }
///     fn view(&self) -> Element<Msg> {
///         self.dock.view(None, |t| t.to_string(), |t| text(*t), Msg::Dock)
///     }
///     fn windows(&self) -> Vec<WindowSpec<Msg>> {
///         self.dock.windows(|t| t.to_string(), Msg::Dock)
///     }
///     fn window_view(&self, key: &str) -> Element<Msg> {
///         self.dock.view(Some(key), |t| t.to_string(), |t| text(*t), Msg::Dock)
///     }
/// }
/// # let _ = Ide { dock: DockSpace::new(Dock::new(DockNode::tabs(vec!["a"]))) };
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DockSpace<T> {
    /// The dock in the main window.
    pub main: Dock<T>,
    /// Docks in their own windows.
    pub floating: Vec<FloatingDock<T>>,
    next: DockId,
    /// Dock where the current tab drag started.
    #[cfg_attr(feature = "serde", serde(skip))]
    drag_from: Option<DockId>,
    /// Size of new floating windows.
    pub float_size: (f32, f32),
}

impl<T> DockSpace<T> {
    /// A dock space with `main` and no floating windows.
    pub fn new(main: Dock<T>) -> Self {
        Self { main, floating: Vec::new(), next: 0, drag_from: None, float_size: (640.0, 420.0) }
    }

    fn dock_mut(&mut self, id: DockId) -> Option<&mut Dock<T>> {
        if id == 0 {
            return Some(&mut self.main);
        }
        self.floating.iter_mut().find(|f| f.id == id).map(|f| &mut f.dock)
    }

    fn docks_mut(&mut self) -> impl Iterator<Item = (DockId, &mut Dock<T>)> {
        std::iter::once((0, &mut self.main)).chain(self.floating.iter_mut().map(|f| (f.id, &mut f.dock)))
    }

    /// The window key of floating dock `id` (see [`App::windows`](crate::App::windows)).
    pub fn window_key(id: DockId) -> String {
        format!("dock-{id}")
    }

    fn id_of_key(key: &str) -> Option<DockId> {
        key.strip_prefix("dock-")?.parse().ok()
    }

    /// Move a tab into a new floating window.
    pub fn float_tab(&mut self, from: DockId, g: GroupId, index: usize, position: Option<(f32, f32)>) {
        let Some(tab) = self.dock_mut(from).and_then(|d| d.take_tab(g, index)) else { return };
        self.next += 1;
        let mut dock = Dock::new(DockNode::tabs(vec![tab]));
        dock.closable = self.main.closable;
        dock.tab_height = self.main.tab_height;
        self.floating.push(FloatingDock { id: self.next, dock, position, size: self.float_size });
    }

    /// Move every floating window's tabs back into the main dock (the
    /// windows close).
    pub fn dock_all_back(&mut self) {
        let ids: Vec<DockId> = self.floating.iter().map(|f| f.id).collect();
        for id in ids {
            self.dock_back(id, None);
        }
        self.floating.retain(|f| !f.dock.is_empty());
    }

    /// The element id of the main dock's group `n`'s active tab (in layout
    /// order, from 0), for [`Cx::focus`](crate::Cx::focus).
    pub fn focus_id(&self, n: usize) -> Option<String> {
        self.main.focus_id("dockspace-0", n)
    }

    /// Open a tab: in the main dock's first group.
    pub fn open(&mut self, tab: T) {
        self.main.open(tab);
    }

    /// Move every tab of floating dock `id` (or just group `g`) into the main dock.
    fn dock_back(&mut self, id: DockId, g: Option<GroupId>) {
        let Some(f) = self.floating.iter_mut().find(|f| f.id == id) else { return };
        let groups: Vec<GroupId> = f.dock.groups().iter().map(|x| x.id).filter(|x| g.is_none_or(|g| g == *x)).collect();
        let mut tabs = Vec::new();
        for gid in groups {
            while let Some(t) = f.dock.take_tab(gid, 0) {
                tabs.push(t);
            }
        }
        for t in tabs {
            self.main.open(t);
        }
    }

    /// Apply a message from any of the docks' UIs.
    pub fn update(&mut self, msg: DockSpaceMsg) {
        // Any action closes the tab menus (one may be open in another window).
        if !matches!(msg, DockSpaceMsg::Dock(_, DockMsg::TabMenu(..))) {
            for (_, d) in self.docks_mut() {
                d.menu = None;
            }
        }
        match msg {
            DockSpaceMsg::CloseWindow(id) => self.dock_back(id, None),
            DockSpaceMsg::Dock(id, m) => match m {
                DockMsg::Drag(g, i, ev) => match ev.phase {
                    DragPhase::Start => {
                        self.drag_from = Some(id);
                        for (other, d) in self.docks_mut() {
                            if other != id {
                                d.begin_remote_drag();
                            }
                        }
                        if let Some(d) = self.dock_mut(id) {
                            d.update(DockMsg::Drag(g, i, ev));
                        }
                    }
                    DragPhase::Move => {
                        if let Some(d) = self.dock_mut(id) {
                            d.update(DockMsg::Drag(g, i, ev));
                        }
                    }
                    DragPhase::End => self.finish_drag(id, g, i, ev),
                },
                DockMsg::PopOut(g) => {
                    let active = self.dock_mut(id).and_then(|d| d.root.as_ref()?.group(g).map(|x| x.active));
                    if let Some(a) = active {
                        self.float_tab(id, g, a, None);
                    }
                }
                DockMsg::DockBack(g) => self.dock_back(id, Some(g)),
                DockMsg::FloatTab(g, i) => self.float_tab(id, g, i, None),
                DockMsg::DockBackTab(g, i) => {
                    if id != 0 {
                        if let Some(t) = self.dock_mut(id).and_then(|d| d.take_tab(g, i)) {
                            self.main.open(t);
                        }
                    }
                }
                other => {
                    if let Some(d) = self.dock_mut(id) {
                        d.update(other);
                    }
                }
            },
        }
        // Floating windows close once they have no tabs left.
        self.floating.retain(|f| !f.dock.is_empty());
    }

    fn finish_drag(&mut self, src: DockId, g: GroupId, i: usize, ev: DragEvent) {
        self.drag_from = None;
        // A drop onto another window's dock wins (its window delivered the
        // drop before this drag ended).
        let mut remote = None;
        for (id, d) in self.docks_mut() {
            if id != src {
                if let Some(o) = d.end_remote_drag() {
                    remote.get_or_insert((id, o));
                }
            }
        }
        let own_target = self.dock_mut(src).is_some_and(|d| d.drag.as_ref().is_some_and(|x| x.over.is_some()));
        if let Some((dst, (tg, zone))) = remote {
            let tab = self.dock_mut(src).and_then(|d| {
                d.drag = None;
                d.take_tab(g, i)
            });
            if let (Some(tab), Some(d)) = (tab, self.dock_mut(dst)) {
                d.insert_tab(tab, tg, zone);
            }
        } else if !own_target && ev.outside {
            if let Some(d) = self.dock_mut(src) {
                d.drag = None;
            }
            // A floating dock's only tab dragged out of its window: nothing to do.
            let alone = src != 0
                && self.dock_mut(src).is_some_and(|d| d.groups().iter().map(|x| x.tabs.len()).sum::<usize>() == 1);
            if !alone {
                let pos = ev.screen.map(|p| (p.x - 60.0, p.y - 14.0));
                self.float_tab(src, g, i, pos);
            }
        } else if let Some(d) = self.dock_mut(src) {
            d.update(DockMsg::Drag(g, i, ev));
        }
    }

    /// The floating windows to declare in [`App::windows`](crate::App::windows).
    pub fn windows<M>(
        &self,
        title: impl Fn(&T) -> String,
        map: impl Fn(DockSpaceMsg) -> M,
    ) -> Vec<crate::runtime::WindowSpec<M>> {
        self.floating
            .iter()
            .map(|f| {
                let name = f.dock.groups().iter().find_map(|g| g.tabs.get(g.active)).map(&title);
                let mut w = crate::runtime::WindowSpec::new(
                    Self::window_key(f.id),
                    name.unwrap_or_else(|| "Panel".into()),
                    map(DockSpaceMsg::CloseWindow(f.id)),
                )
                .size(f.size.0, f.size.1);
                if let Some((x, y)) = f.position {
                    w = w.at(x, y);
                }
                w
            })
            .collect()
    }

    /// Render the main dock (`window: None`) or a floating window's dock
    /// (`Some(key)` from [`App::window_view`](crate::App::window_view)).
    pub fn view<M: Clone + 'static>(
        &self,
        window: Option<&str>,
        title: impl Fn(&T) -> String,
        content: impl Fn(&T) -> Element<M>,
        map: impl Fn(DockSpaceMsg) -> M + 'static,
    ) -> Element<M> {
        self.view_with_icons(window, title, |_| None, content, map)
    }

    /// Like [`DockSpace::view`], with an icon per tab.
    pub fn view_with_icons<M: Clone + 'static>(
        &self,
        window: Option<&str>,
        title: impl Fn(&T) -> String,
        tab_icon: impl Fn(&T) -> Option<Icon>,
        content: impl Fn(&T) -> Element<M>,
        map: impl Fn(DockSpaceMsg) -> M + 'static,
    ) -> Element<M> {
        let id = window.and_then(Self::id_of_key).unwrap_or(0);
        let (dock, action) = match self.floating.iter().find(|f| f.id == id) {
            Some(f) => (
                &f.dock,
                HeaderAction {
                    icon: Icon::DockIn,
                    tip: "Dock back into the main window",
                    msg: DockMsg::DockBack,
                    tab_label: "Dock Back into Main Window",
                    tab_msg: DockMsg::DockBackTab,
                },
            ),
            None => (
                &self.main,
                HeaderAction {
                    icon: Icon::PopOut,
                    tip: "Open in new window",
                    msg: DockMsg::PopOut,
                    tab_label: "Open in New Window",
                    tab_msg: DockMsg::FloatTab,
                },
            ),
        };
        let map: Rc<dyn Fn(DockMsg) -> M> = Rc::new(move |m| map(DockSpaceMsg::Dock(id, m)));
        dock.view_full(&format!("dockspace-{id}"), &title, &tab_icon, &content, map, Some(action))
    }
}

/// A compass or edge-guide button: a little pane with the target part lit.
fn compass_button<M: 'static>(zone: DropZone, on: bool) -> Element<M> {
    let th = theme();
    let c = &th.colors;
    let fill = div().absolute().bg(if on { c.accent } else { c.accent.with_alpha(0.55) }).rounded(1.0);
    let fill = match zone {
        DropZone::Left => fill.top(0.0).bottom(0.0).left(0.0).w(pct(50.0)),
        DropZone::Right => fill.top(0.0).bottom(0.0).right(0.0).w(pct(50.0)),
        DropZone::Top => fill.left(0.0).right(0.0).top(0.0).h(pct(50.0)),
        DropZone::Bottom => fill.left(0.0).right(0.0).bottom(0.0).h(pct(50.0)),
        _ => fill.top(0.0).bottom(0.0).left(0.0).right(0.0),
    };
    div()
        .center()
        .square(COMPASS_B)
        .rounded(th.radius_sm)
        .bg(if on { c.accent_soft } else { c.elevated })
        .border(1.0, if on { c.accent } else { c.border_strong })
        .shadows(th.shadow_popover.clone())
        .aria_hidden()
        .child(div().w(20.0).h(15.0).rounded(2.0).border(1.5, c.text_muted).clip().child(fill))
}

fn tab_id(dock: &str, g: GroupId, i: usize) -> String {
    format!("{dock}/tab/{g}/{i}")
}

/// Compass button size and gap.
const COMPASS_B: f32 = 34.0;
const COMPASS_GAP: f32 = 4.0;

/// The compass's buttons for a group body, when it fits.
fn compass_buttons(body: Rect) -> Option<[(Rect, DropZone); 5]> {
    let span = 3.0 * COMPASS_B + 2.0 * COMPASS_GAP;
    if body.w < span + 16.0 || body.h < span + 16.0 {
        return None;
    }
    let c = body.center();
    let at = |dx: f32, dy: f32| {
        let step = COMPASS_B + COMPASS_GAP;
        Rect::new(c.x - COMPASS_B / 2.0 + dx * step, c.y - COMPASS_B / 2.0 + dy * step, COMPASS_B, COMPASS_B)
    };
    Some([
        (at(0.0, 0.0), DropZone::Center),
        (at(-1.0, 0.0), DropZone::Left),
        (at(1.0, 0.0), DropZone::Right),
        (at(0.0, -1.0), DropZone::Top),
        (at(0.0, 1.0), DropZone::Bottom),
    ])
}

fn body_rect(rect: Rect, tab_h: f32) -> Rect {
    Rect::new(rect.x, rect.y + tab_h, rect.w, (rect.h - tab_h).max(1.0))
}

/// Where a tab dropped at `pos` on a group lands: the compass button under
/// the pointer, else the edge band or center.
fn zone_for(pos: Point, rect: Rect, tab_h: f32, compass: bool) -> Option<DropZone> {
    let body = body_rect(rect, tab_h);
    if pos.y < body.y {
        return Some(DropZone::Center);
    }
    if compass {
        if let Some(b) = compass_buttons(body).and_then(|bs| bs.into_iter().find(|b| b.0.outset(2.0).contains(pos))) {
            return Some(b.1);
        }
    }
    let fx = (pos.x - body.x) / body.w.max(1.0);
    let fy = (pos.y - body.y) / body.h.max(1.0);
    let edge = 0.25;
    // Pick the closest edge within the edge band.
    let cands = [(fx, DropZone::Left), (1.0 - fx, DropZone::Right), (fy, DropZone::Top), (1.0 - fy, DropZone::Bottom)];
    let (d, z) = cands.into_iter().fold((f32::MAX, DropZone::Center), |acc, c| if c.0 < acc.0 { c } else { acc });
    Some(if d < edge { z } else { DropZone::Center })
}

/// An extra button in each group's header (pop out / dock back), and its
/// per-tab command in the tab menu.
#[derive(Clone)]
struct HeaderAction {
    icon: Icon,
    tip: &'static str,
    msg: fn(GroupId) -> DockMsg,
    tab_label: &'static str,
    tab_msg: fn(GroupId, usize) -> DockMsg,
}

struct ViewCtx<'a, T, M> {
    action: Option<HeaderAction>,
    id: &'a str,
    title: &'a dyn Fn(&T) -> String,
    icon: &'a dyn Fn(&T) -> Option<Icon>,
    content: &'a dyn Fn(&T) -> Element<M>,
    map: Rc<dyn Fn(DockMsg) -> M>,
    dock: &'a Dock<T>,
}

impl<T, M: Clone + 'static> ViewCtx<'_, T, M> {
    fn node(&self, n: &DockNode<T>) -> Element<M> {
        match n {
            DockNode::Split { axis, children } => {
                let panes = children.iter().map(|(w, c)| Pane::flex(*w, self.node(c)).min(90.0)).collect();
                let (m, g, ax) = (self.map.clone(), n.first_group(), *axis);
                split(&format!("{}/split/{}/{:?}", self.id, g, axis), *axis, panes)
                    .on_resize(move |w| m(DockMsg::Resized(g, ax, w)))
            }
            DockNode::Group(g) => self.group(g),
        }
    }

    fn group(&self, g: &TabGroup<T>) -> Element<M> {
        let th = theme();
        let c = th.colors.clone();
        let gid = g.id;
        let drag = self.dock.drag.as_ref().filter(|d| d.pos != Point::ZERO);
        let tab_h = if self.dock.tab_height > 0.0 { self.dock.tab_height } else { th.tab_height };
        let mut strip = row().h(tab_h).grow(1.0).min_w(0.0).scroll_x().items(Align::Stretch).role(Role::TabList);
        let insert_at = match drag.and_then(|d| d.over) {
            Some((og, DropZone::Insert(k))) if og == gid => Some(k),
            _ => None,
        };
        for (i, t) in g.tabs.iter().enumerate() {
            let active = i == g.active;
            let dragged = drag.is_some_and(|d| d.from == (gid, i));
            let m = self.map.clone();
            let mut tab = row()
                .id(&tab_id(self.id, gid, i))
                .items_center()
                .gap(7.0)
                .pl(12.0)
                .pr(if self.dock.closable { 6.0 } else { 12.0 })
                .shrink(0.0)
                .border_r(1.0, c.border)
                .color(if active { c.text } else { c.text_muted })
                .transition(0.1)
                .on_click((self.map)(DockMsg::Activate(gid, i)))
                .on_double_click((self.map)(DockMsg::ToggleMaximize(gid)))
                .on_drag(move |ev| m(DockMsg::Drag(gid, i, ev)))
                .cursor(Cursor::Default)
                .focusable()
                .role(Role::Tab)
                .aria_selected(active);
            // Tab commands: right-click, or Shift+F10 on the focused tab.
            let (m1, m2) = (self.map.clone(), self.map.clone());
            tab = tab
                .on_context_menu(move |p| m1(DockMsg::TabMenu(gid, i, Some(p))))
                .on_key(move |k| (k.key == Key::F(10) && k.mods.shift).then(|| m2(DockMsg::TabMenu(gid, i, None))));
            if self.dock.menu == Some((gid, i, None)) {
                if let Some(items) = self.tab_menu(gid, i) {
                    let esc = (self.map)(DockMsg::CloseMenu);
                    tab = tab.child(backdrop((self.map)(DockMsg::CloseMenu), false)).child(
                        menu_panel(items)
                            .absolute()
                            .top(pct(100.0))
                            .left(0.0)
                            .mt(2.0)
                            .z_index(100)
                            .on_key(move |k| (k.key == Key::Escape).then(|| esc.clone())),
                    );
                }
            }
            let m2 = self.map.clone();
            tab = tab.on_drop_target(move |ev| m2(DockMsg::TabTarget(gid, i, ev)));
            // Insertion marker while reordering.
            if insert_at == Some(i) {
                tab = tab.child(
                    div().absolute().top(4.0).bottom(4.0).left(-1.0).w(2.0).rounded(1.0).bg(c.accent).z_index(1),
                );
            } else if insert_at == Some(i + 1) && i + 1 == g.tabs.len() {
                tab = tab.child(
                    div().absolute().top(4.0).bottom(4.0).right(-1.0).w(2.0).rounded(1.0).bg(c.accent).z_index(1),
                );
            }
            if active {
                tab = tab.bg(c.surface).child(div().absolute().top(0.0).left(0.0).right(0.0).h(2.0).bg(c.accent));
                tab = tab.child(div().absolute().bottom(-1.0).left(0.0).right(0.0).h(1.0).bg(c.surface));
            } else {
                tab = tab.hover(|s| s.bg(c.hover).color(c.text));
            }
            if dragged {
                tab = tab.opacity(0.45);
            }
            if let Some(i) = (self.icon)(t) {
                tab = tab.child(icon(i).font_size(14.0).color(if active { c.accent } else { c.text_faint }));
            }
            tab = tab.child(text((self.title)(t)).nowrap());
            if self.dock.closable {
                tab = tab.child(
                    div()
                        .center()
                        .square(20.0)
                        .rounded(4.0)
                        .color(c.text_faint)
                        .transition(0.08)
                        .hover(|s| s.bg(c.pressed).color(c.text))
                        .on_click((self.map)(DockMsg::Close(gid, i)))
                        .child(icon(Icon::Close).font_size(13.0)),
                );
            }
            strip = strip.child(tab);
        }
        let body = match g.tabs.get(g.active) {
            Some(t) => (self.content)(t).key(("dock-content", gid, g.active)),
            None => col()
                .center()
                .gap(8.0)
                .color(c.text_faint)
                .child(icon(Icon::Layers).font_size(40.0).weight(Weight(300)))
                .child(text("Drop a tab here")),
        };
        let is_max = self.dock.maximized == Some(gid);
        let header =
            row().h(tab_h).shrink(0.0).bg(c.panel).border_b(1.0, c.border).items(Align::Stretch).child(strip).child(
                row()
                    .items_center()
                    .gap(2.0)
                    .px(4.0)
                    .children(self.action.clone().map(|a| {
                        div()
                            .center()
                            .square(24.0)
                            .rounded(th.radius_sm)
                            .color(c.text_faint)
                            .transition(0.1)
                            .hover(|s| s.bg(c.hover).color(c.text))
                            .tooltip(a.tip)
                            .role(crate::semantics::Role::Button)
                            .aria_label(a.tip)
                            .focusable()
                            .on_click((self.map)((a.msg)(gid)))
                            .child(icon(a.icon).font_size(13.0))
                    }))
                    .child(
                        div()
                            .center()
                            .square(24.0)
                            .rounded(th.radius_sm)
                            .color(c.text_faint)
                            .transition(0.1)
                            .hover(|s| s.bg(c.hover).color(c.text))
                            .tooltip(if is_max { "Restore panel" } else { "Maximize panel" })
                            .on_click((self.map)(DockMsg::ToggleMaximize(gid)))
                            .child(icon(if is_max { Icon::Restore } else { Icon::Maximize }).font_size(13.0)),
                    ),
            );
        let m = self.map.clone();
        let mut group = col()
            .key(("dock-group", gid))
            .bg(c.surface)
            .min_w(0.0)
            .min_h(0.0)
            .clip()
            .on_drop_target(move |ev| m(DockMsg::Target(gid, ev)))
            .child(header)
            .child(col().grow(1.0).min_h(0.0).min_w(0.0).child(body.grow(1.0).min_h(0.0)));
        // Drop preview
        if let Some((_, zone)) = drag.and_then(|d| d.over).filter(|o| o.0 == gid) {
            let th_ = tab_h;
            let mut ov = div()
                .absolute()
                .pointer_events(false)
                .bg(c.accent.with_alpha(0.16))
                .border(2.0, c.accent.with_alpha(0.8))
                .rounded(th.radius)
                .animate_layout(0.12)
                .top(th_ + 4.0)
                .left(4.0)
                .right(4.0)
                .bottom(4.0);
            ov = match zone {
                DropZone::Center => ov,
                DropZone::Left => ov.right(pct(50.0)),
                DropZone::Right => ov.left(pct(50.0)),
                DropZone::Top => ov.bottom(pct(50.0)),
                DropZone::Bottom => ov.top(pct(50.0)),
                DropZone::Insert(_) => ov.opacity(0.0),
            };
            group = group.child(ov);
        }
        // Compass: precise targets for joining or splitting this group.
        if let Some(d) = drag.filter(|d| self.dock.compass && d.over.is_some_and(|o| o.0 == gid)) {
            if compass_buttons(body_rect(d.over_rect, tab_h)).is_some() {
                let on = d.over.map(|o| o.1);
                let cell = |z: Option<DropZone>| match z {
                    Some(z) => compass_button(z, on == Some(z)),
                    None => div().square(COMPASS_B),
                };
                let line = |a, b, c| row().gap(COMPASS_GAP).child(cell(a)).child(cell(b)).child(cell(c));
                group = group.child(
                    div()
                        .absolute()
                        .top(tab_h)
                        .left(0.0)
                        .right(0.0)
                        .bottom(0.0)
                        .center()
                        .pointer_events(false)
                        .z_index(20)
                        .child(
                            col()
                                .gap(COMPASS_GAP)
                                .child(line(None, Some(DropZone::Top), None))
                                .child(line(Some(DropZone::Left), Some(DropZone::Center), Some(DropZone::Right)))
                                .child(line(None, Some(DropZone::Bottom), None)),
                        ),
                );
            }
        }
        group
    }

    /// The tab context menu's commands.
    fn tab_menu(&self, g: GroupId, i: usize) -> Option<Vec<MenuItem<M>>> {
        let grp = self.dock.root.as_ref()?.group(g)?;
        if i >= grp.tabs.len() {
            return None;
        }
        let m = &self.map;
        let single = grp.tabs.len() == 1;
        let alone = single && self.dock.groups().len() == 1;
        let dis = |it: MenuItem<M>, d: bool| match it {
            MenuItem::Action { label, shortcut, icon, msg, .. } => {
                MenuItem::Action { label, shortcut, icon, msg, disabled: d }
            }
            other => other,
        };
        let mut items = Vec::new();
        if self.dock.closable {
            items.push(MenuItem::action("Close", m(DockMsg::Close(g, i))));
            items.push(dis(MenuItem::action("Close Others", m(DockMsg::CloseOthers(g, i))), single));
            items.push(MenuItem::action("Close All in Group", m(DockMsg::CloseGroup(g))));
            items.push(MenuItem::Separator);
        }
        items.push(dis(MenuItem::action("Split Right", m(DockMsg::Split(g, i, DropZone::Right))), single));
        items.push(dis(MenuItem::action("Split Down", m(DockMsg::Split(g, i, DropZone::Bottom))), single));
        let edge = |label: &str, z| dis(MenuItem::action(label, m(DockMsg::MoveToEdge(g, i, z))), alone);
        items.push(MenuItem::submenu(
            "Move to Edge",
            vec![
                edge("Left", DropZone::Left),
                edge("Right", DropZone::Right),
                edge("Top", DropZone::Top),
                edge("Bottom", DropZone::Bottom),
            ],
        ));
        if let Some(a) = &self.action {
            items.push(MenuItem::action(a.tab_label, m((a.tab_msg)(g, i))));
        }
        items.push(MenuItem::Separator);
        let max = self.dock.maximized == Some(g);
        items.push(MenuItem::action(
            if max { "Restore Group" } else { "Maximize Group" },
            m(DockMsg::ToggleMaximize(g)),
        ));
        Some(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Dock<&'static str> {
        Dock::new(DockNode::hsplit(vec![(1.0, DockNode::tabs(vec!["a", "b"])), (2.0, DockNode::tabs(vec!["c"]))]))
    }

    fn tabs(d: &Dock<&'static str>) -> Vec<Vec<&'static str>> {
        d.groups().iter().map(|g| g.tabs.clone()).collect()
    }

    #[test]
    fn move_to_center_and_prune() {
        let mut d = sample();
        let (g1, g2) = (d.groups()[0].id, d.groups()[1].id);
        d.move_tab(g2, 0, g1, DropZone::Center);
        assert_eq!(tabs(&d), vec![vec!["a", "b", "c"]]);
    }

    #[test]
    fn move_to_edge_splits() {
        let mut d = sample();
        let (g1, g2) = (d.groups()[0].id, d.groups()[1].id);
        d.move_tab(g1, 1, g2, DropZone::Bottom);
        assert_eq!(tabs(&d), vec![vec!["a"], vec!["c"], vec!["b"]]);
        match d.root().unwrap() {
            DockNode::Split { axis: Axis::Horizontal, children } => {
                assert!(matches!(children[1].1, DockNode::Split { axis: Axis::Vertical, .. }));
            }
            _ => panic!("unexpected layout"),
        }
        // Same-axis insertion becomes a sibling.
        let g1 = d.groups()[0].id;
        d.move_tab(g1, 0, d.groups()[1].id, DropZone::Top);
        assert_eq!(tabs(&d), vec![vec!["a"], vec!["c"], vec!["b"]]);
    }

    #[test]
    fn same_group_edge_with_single_tab_is_noop() {
        let mut d = sample();
        let g2 = d.groups()[1].id;
        d.move_tab(g2, 0, g2, DropZone::Left);
        assert_eq!(tabs(&d), vec![vec!["a", "b"], vec!["c"]]);
    }

    #[test]
    fn commands() {
        let mut d = Dock::new(DockNode::hsplit(vec![
            (1.0, DockNode::tabs(vec!["a", "b", "c"])),
            (2.0, DockNode::tabs(vec!["d"])),
        ]));
        let g1 = d.groups()[0].id;
        d.update(DockMsg::Split(g1, 1, DropZone::Right));
        assert_eq!(tabs(&d), vec![vec!["a", "c"], vec!["b"], vec!["d"]]);
        d.update(DockMsg::CloseOthers(g1, 1));
        assert_eq!(tabs(&d), vec![vec!["c"], vec!["b"], vec!["d"]]);
        // To the dock's bottom edge: a full-width panel under everything.
        let g3 = d.groups()[2].id;
        d.update(DockMsg::MoveToEdge(g3, 0, DropZone::Bottom));
        assert_eq!(tabs(&d), vec![vec!["c"], vec!["b"], vec!["d"]]);
        match d.root().unwrap() {
            DockNode::Split { axis: Axis::Vertical, children } => {
                assert!(matches!(children[0].1, DockNode::Split { axis: Axis::Horizontal, .. }));
                assert!(matches!(&children[1].1, DockNode::Group(g) if g.tabs == vec!["d"]));
                assert!((children[0].0 / children[1].0 - 3.0).abs() < 0.01, "a quarter of the height");
            }
            other => panic!("unexpected layout {other:?}"),
        }
        // Same axis as the root: a new first child.
        let g = d.groups()[1].id;
        d.update(DockMsg::MoveToEdge(g, 0, DropZone::Top));
        assert_eq!(tabs(&d), vec![vec!["b"], vec!["c"], vec!["d"]]);
        assert!(matches!(d.root().unwrap(), DockNode::Split { axis: Axis::Vertical, children } if children.len() == 3));
        let g = d.groups()[0].id;
        d.update(DockMsg::CloseGroup(g));
        assert_eq!(tabs(&d), vec![vec!["c"], vec!["d"]]);
        // Splitting a group's only tab does nothing.
        let g = d.groups()[0].id;
        d.update(DockMsg::Split(g, 0, DropZone::Left));
        assert_eq!(tabs(&d), vec![vec!["c"], vec!["d"]]);
    }

    #[test]
    fn close_last_tab_prunes_group() {
        let mut d = sample();
        let g2 = d.groups()[1].id;
        d.update(DockMsg::Close(g2, 0));
        assert_eq!(tabs(&d), vec![vec!["a", "b"]]);
        let g1 = d.groups()[0].id;
        d.update(DockMsg::Close(g1, 0));
        d.update(DockMsg::Close(g1, 0));
        assert_eq!(tabs(&d), vec![Vec::<&str>::new()], "last group stays as a drop target");
    }

    #[test]
    fn zones() {
        let r = Rect::new(0.0, 0.0, 400.0, 334.0);
        let z = |x, y, compass| zone_for(Point::new(x, y), r, 34.0, compass).unwrap();
        for compass in [false, true] {
            assert_eq!(z(200.0, 10.0, compass), DropZone::Center);
            assert_eq!(z(20.0, 180.0, compass), DropZone::Left);
            assert_eq!(z(390.0, 180.0, compass), DropZone::Right);
            assert_eq!(z(200.0, 330.0, compass), DropZone::Bottom);
            assert_eq!(z(200.0, 50.0, compass), DropZone::Top);
            assert_eq!(z(200.0, 184.0, compass), DropZone::Center);
        }
        // Compass buttons around the body's center (200, 184), 38 px apart.
        assert_eq!(z(162.0, 184.0, true), DropZone::Left);
        assert_eq!(z(238.0, 184.0, true), DropZone::Right);
        assert_eq!(z(200.0, 146.0, true), DropZone::Top);
        assert_eq!(z(200.0, 222.0, true), DropZone::Bottom);
        assert_eq!(z(162.0, 184.0, false), DropZone::Center, "no compass: the middle joins");
        // Too small for a compass: edge bands only.
        let small = Rect::new(0.0, 0.0, 120.0, 150.0);
        assert!(compass_buttons(body_rect(small, 34.0)).is_none());
    }
}
