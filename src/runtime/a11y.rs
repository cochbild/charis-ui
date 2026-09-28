//! Accessibility: an AccessKit tree built from the current frame, and
//! actions requested by assistive technology routed back into the runtime.
//!
//! Only meaningful nodes are exposed: text, controls, inputs, scroll areas
//! and anything with explicit [`Semantics`]. Layout-only containers are
//! transparent (their children attach to the nearest exposed ancestor), and
//! text inside a control (a button's label, a checkbox's text) becomes the
//! control's name instead of separate nodes, so it is read once.

use accesskit::{
    Action, ActionData, ActionRequest, Node as AkNode, NodeId, Rect as AkRect, Role as AkRole, Toggled, TreeId,
    TreeInfo, TreeUpdate,
};

use super::*;
use crate::fxhash::FxHashMap;
use crate::semantics::{Role, Semantics};

fn ak_role(r: Role) -> Option<AkRole> {
    Some(match r {
        Role::Button => AkRole::Button,
        Role::CheckBox => AkRole::CheckBox,
        Role::Switch => AkRole::Switch,
        Role::RadioButton => AkRole::RadioButton,
        Role::RadioGroup => AkRole::RadioGroup,
        Role::Tab => AkRole::Tab,
        Role::TabList => AkRole::TabList,
        Role::TabPanel => AkRole::TabPanel,
        Role::Menu => AkRole::Menu,
        Role::MenuBar => AkRole::MenuBar,
        Role::MenuItem => AkRole::MenuItem,
        Role::Link => AkRole::Link,
        Role::Heading => AkRole::Heading,
        Role::List => AkRole::List,
        Role::ListItem => AkRole::ListItem,
        Role::Tree => AkRole::Tree,
        Role::TreeItem => AkRole::TreeItem,
        Role::Table => AkRole::Table,
        Role::Row => AkRole::Row,
        Role::Cell => AkRole::Cell,
        Role::ColumnHeader => AkRole::ColumnHeader,
        Role::Dialog => AkRole::Dialog,
        Role::ProgressBar => AkRole::ProgressIndicator,
        Role::Image => AkRole::Image,
        Role::Toolbar => AkRole::Toolbar,
        Role::Status => AkRole::Status,
        Role::Navigation => AkRole::Navigation,
        Role::Group => AkRole::Group,
        // AccessKit has no separator role; separators are decoration.
        Role::Separator => return None,
        Role::Label => AkRole::Label,
        Role::ComboBox => AkRole::ComboBox,
        Role::Slider => AkRole::Slider,
        Role::TextInput => AkRole::TextInput,
        Role::ScrollView => AkRole::ScrollView,
    })
}

/// Roles whose text descendants are folded into their own name.
fn names_from_content(r: AkRole) -> bool {
    matches!(
        r,
        AkRole::Button
            | AkRole::CheckBox
            | AkRole::Switch
            | AkRole::RadioButton
            | AkRole::Tab
            | AkRole::MenuItem
            | AkRole::Link
            | AkRole::Heading
            | AkRole::ListItem
            | AkRole::TreeItem
            | AkRole::Cell
            | AkRole::ColumnHeader
            | AkRole::ComboBox
            | AkRole::ListBoxOption
    )
}

impl<A: App> Runtime<A> {
    /// The window's accessibility tree for the current frame (a full update).
    pub fn accessibility_tree(&mut self) -> TreeUpdate {
        if self.frame.nodes.is_empty() {
            self.render();
        }
        let root = self.frame.nodes[0].id;
        let mut out = Vec::new();
        let mut kids = Vec::new();
        for c in self.frame.nodes[0].children.clone() {
            self.ak_walk(c, &mut kids, &mut out, false);
        }
        let mut win = AkNode::new(AkRole::Window);
        win.set_children(kids);
        win.set_bounds(self.ak_rect(self.frame.nodes[0].rect));
        out.push((NodeId(root), win));
        let focus = self.focused.filter(|f| out.iter().any(|(id, _)| id.0 == *f)).unwrap_or(root);
        let mut tree = TreeInfo::new(NodeId(root));
        tree.toolkit_name = Some(env!("CARGO_PKG_NAME").to_string());
        tree.toolkit_version = Some(env!("CARGO_PKG_VERSION").to_string());
        TreeUpdate { nodes: out, tree: Some(tree), tree_id: TreeId::ROOT, focus: NodeId(focus) }
    }

