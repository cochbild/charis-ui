//! Memoized subtrees ([`lazy`](crate::lazy)).
//!
//! Before `view` runs, the runtime decides which of last frame's memoized
//! subtrees are still valid (same theme and scale, nothing inside hovered,
//! pressed, focused or animating, and no live-state widgets inside) and
//! publishes them in a thread-local registry. `lazy` claims an entry when
//! its deps match, skipping its build closure; `flatten` then copies the
//! subtree's nodes from the previous frame, and layout reuses their cached
//! layout.

use std::cell::RefCell;

use super::*;

/// Salt for the ids of `lazy` wrappers (their ids ignore the parent, so a
/// memoized subtree keeps its ids, and its state, wherever it is placed).
pub(crate) const LAZY_SALT: u64 = 0x1a2f_7e3d_9c4b_5a61;

#[derive(Default)]
struct Registry {
    /// Reusable memo keys: the deps they were built with and the components
    /// rendered inside them.
    valid: HashMap<u64, (u64, Vec<u64>)>,
    claimed: HashSet<u64>,
    /// Components rendered under each `lazy` currently being built.
    collecting: Vec<Vec<u64>>,
    /// Components rendered (or kept by a reused subtree) this build.
    seen: HashSet<u64>,
}

thread_local! {
    static REGISTRY: RefCell<Option<Registry>> = const { RefCell::new(None) };
}

/// Called by [`lazy`](crate::lazy): when last frame's subtree for `key` can
/// be reused, claims it and returns the components inside it.
pub(crate) fn claim(key: u64, deps: u64) -> Option<Vec<u64>> {
    REGISTRY.with(|r| {
        let mut r = r.borrow_mut();
        let r = r.as_mut()?;
        let comps = match r.valid.get(&key) {
            Some((d, comps)) if *d == deps && !r.claimed.contains(&key) => comps.clone(),
            _ => return None,
        };
        r.claimed.insert(key);
        for &c in &comps {
            note(r, c);
        }
        Some(comps)
    })
}

fn note(r: &mut Registry, comp: u64) {
    r.seen.insert(comp);
    if let Some(top) = r.collecting.last_mut() {
        top.push(comp);
    }
}

/// Record that a component was rendered (keeps its state alive, and ties it
/// to the enclosing `lazy`s).
pub(crate) fn note_component(comp: u64) {
    REGISTRY.with(|r| {
        if let Some(r) = r.borrow_mut().as_mut() {
            note(r, comp);
        }
    })
}

/// Run a `lazy` build closure, collecting the components rendered inside.
pub(crate) fn collect_components<T>(build: impl FnOnce() -> T) -> (T, Vec<u64>) {
    let active = REGISTRY.with(|r| r.borrow_mut().as_mut().map(|r| r.collecting.push(Vec::new())).is_some());
    let out = build();
    let comps = if active {
        REGISTRY.with(|r| {
            let mut r = r.borrow_mut();
            let Some(r) = r.as_mut() else { return Vec::new() };
            let comps = r.collecting.pop().unwrap_or_default();
            if let Some(top) = r.collecting.last_mut() {
                top.extend_from_slice(&comps);
            }
            comps
        })
    } else {
        Vec::new()
    };
    (out, comps)
}

/// A memoized subtree of the current frame: nodes `start..end` are the
/// descendants of the `lazy` wrapper.
pub(super) struct MemoEntry {
    deps: u64,
    start: usize,
    end: usize,
    /// Contains widgets that depend on live runtime state (virtual lists,
    /// splits, dropdowns, table cells, `position: fixed` elements).
    volatile: bool,
    /// A transition was still running when it was built.
    animating: bool,
    /// Hash of the hovered / pressed / focused ids inside it when built.
    interact: u64,
    /// Memo keys directly nested inside.
    nested: Vec<u64>,
    /// Components rendered inside (including nested memos').
    components: Vec<u64>,
}

