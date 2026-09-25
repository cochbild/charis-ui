//! Dockable, rearrangeable tabbed panels (VS Code / JetBrains style).
//!
//! A [`Dock`] owns a tree of splits and tab groups. Users can resize every
//! split, switch and close tabs, and drag a tab onto another group — onto its
//! center to join it, or onto an edge to split that group left/right/top/bottom.
//!
//! ```no_run
//! use rust_ui::prelude::*;
//! use rust_ui::dock::{Dock, DockMsg, DockNode};
//!
//! struct Ide { dock: Dock<&'static str> }
//! #[derive(Clone)] enum Msg { Dock(DockMsg) }
//!
//! impl App for Ide {
//!     type Msg = Msg;
//!     fn update(&mut self, msg: Msg, _: &mut Cx) {
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
use crate::style::*;
use crate::theme::theme;

/// Identifies a tab group inside a [`Dock`].
pub type GroupId = u64;

/// Where a dragged tab will land relative to a group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropZone {
    Center,
    Left,
    Right,
    Top,
    Bottom,
}

/// A node of the dock layout tree.
#[derive(Debug, Clone)]
pub enum DockNode<T> {
    /// Children laid out along `axis` with flex weights.
    Split {
        axis: Axis,
        children: Vec<(f32, DockNode<T>)>,
    },
    Group(TabGroup<T>),
}

/// A stack of tabs, one of which is visible.
#[derive(Debug, Clone)]
pub struct TabGroup<T> {
    pub id: GroupId,
    pub tabs: Vec<T>,
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
                    1 => Some(kept.pop().unwrap().1),
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
    Activate(GroupId, usize),
    Close(GroupId, usize),
    Drag(GroupId, usize, DragEvent),
    Target(GroupId, DropEvent),
}

#[derive(Debug, Clone)]
struct DockDrag {
    from: (GroupId, usize),
    pos: Point,
    over: Option<(GroupId, DropZone)>,
}

/// A dock layout: splits of tab groups that users can rearrange.
#[derive(Debug, Clone)]
pub struct Dock<T> {
    root: Option<DockNode<T>>,
    next_id: GroupId,
    drag: Option<DockDrag>,
    /// Height of each group's tab strip.
    pub tab_height: f32,
    /// Show close buttons on tabs.
    pub closable: bool,
}