    /// What changed in the accessibility tree since the last call: the
    /// nodes that are new or differ from what was sent before, and the focus.
    /// The first call (and the first after [`Runtime::reset_accessibility`])
    /// returns the full tree. The window sends these to screen readers, so
    /// they only process what changed.
    pub fn accessibility_update(&mut self) -> TreeUpdate {
        let full = self.accessibility_tree();
        if self.a11y_sent.is_empty() {
            self.a11y_sent = full.nodes.iter().map(|(id, n)| (id.0, n.clone())).collect();
            return full;
        }
        let mut nodes = Vec::new();
        let mut sent = FxHashMap::default();
        sent.reserve(full.nodes.len());
        for (id, node) in full.nodes {
            if self.a11y_sent.get(&id.0) != Some(&node) {
                nodes.push((id, node.clone()));
            }
            sent.insert(id.0, node);
        }
        self.a11y_sent = sent;
        TreeUpdate { nodes, tree: None, tree_id: full.tree_id, focus: full.focus }
    }

    /// Forget what was sent, so the next [`Runtime::accessibility_update`] is
    /// a full tree (a screen reader just started, or asked for the tree).
    pub fn reset_accessibility(&mut self) {
        self.a11y_sent.clear();
    }

    fn ak_rect(&self, r: Rect) -> AkRect {
        let s = self.scale as f64;
        AkRect { x0: r.x as f64 * s, y0: r.y as f64 * s, x1: (r.x + r.w) as f64 * s, y1: (r.y + r.h) as f64 * s }
    }

    /// The role a node is exposed with, or `None` for layout-only nodes.
    fn ak_role_of(&self, n: &Node<A::Msg>) -> Option<AkRole> {
        if let Some(r) = n.sem.as_ref().and_then(|s| s.role) {
            return ak_role(r);
        }
        match (&n.content, &n.behavior) {
            (NodeContent::Input(s), _) if s.password => Some(AkRole::PasswordInput),
            (NodeContent::Input(s), _) if s.multiline => Some(AkRole::MultilineTextInput),
            (NodeContent::Input(_), _) => Some(AkRole::TextInput),
            (_, Behavior::Slider { .. }) => Some(AkRole::Slider),
            (_, Behavior::Scroll { .. }) => Some(AkRole::ScrollView),
            (_, Behavior::DropdownToggle(_)) => Some(AkRole::ComboBox),
            (_, Behavior::DropdownPick(..)) => Some(AkRole::ListBoxOption),
            (_, Behavior::Splitter { .. }) => Some(AkRole::Splitter),
            (_, Behavior::WindowControl(_) | Behavior::Copy(_) | Behavior::ColumnResize { .. }) => Some(AkRole::Button),
            (NodeContent::Text(t), _) if !t.text.trim().is_empty() => Some(AkRole::Label),
            _ if n.handlers.click.is_some() || n.focusable => Some(AkRole::Button),
            _ => None,
        }
    }

    /// Text of a subtree, for naming controls.
    fn ak_text_of(&self, i: usize, out: &mut String) {
        let n = &self.frame.nodes[i];
        if n.sem.as_ref().is_some_and(|s| s.hidden) {
            return;
        }
        if let NodeContent::Text(t) = &n.content {
            let t = t.text.trim();
            if !t.is_empty() {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(t);
            }
        }
        for &c in &n.children {
            self.ak_text_of(c, out);
        }
    }

