# Testing

`Headless` runs an app without a window: the same runtime, layout and renderer, driven from
code. Tests can click, type and drag, advance time, and then check the app state, the layout,
the pixels or the accessibility tree. It needs no display and no GPU, so it runs anywhere
`cargo test` does.

## A first test

```rust
use charis_ui::prelude::*;

#[derive(Default)]
struct Login {
    name: String,
    submitted: bool,
}

#[derive(Clone)]
enum Msg {
    Name(String),
    Submit,
}

impl App for Login {
    type Msg = Msg;
    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        match msg {
            Msg::Name(s) => self.name = s,
            Msg::Submit => self.submitted = true,
        }
    }
    fn view(&self) -> Element<Msg> {
        col()
            .p(16.0)
            .gap(8.0)
            .child(text_input(self.name.clone(), Msg::Name).id("name"))
            .child(primary_button("Sign in").id("submit").on_click(Msg::Submit))
    }
}

// Width, height, scale factor.
let mut h = Headless::new(Login::default(), 400.0, 300.0, 1.0);
h.settle();

// Click the input by its id, then type.
let input = h.rt.rect_of("name").unwrap();
h.click(input.center().x, input.center().y);
h.type_text("ada");
assert_eq!(h.rt.app.name, "ada");

// Find the button by its text.
let button = h.rt.rect_of_text("Sign in").unwrap();
h.click(button.center().x, button.center().y);
assert!(h.rt.app.submitted);
```

- `h.rt` is the `Runtime`; `h.rt.app` is your app.
- `h.settle()` runs frames until animations finish. `h.advance(secs)` moves time forward by an
  exact amount, for checking a transition halfway.
- `rect_of(id)` finds an element by `.id()`, and `rect_of_text(s)` finds text on screen. Both
  return the layout rectangle in logical pixels.
- `h.rt.send(msg)` delivers a message directly, as if a widget had sent it.

## Input

| Call | What it does |
|---|---|
| `h.click(x, y)` | Press and release the left button. |
| `h.move_to(x, y)` | Move the pointer (hover). |
| `h.drag(from, to, steps)` | Press, move in steps, release. |
| `h.type_text(s)` | Commit text, as typing or an IME would. |
| `h.event(e)` | Send any `Event`: keys, wheel, right-clicks, paste, IME pre-edit, focus. |

Keys go through `Event::Key`:

```rust
use charis_ui::prelude::*;
use charis_ui::Event;

# struct A;
# impl App for A {
#     type Msg = ();
#     fn update(&mut self, _: (), _: &mut Cx<()>) {}
#     fn view(&self) -> Element<()> { div() }
# }
# let mut h = Headless::new(A, 100.0, 100.0, 1.0);
fn press(h: &mut Headless<A>, key: Key, ctrl: bool) {
    let mods = Modifiers { ctrl, ..Default::default() };
    h.event(Event::Key(KeyEvent { key, mods, repeat: false }));
    h.settle();
}
press(&mut h, Key::Char('s'), true);
press(&mut h, Key::Escape, false);
```

## Async work and effects

Tasks started with `cx.spawn` run for real. `h.wait_until(timeout, |app| …)` keeps running
frames until the condition holds or the timeout passes. For effects that would need a person,
the runtime has hooks:

- `h.rt.set_dialog_responder(|request| …)` answers file dialogs with the paths you choose.
- `h.rt.set_external_clipboard(…)` fakes the system clipboard.
- `HeadlessApp` drives apps with several windows: each window has its own `Headless`, and the
  app is shared, so tests can drag a tab from one window into another.

## Pixels and screenshots

`h.pixel(x, y)` returns the RGBA of a rendered pixel, for checking that a color or a highlight
really shows. `h.save_png(path)` writes the frame to a file, which is handy while writing a
test and for documentation screenshots.

**Golden tests** compare a frame with a checked-in PNG. `tests/golden.rs` in the repository
shows the pattern: render at a fixed size with the bundled font, compare each pixel with a small
tolerance, and regenerate the references with `UPDATE_GOLDEN=1` after an intended change. Keep
the tolerance: anti-aliasing can differ slightly between releases (see the
[versioning policy](https://github.com/cochbild/charis-ui/blob/main/docs/SEMVER.md)).

`h.save_png_gpu(path)` renders the same frame with wgpu when an adapter is available. The
crate's own parity test uses it to keep the two renderers within a small difference of each
other.

## Accessibility

`h.rt.accessibility_tree()` returns the AccessKit tree for the current frame: every node's
role, name, value, state and actions. Tests can assert on it to check that custom widgets are
labeled and that controls have the roles screen readers expect. `tests/accessibility.rs` has
examples.

## Tips

- Give elements you want to find an `.id()`. It costs nothing and makes tests robust against
  layout changes.
- Use a scale of `1.0` for layout assertions and `2.0` for screenshots.
- Headless runs use the bundled Inter font unless you load others, so text metrics are the
  same on every machine.
