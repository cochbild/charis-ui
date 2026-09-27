//! Background work: async tasks, streams and cross-thread messages.
//!
//! From [`App::update`](crate::App::update):
//!
//! ```no_run
//! # use charis_ui::prelude::*;
//! # #[derive(Clone)] enum Msg { Loaded(String), Token(String), Done }
//! # struct S; impl S { fn upd(&mut self, cx: &mut Cx<Msg>) {
//! // One result:
//! cx.spawn(async { "hello".to_string() }, Msg::Loaded);
//! // Many results (e.g. streamed LLM tokens), cancellable:
//! let stream = futures::stream::iter(["a", "b"].map(String::from));
//! let handle = cx.run(stream, Msg::Token);
//! handle.abort();
//! // Push messages from any thread:
//! let proxy = cx.proxy();
//! std::thread::spawn(move || { proxy.send(Msg::Done); });
//! # }}
//! ```
//!
//! Futures run on tokio when the `tokio` feature is enabled (an existing
//! runtime is reused if the app created one), otherwise on a background
//! thread each.

use std::future::Future;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use futures::future::{AbortHandle, Abortable};
use futures::{Stream, StreamExt};

/// Wakes the UI event loop after a message is posted from another thread.
pub(crate) type Waker = Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>;

/// Sends messages to the app from any thread. Cheap to clone.
pub struct Proxy<M> {
    tx: Sender<M>,
    wake: Waker,
}

impl<M> Clone for Proxy<M> {
    fn clone(&self) -> Self {
        Self { tx: self.tx.clone(), wake: self.wake.clone() }
    }
}

impl<M> Proxy<M> {
    /// Deliver a message to [`App::update`](crate::App::update). Returns false
    /// if the app has shut down.
    pub fn send(&self, msg: M) -> bool {
        let ok = self.tx.send(msg).is_ok();
        if let Ok(w) = self.wake.lock() {
            if let Some(w) = w.as_ref() {
                w();
            }
        }
        ok
    }
}

/// A handle to a spawned task or stream. Dropping it does **not** cancel the
/// task; call [`TaskHandle::abort`].
#[derive(Clone, Debug)]
pub struct TaskHandle {
    abort: AbortHandle,
}

impl TaskHandle {
    /// Cancel the task. No further messages from it are delivered.
    pub fn abort(&self) {
        self.abort.abort();
    }

    /// Whether [`TaskHandle::abort`] was called.
    pub fn is_aborted(&self) -> bool {
        self.abort.is_aborted()
    }
}

/// Channel plumbing owned by the runtime.
pub(crate) struct Mailbox<M> {
    pub tx: Sender<M>,
    pub rx: Receiver<M>,
    pub wake: Waker,
}

impl<M> Mailbox<M> {
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self { tx, rx, wake: Arc::new(Mutex::new(None)) }
    }
    pub fn proxy(&self) -> Proxy<M> {
        Proxy { tx: self.tx.clone(), wake: self.wake.clone() }
    }
}

fn execute(fut: impl Future<Output = ()> + Send + 'static) {
    #[cfg(feature = "tokio")]
    {
        tokio_handle().spawn(fut);
    }
    #[cfg(not(feature = "tokio"))]
    {
        std::thread::Builder::new()
            .name("charis-task".into())
            .spawn(move || futures::executor::block_on(fut))
            .expect("failed to spawn a task thread");
    }
}

#[cfg(feature = "tokio")]
fn tokio_handle() -> tokio::runtime::Handle {
    use std::sync::OnceLock;
    if let Ok(h) = tokio::runtime::Handle::try_current() {
        return h;
    }
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("charis-tokio")
            .build()
            .expect("tokio runtime")
    })
    .handle()
    .clone()
}

pub(crate) fn spawn<F, M>(proxy: Proxy<M>, fut: F, map: impl FnOnce(F::Output) -> M + Send + 'static) -> TaskHandle
where
    F: Future + Send + 'static,
    F::Output: Send,
    M: Send + 'static,
{
    let (abort, reg) = AbortHandle::new_pair();
    let task = Abortable::new(fut, reg);
    execute(async move {
        if let Ok(out) = task.await {
            proxy.send(map(out));
        }
    });
    TaskHandle { abort }
}

pub(crate) fn run<S, M>(proxy: Proxy<M>, stream: S, mut map: impl FnMut(S::Item) -> M + Send + 'static) -> TaskHandle
where
    S: Stream + Send + 'static,
    S::Item: Send,
    M: Send + 'static,
{
    let (abort, reg) = AbortHandle::new_pair();
    let mut stream = Box::pin(futures::stream::Abortable::new(stream, reg));
    execute(async move {
        while let Some(item) = stream.next().await {
            if !proxy.send(map(item)) {
                break;
            }
        }
    });
    TaskHandle { abort }
}

pub(crate) fn spawn_blocking<M: Send + 'static>(proxy: Proxy<M>, f: impl FnOnce() -> M + Send + 'static) -> TaskHandle {
    let (abort, reg) = AbortHandle::new_pair();
    let a = abort.clone();
    std::thread::Builder::new()
        .name("charis-blocking".into())
        .spawn(move || {
            let m = f();
            if !a.is_aborted() {
                proxy.send(m);
            }
            drop(reg);
        })
        .expect("failed to spawn a blocking thread");
    TaskHandle { abort }
}
