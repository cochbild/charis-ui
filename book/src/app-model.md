# The app model

Charis follows the Elm architecture:

1. Your app struct holds the state.
2. `view(&self)` turns the state into a tree of elements.
3. User input produces messages, and `update(&mut self, msg, cx)` handles them.
4. After an update, the runtime calls `view` again and redraws what changed.

The runtime keeps UI-only state for you: scroll offsets, splitter positions, text cursors, hover
and focus, running animations. Your model stays about your data.

## Messages

`App::Msg` is any `Clone + 'static` type, usually an enum. Handlers take either a message to send
or a function that builds one from a value:

```rust
use charis_ui::prelude::*;

#[derive(Clone)]
enum Msg {
    Save,
    Rename(String),
    Volume(f32),
}

fn form(name: &str, volume: f32) -> Element<Msg> {
    col()
        .gap(8.0)
        .child(text_input(name.to_string(), Msg::Rename).on_submit(Msg::Save))
        .child(slider(volume, 0.0, 1.0).on_change(Msg::Volume))
        .child(primary_button("Save").on_click(Msg::Save))
}
# let _ = form("a", 0.5);
```

`text_input` and `slider` are *controlled*: they show the value you pass, and report changes as
messages. Store the new value in `update`, and the next `view` shows it.

## Effects with `Cx`

`update` gets a `Cx`, the handle for everything that isn't a state change:

| Call | Effect |
|---|---|
| `cx.focus("id")`, `cx.blur()` | Move keyboard focus. |
| `cx.scroll_to_end("id")`, `cx.scroll_to_item("id", i)` | Scroll a container or virtual list. |
| `cx.copy_to_clipboard(s)`, `cx.read_clipboard(f)` | Use the system clipboard. |
| `cx.open_file(dialog, f)`, `cx.save_file(dialog, f)` | Ask for a file (answered as a message). |
| `cx.spawn(future, f)`, `cx.spawn_blocking(f)` | Run work off the UI thread (answered as a message). |
| `cx.set_title(t)`, `cx.toggle_maximize()`, `cx.close_window()` | Control the window. |

## Async work

`cx.spawn` runs a future and turns its output into a message. `cx.spawn_blocking` runs a closure
on a thread pool. Both return a `TaskHandle` that cancels the task when you call `abort()`.
`cx.run` does the same for a stream, sending one message per item.

```rust
use charis_ui::prelude::*;

#[derive(Default)]
struct Loader {
    text: Option<String>,
}

#[derive(Clone)]
enum Msg {
    Load,
    Loaded(String),
}

impl App for Loader {
    type Msg = Msg;
    fn update(&mut self, msg: Msg, cx: &mut Cx<Msg>) {
        match msg {
            Msg::Load => {
                cx.spawn_blocking(|| Msg::Loaded("file contents".to_string()));
            }
            Msg::Loaded(text) => self.text = Some(text),
        }
    }
    fn view(&self) -> Element<Msg> {
        match &self.text {
            Some(t) => text(t.clone()),
            None => button("Load").on_click(Msg::Load),
        }
    }
}

let mut h = Headless::new(Loader::default(), 300.0, 200.0, 1.0);
h.rt.send(Msg::Load);
assert!(h.wait_until(std::time::Duration::from_secs(5), |app| app.text.is_some()));
```

With the `tokio` feature, futures run on a tokio runtime, so tokio-based crates work inside
`cx.spawn`. Background threads that outlive a task can send messages through `cx.proxy()`.

## Subscriptions

`App::subscriptions` declares timers and window events to listen to. It is re-read after every
update, so a timer runs only while its condition holds:

```rust
# use charis_ui::prelude::*;
# use std::time::Duration;
# struct Clock { running: bool }
# #[derive(Clone)] enum Msg { Tick, CloseRequested }
# impl App for Clock {
#     type Msg = Msg;
#     fn update(&mut self, _: Msg, _: &mut Cx<Msg>) {}
#     fn view(&self) -> Element<Msg> { div() }
fn subscriptions(&self) -> Subscriptions<Msg> {
    Subscriptions::none()
        .every_if(self.running, Duration::from_secs(1), Msg::Tick)
        .on_close_request(Msg::CloseRequested)
}
# }
```