    fn ak_walk(&self, i: usize, siblings: &mut Vec<NodeId>, out: &mut Vec<(NodeId, AkNode)>, in_named: bool) {
        let n = &self.frame.nodes[i];
        let sem: Option<&Semantics> = n.sem.as_deref();
        if sem.is_some_and(|s| s.hidden) || n.style.opacity <= 0.01 {
            return;
        }
        let Some(role) = self.ak_role_of(n) else {
            for &c in &n.children {
                self.ak_walk(c, siblings, out, in_named);
            }
            return;
        };
        // Plain text inside a named control is already part of its name.
        if in_named && role == AkRole::Label && sem.is_none() {
            return;
        }
        let named = names_from_content(role);
        let mut node = AkNode::new(role);
        let mut kids = Vec::new();
        for &c in &n.children {
            self.ak_walk(c, &mut kids, out, in_named || named);
        }
        node.set_children(kids);
        let bounds = n.clip.map_or(n.rect, |c| n.rect.intersect(&c));
        node.set_bounds(self.ak_rect(bounds));

        // Name and description.
        let explicit = sem.and_then(|s| s.label.clone());
        match (&n.content, explicit) {
            (_, Some(l)) => node.set_label(l),
            (NodeContent::Text(t), None) if role == AkRole::Label => node.set_value(t.text.clone()),
            (NodeContent::Text(t), None) => node.set_label(t.text.clone()),
            (_, None) if named || role == AkRole::Button => {
                let mut t = String::new();
                self.ak_text_of(i, &mut t);
                match (t.is_empty(), &n.tooltip) {
                    (false, _) => node.set_label(t),
                    (true, Some(tip)) => node.set_label(tip.clone()),
                    (true, None) => {
                        if let Behavior::WindowControl(c) = n.behavior {
                            node.set_label(match c {
                                WindowControl::Minimize => "Minimize",
                                WindowControl::ToggleMaximize => "Maximize",
                                WindowControl::Close => "Close",
                            });
                        }
                    }
                }
            }
            _ => {}
        }
        if let Some(d) = sem.and_then(|s| s.description.clone()) {
            node.set_description(d);
        } else if let (Some(tip), false) = (&n.tooltip, node.label().is_some_and(|l| Some(l) == n.tooltip.as_deref())) {
            node.set_description(tip.clone());
        }

        // States.
        if let Some(s) = sem {
            if let Some(c) = s.checked {
                node.set_toggled(if c { Toggled::True } else { Toggled::False });
            }
            if let Some(v) = s.selected {
                node.set_selected(v);
            }
            if let Some(v) = s.expanded {
                node.set_expanded(v);
            }
            if let Some(l) = s.level {
                node.set_level(l as usize);
            }
            if let Some((v, lo, hi)) = s.value {
                node.set_numeric_value(v);
                node.set_min_numeric_value(lo);
                node.set_max_numeric_value(hi);
            }
            if let Some(u) = &s.url {
                node.set_url(u.clone());
            }
            if s.modal {
                node.set_modal();
            }
        }
        if n.disabled {
            node.set_disabled();
        }
        if n.focusable && !n.disabled {
            node.add_action(Action::Focus);
        }
        let clickable = n.handlers.click.is_some()
            || matches!(
                n.behavior,
                Behavior::WindowControl(_)
                    | Behavior::Copy(_)
                    | Behavior::DropdownToggle(_)
                    | Behavior::DropdownPick(..)
                    | Behavior::DropdownClose(_)
            );
        if clickable && !n.disabled {
            node.add_action(Action::Click);
        }
        match (&n.content, &n.behavior) {
            (NodeContent::Input(spec), _) => {
                if !spec.password {
                    let v = self.pending_values.get(&n.id).unwrap_or(&spec.value);
                    node.set_value(v.to_string());
                }
                if !spec.placeholder.is_empty() {
                    node.set_placeholder(spec.placeholder.clone());
                }
                node.add_action(Action::SetValue);
                node.add_action(Action::ReplaceSelectedText);
            }
            (_, Behavior::Slider { value, min, max, step }) => {
                node.set_numeric_value(*value as f64);
                node.set_min_numeric_value(*min as f64);
                node.set_max_numeric_value(*max as f64);
                let step = if *step > 0.0 { *step } else { (max - min) / 100.0 };
                node.set_numeric_value_step(step as f64);
                node.add_action(Action::Increment);
                node.add_action(Action::Decrement);
                node.add_action(Action::SetValue);
            }
            (_, Behavior::Scroll { y, .. }) => {
                node.set_clips_children();
                if *y {
                    node.set_scroll_y(n.scroll.y as f64);
                    node.set_scroll_y_min(0.0);
                    let max = self.scrolls.get(&n.id).map_or(0.0, |s| s.max.y);
                    node.set_scroll_y_max(max as f64);
                    node.add_action(Action::ScrollUp);
                    node.add_action(Action::ScrollDown);
                }
            }
            (_, Behavior::DropdownPick(d, idx)) => {
                if let Some(st) = self.dropdowns.get(d) {
                    node.set_selected(st.selected == Some(*idx));
                }
            }
            (_, Behavior::DropdownToggle(d)) => {
                if let Some(st) = self.dropdowns.get(d) {
                    node.set_expanded(st.open);
                    if let Some(label) = st.selected.and_then(|s| st.options.get(s)) {
                        node.set_value(label.clone());
                    }
                }
                node.add_action(Action::Expand);
                node.add_action(Action::Collapse);
            }
            _ => {}
        }
        out.push((NodeId(n.id), node));
        siblings.push(NodeId(n.id));
    }

