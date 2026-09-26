//! Components with their own state.
//!
//! Most state belongs in the app, where `update` can see it. But reusable
//! pieces of UI often have state nobody else cares about: whether a
//! section is expanded, a date picker's visible month, the text typed into a
//! filter box. A [`Component`] keeps that state in the runtime, handles its
//! own events, and sends the app a message only when something happens that
//! the app needs to know about.
//!
//! ```
//! use rust_ui::prelude::*;
//!
//! /// A counter that tells the app when it reaches a limit.
//! struct Counter {
//!     limit: u32,
//! }
//!
//! #[derive(Clone)]
//! enum Event {
//!     Inc,
//! }
//!
//! impl Component for Counter {
//!     type State = u32;
//!     type Event = Event;
//!     type Output = String; // the app's message type, or anything mapped to it
//!
//!     fn update(&self, count: &mut u32, e: Event) -> Option<String> {
//!         match e {
//!             Event::Inc => {
//!                 *count += 1;
//!                 (*count == self.limit).then(|| format!("reached {}", self.limit))
//!             }
//!         }
//!     }
//!
//!     fn view(&self, count: &u32) -> Element<Event> {
//!         button(format!("Clicked {count} times")).on_click(Event::Inc)
//!     }
//! }
//!
//! let e: Element<String> = component("counter", Counter { limit: 3 });
//! ```
//!
//! The key identifies the component's state: it must be unique among
//! components (of the same state type) in the window, and the state lives as
//! long as a component with that key is rendered each frame (inside reused
//! [`lazy`](crate::lazy) subtrees too). Components nest, and a local update
//! rebuilds any `lazy` subtree the component is in.

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use crate::element::{Element, Local, LocalFn, Out};
use crate::fxhash::{FxHashSet, IdMap};

/// A reusable piece of UI with its own state and events.
pub trait Component: 'static {
    /// Local state, created by [`Component::init`] the first time the
    /// component is rendered.
    type State: Default + 'static;
    /// What the component's own elements send (handled by `update`).
    type Event: Clone + 'static;
    /// What the component sends to its parent.
    type Output: 'static;

    /// The initial state (default: `State::default()`).
    fn init(&self) -> Self::State {
        Self::State::default()
    }

    /// Handle an event; return a message for the parent, if any.
    fn update(&self, state: &mut Self::State, event: Self::Event) -> Option<Self::Output>;

    /// Build the component's UI from its state.
    fn view(&self, state: &Self::State) -> Element<Self::Event>;
}

/// Render a [`Component`]. See the [module docs](crate::component).
pub fn component<C: Component>(key: impl Hash, c: C) -> Element<C::Output> {
    let id = {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut h);
        TypeId::of::<C::State>().hash(&mut h);
        h.finish()
    };
    crate::runtime::memo::note_component(id);
    let store = CURRENT.with(|c| c.borrow().clone());
    let state: Box<dyn Any> = store
        .as_ref()
        .and_then(|s| s.borrow_mut().states.remove(&id))
        .filter(|b| b.is::<C::State>())
        .unwrap_or_else(|| Box::new(c.init()));
    // The state is out of the store while `view` runs, so nested components
    // can use the store.
    let el = match state.downcast_ref::<C::State>() {
        Some(s) => c.view(s),
        None => c.view(&c.init()),
    };
    if let Some(store) = &store {
        store.borrow_mut().states.insert(id, state);
    }
    let c = Rc::new(c);
    el.map_out(Rc::new(move |o: Out<C::Event>| {
        let c = c.clone();
        // An event of this component, or the output of a nested component
        // (its local update, then this one's).
        let inner: LocalFn<C::Event> = match o {
            Out::Msg(e) => Rc::new(move |_| Some(e.clone())),
            Out::Local(l) => l.apply,
        };
        Out::Local(Local {
            apply: Rc::new(move |store: &mut Store| {
                let e = inner(store)?;
                store.update::<C::State, _>(id, |s| c.update(s, e)).flatten()
            }),
        })
    }))
}