/// Per-build memo bookkeeping.
#[derive(Default)]
pub(super) struct MemoBuild {
    pub entries: HashMap<u64, MemoEntry>,
    /// Keys recorded under the `lazy` currently being flattened.
    stack: Vec<Vec<u64>>,
    /// Counters bumped by `flatten` for volatile content and running
    /// transitions; a subtree is volatile/animating if they moved while it
    /// was flattened.
    pub volatile: u32,
    pub animating: u32,
}

/// Theme equality for memo purposes: style-class functions can't be
/// compared, so classes count as equal when they have the same names.
fn same_theme(a: &Theme, b: &Theme) -> bool {
    if a.classes != b.classes {
        let mut an: Vec<&str> = a.classes.names().collect();
        let mut bn: Vec<&str> = b.classes.names().collect();
        an.sort_unstable();
        bn.sort_unstable();
        if an != bn {
            return false;
        }
    }
    let mut a2 = a.clone();
    a2.classes = b.classes.clone();
    a2 == *b
}

/// Move a node's contents out, leaving a husk with its id and geometry.
fn take_node<M>(n: &mut Node<M>) -> Node<M> {
    Node {
        id: n.id,
        key: n.key,
        parent: n.parent,
        children: std::mem::take(&mut n.children),
        style: std::mem::take(&mut n.style),
        content: std::mem::replace(&mut n.content, NodeContent::None),
        handlers: std::mem::take(&mut n.handlers),
        behavior: std::mem::replace(&mut n.behavior, Behavior::None),
        focusable: n.focusable,
        autofocus: n.autofocus,
        disabled: n.disabled,
        hit_slop: n.hit_slop,
        pointer_events: n.pointer_events,
        tooltip: n.tooltip.take(),
        follow_end: n.follow_end,
        text: n.text.clone(),
        color: n.color,
        tnode: n.tnode,
        rect: n.rect,
        clip: n.clip,
        content_size: n.content_size,
        scroll: n.scroll,
        split: n.split,
        pane: n.pane,
        virt_item: n.virt_item,
        sem: n.sem.take(),
        own_pointer: n.own_pointer,
        inherit_align: n.inherit_align,
        reused: n.reused,
    }
}