    /// Perform an action requested by assistive technology.
    pub fn accessibility_action(&mut self, req: ActionRequest) {
        let id = req.target_node.0;
        let Some(&i) = self.frame.by_id.get(&id) else { return };
        let value = match &req.data {
            Some(ActionData::Value(v)) => Some(v.to_string()),
            _ => None,
        };
        match req.action {
            Action::Focus => {
                if self.frame.nodes[i].focusable {
                    self.focused = Some(id);
                    self.focus_visible = true;
                }
            }
            Action::Blur => {
                if self.focused == Some(id) {
                    self.focused = None;
                }
            }
            Action::Click => self.ak_click(i),
            Action::Expand | Action::Collapse => {
                let open = req.action == Action::Expand;
                if let Behavior::DropdownToggle(d) = self.frame.nodes[i].behavior {
                    if open {
                        self.focused = Some(id);
                        let st = self.dropdowns.entry(d).or_default();
                        st.open = true;
                        st.filter.clear();
                    } else {
                        self.close_dropdowns(None);
                    }
                } else {
                    self.ak_click(i);
                }
            }
            Action::SetValue | Action::ReplaceSelectedText => {
                let Some(v) = value else { return };
                let n = &self.frame.nodes[i];
                match (&n.content, &n.behavior) {
                    (NodeContent::Input(spec), _) => {
                        let current = self.pending_values.get(&id).cloned().unwrap_or_else(|| spec.value.clone());
                        let new = if req.action == Action::SetValue {
                            v
                        } else {
                            let mut sel = self.inputs.get(&id).map(|s| s.sel).unwrap_or_default();
                            sel.clamp(&current);
                            let (nv, s) = edit::replace(&current, sel, &v);
                            self.inputs.entry(id).or_default().sel = s;
                            nv
                        };
                        if let Some(h) = self.frame.nodes[i].handlers.input.clone() {
                            self.pending_values.insert(id, std::rc::Rc::from(new.as_str()));
                            self.queue.push(h(new));
                        }
                    }
                    (_, Behavior::Slider { .. }) => {
                        if let Ok(x) = v.trim().parse::<f32>() {
                            self.ak_set_slider(i, x);
                        }
                    }
                    _ => {}
                }
            }
            Action::Increment | Action::Decrement => {
                if let Behavior::Slider { value, min, max, step } = self.frame.nodes[i].behavior {
                    let step = if step > 0.0 { step } else { (max - min) / 100.0 };
                    let dir = if req.action == Action::Increment { 1.0 } else { -1.0 };
                    self.ak_set_slider(i, value + dir * step);
                }
            }
            Action::ScrollDown | Action::ScrollUp => {
                let n = &self.frame.nodes[i];
                let page = (n.rect.h * 0.8).max(40.0);
                let d = if req.action == Action::ScrollDown { page } else { -page };
                let now = self.now;
                let st = self.scrolls.entry(id).or_default();
                let a = st.y.get_or_insert(Anim::new(0.0));
                let t = (a.target() + d).clamp(0.0, st.max.y);
                a.set(t, now, 0.18);
                if st.pinned.is_some() {
                    st.pinned = Some(t >= st.max.y - 2.0);
                }
            }
            Action::ScrollIntoView => self.ak_scroll_into_view(i),
            _ => {}
        }
        self.dirty = true;
        self.flush();
    }

    fn ak_set_slider(&mut self, i: usize, x: f32) {
        let n = &self.frame.nodes[i];
        if let (Behavior::Slider { min, max, step, .. }, Some(h)) = (&n.behavior, n.handlers.value.clone()) {
            let mut v = x.clamp(*min, *max);
            if *step > 0.0 {
                v = min + ((v - min) / step).round() * step;
            }
            self.queue.push(h(v.clamp(*min, *max)));
        }
    }

    /// Activate a node like a mouse click. When the node is visible, a real
    /// click at its center runs every behavior; otherwise its click handler
    /// (or the nearest ancestor's) fires directly.
    fn ak_click(&mut self, i: usize) {
        let n = &self.frame.nodes[i];
        let id = n.id;
        let rect = n.clip.map_or(n.rect, |c| n.rect.intersect(&c));
        let center = rect.center();
        let hits_it = rect.w > 0.0 && rect.h > 0.0 && self.hit(center).is_some_and(|h| self.chain(h).contains(&i));
        if hits_it {
            let saved = self.pointer;
            self.pointer_down(center, MouseButton::Left);
            self.pointer_up(center, MouseButton::Left);
            // A synthetic click shouldn't move the hover state.
            self.pointer = saved;
            if let Some(p) = saved {
                self.update_hover(p);
            }
            return;
        }
        if let Some(h) = self.find_up(id, |n| n.handlers.click.is_some()).and_then(|f| self.node_by_id(f)) {
            if let Some(m) = h.handlers.click.clone() {
                self.queue.push(m);
            }
        }
    }

    /// Scroll every scroll container around a node so the node is visible.
    fn ak_scroll_into_view(&mut self, i: usize) {
        let target = self.frame.nodes[i].rect;
        let mut p = self.frame.nodes[i].parent;
        let now = self.now;
        while let Some(pi) = p {
            let n = &self.frame.nodes[pi];
            if let Behavior::Scroll { y: true, .. } = n.behavior {
                let view = inner_rect(n.rect, &n.style.border_width);
                let st = self.scrolls.entry(n.id).or_default();
                let a = st.y.get_or_insert(Anim::new(0.0));
                let cur = a.target();
                let t = if target.y < view.y {
                    cur - (view.y - target.y)
                } else if target.bottom() > view.bottom() {
                    cur + (target.bottom() - view.bottom()).min(target.y - view.y)
                } else {
                    cur
                };
                a.set(t.clamp(0.0, st.max.y), now, 0.18);
            }
            p = n.parent;
        }
    }
}