/// A component from two closures, for one-off stateful pieces without a
/// named type. `view` gets the state; `update` handles events and may return
/// a message for the parent.
///
/// ```
/// # use rust_ui::prelude::*;
/// # #[derive(Clone)] enum Msg { Picked(usize) }
/// #[derive(Clone)]
/// enum Ev { Toggle, Pick(usize) }
///
/// let e: Element<Msg> = stateful(
///     "sidebar-section",
///     |open: &bool| {
///         col()
///             .child(button("Recent").on_click(Ev::Toggle))
///             .child_if(*open, || button("Item 1").on_click(Ev::Pick(1)))
///     },
///     |open: &mut bool, e| match e {
///         Ev::Toggle => { *open = !*open; None }
///         Ev::Pick(i) => Some(Msg::Picked(i)),
///     },
/// );
/// ```
pub fn stateful<S, E, M>(
    key: impl Hash,
    view: impl Fn(&S) -> Element<E> + 'static,
    update: impl Fn(&mut S, E) -> Option<M> + 'static,
) -> Element<M>
where
    S: Default + 'static,
    E: Clone + 'static,
    M: 'static,
{
    component(key, Closures { view, update, _t: std::marker::PhantomData })
}

struct Closures<V, U, S, E, M> {
    view: V,
    update: U,
    _t: std::marker::PhantomData<fn(S, E) -> M>,
}

impl<V, U, S, E, M> Component for Closures<V, U, S, E, M>
where
    V: Fn(&S) -> Element<E> + 'static,
    U: Fn(&mut S, E) -> Option<M> + 'static,
    S: Default + 'static,
    E: Clone + 'static,
    M: 'static,
{
    type State = S;
    type Event = E;
    type Output = M;
    fn update(&self, state: &mut S, event: E) -> Option<M> {
        (self.update)(state, event)
    }
    fn view(&self, state: &S) -> Element<E> {
        (self.view)(state)
    }
}

/// Component states, owned by the runtime.
#[derive(Default)]
pub struct Store {
    states: IdMap<Box<dyn Any>>,
    /// Components whose state changed since the last build.
    dirty: FxHashSet<u64>,
}

impl Store {
    /// Run `f` on component `id`'s state (if it exists and has type `S`),
    /// marking it changed.
    fn update<S: 'static, R>(&mut self, id: u64, f: impl FnOnce(&mut S) -> R) -> Option<R> {
        let s = self.states.get_mut(&id)?.downcast_mut::<S>()?;
        self.dirty.insert(id);
        Some(f(s))
    }

    pub(crate) fn is_dirty(&self, id: u64) -> bool {
        self.dirty.contains(&id)
    }

    /// After a build: drop the state of components that weren't rendered,
    /// and forget which changed.
    pub(crate) fn end_build(&mut self, seen: &impl SeenSet) {
        self.states.retain(|id, _| seen.has(*id));
        self.dirty.clear();
    }

    /// Number of live component states (for tests and debugging).
    pub fn len(&self) -> usize {
        self.states.len()
    }

    /// Whether there are no live component states.
    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }
}

/// Set of component ids rendered in a build.
pub(crate) trait SeenSet {
    fn has(&self, id: u64) -> bool;
}

impl<S: std::hash::BuildHasher> SeenSet for std::collections::HashSet<u64, S> {
    fn has(&self, id: u64) -> bool {
        self.contains(&id)
    }
}

thread_local! {
    static CURRENT: RefCell<Option<Rc<RefCell<Store>>>> = const { RefCell::new(None) };
}

/// Make `store` the one `component` reads during a build; returns the
/// previous one (restore it with another call).
pub(crate) fn install(store: Option<Rc<RefCell<Store>>>) -> Option<Rc<RefCell<Store>>> {
    CURRENT.with(|c| std::mem::replace(&mut *c.borrow_mut(), store))
}

/// Apply a local update to the store.
pub(crate) fn apply<M>(store: &Rc<RefCell<Store>>, l: Local<M>) -> Option<M> {
    (l.apply)(&mut store.borrow_mut())
}