impl<A: App> Runtime<A> {
    /// Hash of the interaction state inside nodes `start..end` of the
    /// current frame.
    fn interact_sig(&self, start: usize, end: usize) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let inside = |id: &u64| self.frame.by_id.get(id).is_some_and(|&i| i >= start && i < end);
        for id in self.hovered.iter().filter(|id| inside(id)) {
            (0u8, id).hash(&mut h);
        }
        for id in self.pressed.iter().filter(|id| inside(id)) {
            (1u8, id).hash(&mut h);
        }
        if let Some(f) = self.focused.filter(inside) {
            (2u8, f, self.focus_visible).hash(&mut h);
        }
        h.finish()
    }

    /// Publish which memoized subtrees of the current frame are reusable,
    /// before `view` runs. `prev_theme` is the theme they were built with.
    pub(super) fn memo_begin(&mut self, prev_theme: &Theme, prev_scale: f32) {
        let mut reg = Registry::default();
        if prev_scale == self.scale && same_theme(prev_theme, &self.theme) {
            let store = self.components.borrow();
            for (&key, e) in &self.memos {
                if !e.volatile
                    && !e.animating
                    && !e.components.iter().any(|c| store.is_dirty(*c))
                    && self.interact_sig(e.start, e.end) == e.interact
                {
                    reg.valid.insert(key, (e.deps, e.components.clone()));
                }
            }
        }
        REGISTRY.with(|r| *r.borrow_mut() = Some(reg));
        self.memo_build = MemoBuild::default();
        self.prev_store = crate::component::install(Some(self.components.clone()));
    }

    /// Finish the build: withdraw the registry and record the new frame's
    /// memo entries with their interaction state.
    pub(super) fn memo_end(&mut self) {
        let reg = REGISTRY.with(|r| r.borrow_mut().take()).unwrap_or_default();
        crate::component::install(self.prev_store.take());
        self.components.borrow_mut().end_build(&reg.seen);
        let mut entries = std::mem::take(&mut self.memo_build.entries);
        for e in entries.values_mut() {
            e.interact = self.interact_sig(e.start, e.end);
        }
        self.memos = entries;
    }

    /// Flatten a `lazy` wrapper's content (the wrapper node `idx` is already
    /// pushed): build it fresh, or copy last frame's subtree.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn flatten_lazy(
        &mut self,
        idx: usize,
        id: u64,
        spec: LazySpec<A::Msg>,
        text: &TextStyle,
        color: Color,
        pointer: bool,
        frame: &mut Frame<A::Msg>,
    ) {
        match spec.built {
            Some(child) => {
                let start = frame.nodes.len();
                let (v0, a0) = (self.memo_build.volatile, self.memo_build.animating);
                self.memo_build.stack.push(Vec::new());
                self.flatten(*child, Some(idx), id, 0, text, color, pointer, frame);
                let nested = self.memo_build.stack.pop().unwrap_or_default();
                let entry = MemoEntry {
                    deps: spec.deps,
                    start,
                    end: frame.nodes.len(),
                    volatile: self.memo_build.volatile != v0,
                    animating: self.memo_build.animating != a0,
                    interact: 0,
                    nested,
                    components: spec.components,
                };
                self.record_memo(spec.key, entry);
            }
            None => self.reuse_memo(spec.key, idx, frame),
        }
    }

    fn record_memo(&mut self, key: u64, entry: MemoEntry) {
        if let Some(parent) = self.memo_build.stack.last_mut() {
            parent.push(key);
        }
        self.memo_build.entries.insert(key, entry);
    }

    /// Copy the memoized subtree `key` from the previous frame under the
    /// wrapper node `idx`, re-resolving what it inherits from its parent.
    fn reuse_memo(&mut self, key: u64, idx: usize, frame: &mut Frame<A::Msg>) {
        let Some(old) = self.memos.get(&key) else {
            debug_assert!(false, "claimed memo {key:x} has no entry");
            return;
        };
        let (os, oe) = (old.start, old.end);
        let base = frame.nodes.len();
        let fno = self.frame_no;
        frame.nodes.reserve(oe - os);
        for oi in os..oe {
            // The previous frame is discarded after this build, and nothing
            // reads these nodes before then, so move them instead of cloning.
            let mut n = take_node(&mut self.frame.nodes[oi]);
            let p = match n.parent {
                Some(p) if p >= os && p < oe => p - os + base,
                _ => idx,
            };
            n.parent = Some(p);
            for c in &mut n.children {
                *c = *c - os + base;
            }
            let pn = &frame.nodes[p];
            let t = inherit_text(&n.style, &pn.text);
            n.reused = t == n.text;
            n.text = t;
            n.color = n.style.color.unwrap_or(pn.color);
            n.pointer_events = n.own_pointer.unwrap_or(pn.pointer_events);
            if n.inherit_align {
                n.style.text_align = pn.style.text_align;
            }
            // Keep settled transitions alive, so the next change animates.
            if n.style.transition > 0.0 {
                if let Some(tr) = self.transitions.get_mut(&n.id) {
                    tr.seen = fno;
                }
            }
            frame.by_id.insert(n.id, frame.nodes.len());
            frame.nodes.push(n);
        }
        if oe > os {
            frame.nodes[idx].children.push(base);
        }
        self.relocate_memo(key, base as isize - os as isize);
    }

    /// Carry a reused entry (and those nested in it) into this build.
    fn relocate_memo(&mut self, key: u64, delta: isize) {
        let Some(old) = self.memos.get(&key) else { return };
        let entry = MemoEntry {
            deps: old.deps,
            start: (old.start as isize + delta) as usize,
            end: (old.end as isize + delta) as usize,
            volatile: false,
            animating: false,
            interact: 0,
            nested: old.nested.clone(),
            components: old.components.clone(),
        };
        let nested = entry.nested.clone();
        self.record_memo(key, entry);
        self.memo_build.stack.push(Vec::new());
        for k in nested {
            self.relocate_memo(k, delta);
        }
        self.memo_build.stack.pop();
    }
}