impl<T> Dock<T> {
    pub fn new(mut root: DockNode<T>) -> Self {
        let mut next = 0;
        root.assign_ids(&mut next);
        Self { root: Some(root), next_id: next, drag: None, tab_height: 34.0, closable: true }
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

    /// Whether a tab drag is in progress.
    pub fn dragging(&self) -> bool {
        self.drag.as_ref().is_some_and(|d| d.pos != Point::ZERO)
    }

    /// Apply a UI message.
    pub fn update(&mut self, msg: DockMsg) {
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
                DragPhase::Start => self.drag = Some(DockDrag { from: (g, i), pos: ev.pos, over: None }),
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
            DockMsg::Target(g, ev) => {
                let tab_h = self.tab_height;
                if let Some(d) = &mut self.drag {
                    match ev.phase {
                        DropPhase::Over | DropPhase::Drop => d.over = Some((g, zone_for(ev.pos, ev.rect, tab_h))),
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
        if keep_one {
            return; // Keep the last group even when empty so there's a place to drop tabs.
        }
        self.root = self.root.take().and_then(|r| r.prune());
    }

    /// Move tab `index` of group `from` to group `to` at `zone`.
    pub fn move_tab(&mut self, from: GroupId, index: usize, to: GroupId, zone: DropZone) {
        let Some(root) = self.root.as_mut() else { return };
        let Some(src) = root.group(from) else { return };
        if index >= src.tabs.len() {
            return;
        }
        if from == to && (zone == DropZone::Center || src.tabs.len() == 1) {
            if let Some(g) = root.group_mut(from) {
                g.active = index;
            }
            return;
        }
        let tab = {
            let g = root.group_mut(from).unwrap();
            let t = g.tabs.remove(index);
            if g.active >= g.tabs.len() {
                g.active = g.tabs.len().saturating_sub(1);
            } else if index < g.active {
                g.active -= 1;
            }
            t
        };
        if zone == DropZone::Center {
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
        let map: Rc<dyn Fn(DockMsg) -> M> = Rc::new(map);
        let ctx = ViewCtx { id, title: &title, icon: &tab_icon, content: &content, map, dock: self };
        let mut root = div().grow(1.0).min_w(0.0).min_h(0.0).flex_col();
        root = match &self.root {
            Some(r) => root.child(ctx.node(r).grow(1.0)),
            None => root,
        };
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

fn zone_for(pos: Point, rect: Rect, tab_h: f32) -> DropZone {
    let body = Rect::new(rect.x, rect.y + tab_h, rect.w, (rect.h - tab_h).max(1.0));
    if pos.y < body.y {
        return DropZone::Center;
    }
    let fx = (pos.x - body.x) / body.w.max(1.0);
    let fy = (pos.y - body.y) / body.h.max(1.0);
    let edge = 0.25;
    // Pick the closest edge within the edge band.
    let cands = [(fx, DropZone::Left), (1.0 - fx, DropZone::Right), (fy, DropZone::Top), (1.0 - fy, DropZone::Bottom)];
    let (d, z) = cands.into_iter().fold((f32::MAX, DropZone::Center), |acc, c| if c.0 < acc.0 { c } else { acc });
    if d < edge {
        z
    } else {
        DropZone::Center
    }
}

struct ViewCtx<'a, T, M> {
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
                split(&format!("{}/split/{}/{:?}", self.id, n.first_group(), axis), *axis, panes)
            }
            DockNode::Group(g) => self.group(g),
        }
    }

    fn group(&self, g: &TabGroup<T>) -> Element<M> {
        let th = theme();
        let c = th.colors.clone();
        let gid = g.id;
        let drag = self.dock.drag.as_ref().filter(|d| d.pos != Point::ZERO);
        let mut strip = row()
            .h(self.dock.tab_height)
            .shrink(0.0)
            .bg(c.panel)
            .border_b(1.0, c.border)
            .scroll_x()
            .items(Align::Stretch);
        for (i, t) in g.tabs.iter().enumerate() {
            let active = i == g.active;
            let dragged = drag.is_some_and(|d| d.from == (gid, i));
            let m = self.map.clone();
            let mut tab = row()
                .key(("dock-tab", gid, i))
                .items_center()
                .gap(7.0)
                .pl(12.0)
                .pr(if self.dock.closable { 6.0 } else { 12.0 })
                .shrink(0.0)
                .border_r(1.0, c.border)
                .color(if active { c.text } else { c.text_muted })
                .transition(0.1)
                .on_click((self.map)(DockMsg::Activate(gid, i)))
                .on_drag(move |ev| m(DockMsg::Drag(gid, i, ev)))
                .cursor(Cursor::Default);
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
        let m = self.map.clone();
        let mut group = col()
            .key(("dock-group", gid))
            .bg(c.surface)
            .min_w(0.0)
            .min_h(0.0)
            .clip()
            .on_drop_target(move |ev| m(DockMsg::Target(gid, ev)))
            .child(strip)
            .child(col().grow(1.0).min_h(0.0).min_w(0.0).child(body.grow(1.0).min_h(0.0)));
        // Drop preview
        if let Some((_, zone)) = drag.and_then(|d| d.over).filter(|o| o.0 == gid) {
            let th_ = self.dock.tab_height;
            let mut ov = div()
                .absolute()
                .pointer_events(false)
                .bg(c.accent.with_alpha(0.16))
                .border(2.0, c.accent.with_alpha(0.8))
                .rounded(th.radius)
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
            };
            group = group.child(ov);
        }
        group
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
        assert_eq!(zone_for(Point::new(200.0, 10.0), r, 34.0), DropZone::Center);
        assert_eq!(zone_for(Point::new(20.0, 180.0), r, 34.0), DropZone::Left);
        assert_eq!(zone_for(Point::new(390.0, 180.0), r, 34.0), DropZone::Right);
        assert_eq!(zone_for(Point::new(200.0, 330.0), r, 34.0), DropZone::Bottom);
        assert_eq!(zone_for(Point::new(200.0, 50.0), r, 34.0), DropZone::Top);
        assert_eq!(zone_for(Point::new(200.0, 180.0), r, 34.0), DropZone::Center);
    }
}