## Identity

The runtime matches elements between frames by their position in the tree. When elements can
move (a reordered list, a panel that comes and goes), give them identity so their state follows
them:

- `.id("name")`: a unique name. Also used by `cx.focus`, `cx.scroll_to_end` and tests.
- `.key(value)`: a key unique among siblings, for list items.

## Components

A component keeps state the app doesn't care about: whether a section is expanded, the text of
a filter box, a picker's visible month. It handles its own events and sends the app a message
only when the app needs to know.

```rust
use charis_ui::prelude::*;

struct Folder {
    name: String,
    items: Vec<String>,
}

#[derive(Clone)]
enum FolderEv {
    Toggle,
    Open(usize),
}

#[derive(Clone)]
enum Msg {
    Open(String),
}

impl Component for Folder {
    type State = bool; // expanded?
    type Event = FolderEv;
    type Output = Msg;

    fn update(&self, open: &mut bool, e: FolderEv) -> Option<Msg> {
        match e {
            FolderEv::Toggle => {
                *open = !*open;
                None
            }
            FolderEv::Open(i) => Some(Msg::Open(self.items[i].clone())),
        }
    }

    fn view(&self, open: &bool) -> Element<FolderEv> {
        let mut e = col().child(button(self.name.clone()).on_click(FolderEv::Toggle));
        if *open {
            for (i, item) in self.items.iter().enumerate() {
                e = e.child(button(item.clone()).on_click(FolderEv::Open(i)));
            }
        }
        e
    }
}

fn sidebar() -> Element<Msg> {
    col().child(component("src", Folder { name: "src".into(), items: vec!["main.rs".into()] }))
}
# let _ = sidebar();
```

For small cases, `stateful(key, view, update)` takes two closures instead of a trait impl. A
component's state lives as long as the component is rendered under the same key.

## Memoized subtrees

`lazy(key, deps, || view)` reuses a subtree while `deps` hashes the same, skipping `view`,
tree building and layout for it. Put it around big parts of the UI that change rarely:

```rust
# use charis_ui::prelude::*;
# #[derive(Clone)] enum Msg {}
# let (files, revision) = (vec!["a.rs".to_string()], 7u64);
let explorer: Element<Msg> = lazy("explorer", revision, || {
    col().children(files.iter().map(|f| text(f.clone())))
});
```

`deps` must cover everything the closure reads that can change. Hover, focus and animations
inside the subtree rebuild it on their own.

## Multiple windows

Extra windows are declared from state, like the rest of the UI. `windows()` lists them and
`window_view(key)` draws each one. A window opens when its key appears in the list and closes
when it disappears; the user closing it sends its `on_close` message.

```rust
# use charis_ui::prelude::*;
# struct Ed { inspectors: Vec<u32> }
# #[derive(Clone)] enum Msg { Close(u32) }
# impl App for Ed {
#     type Msg = Msg;
#     fn update(&mut self, msg: Msg, _: &mut Cx<Msg>) {
#         let Msg::Close(i) = msg;
#         self.inspectors.retain(|&x| x != i);
#     }
#     fn view(&self) -> Element<Msg> { div() }
fn windows(&self) -> Vec<WindowSpec<Msg>> {
    self.inspectors
        .iter()
        .map(|&i| WindowSpec::new(format!("inspector-{i}"), format!("Inspector {i}"), Msg::Close(i)).size(320.0, 220.0))
        .collect()
}

fn window_view(&self, key: &str) -> Element<Msg> {
    text(format!("This is {key}"))
}
# }
```

All windows share the one app, so an update from any window re-renders them all. Each window
keeps its own hover, focus and scroll state.
