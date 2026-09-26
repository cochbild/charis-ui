//! Headless rendering: drive an app without a window, e.g. for tests,
//! screenshots and CI.

use crate::geometry::{Point, Size};
use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;

use crate::runtime::{App, Event, MouseButton, Runtime, Shared, WindowSpec};

/// A windowless harness around a [`Runtime`] with a manual clock.
pub struct Headless<A: App> {
    pub rt: Runtime<A>,
}

impl<A: App> Headless<A> {
    /// Headless runs don't follow the OS reduced-motion setting (so tests
    /// behave the same on every machine) unless the thread has an explicit
    /// [`set_reduced_motion`](crate::anim::set_reduced_motion).
    pub fn new(app: A, width: f32, height: f32, scale: f32) -> Self {
        if crate::anim::reduced_motion_override().is_none() {
            crate::anim::set_reduced_motion(Some(false));
        }
        let mut rt = Runtime::new(app);
        rt.resize(Size::new(width, height), scale);
        rt.render();
        Self { rt }
    }

    /// Advance the clock by `secs`, deliver background messages and due
    /// timers, and render.
    pub fn advance(&mut self, secs: f64) {
        let t = self.rt.time() + secs;
        self.rt.set_time(t);
        self.rt.poll();
        self.rt.render();
    }

    /// Wait (in real time, up to `timeout`) for background tasks until
    /// `done(app)` holds, delivering their messages. Returns whether it held.
    pub fn wait_until(&mut self, timeout: std::time::Duration, done: impl Fn(&A) -> bool) -> bool {
        let start = std::time::Instant::now();
        loop {
            self.rt.poll();
            if done(&self.rt.app) {
                self.rt.render();
                return true;
            }
            if start.elapsed() > timeout {
                return false;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    /// Advance time until no animation is running (max 5s).
    pub fn settle(&mut self) {
        for _ in 0..300 {
            self.advance(1.0 / 60.0);
            match self.rt.next_frame() {
                Some(t) if t <= self.rt.time() => continue,
                _ => break,
            }
        }
    }

    pub fn event(&mut self, e: Event) {
        self.rt.handle(e);
        self.rt.render();
    }

    pub fn move_to(&mut self, x: f32, y: f32) {
        self.event(Event::PointerMove(Point::new(x, y)));
    }

    pub fn click(&mut self, x: f32, y: f32) {
        self.move_to(x, y);
        self.event(Event::PointerDown(Point::new(x, y), MouseButton::Left));
        self.event(Event::PointerUp(Point::new(x, y), MouseButton::Left));
    }

    pub fn drag(&mut self, from: (f32, f32), to: (f32, f32), steps: usize) {
        self.move_to(from.0, from.1);
        self.event(Event::PointerDown(Point::new(from.0, from.1), MouseButton::Left));
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            self.move_to(from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
        }
        self.event(Event::PointerUp(Point::new(to.0, to.1), MouseButton::Left));
    }

    pub fn type_text(&mut self, s: &str) {
        self.event(Event::Text(s.to_string()));
    }

    /// Save the last frame as PNG.
    pub fn save_png(&mut self, path: impl AsRef<std::path::Path>) -> Result<(), String> {
        self.rt.render();
        self.rt.pixmap().ok_or("no frame")?.save_png(path).map_err(|e| e.to_string())
    }

    /// Render the current frame with the GPU backend and save it as PNG.
    /// Returns an error if no GPU adapter is available.
    #[cfg(feature = "gpu")]
    pub fn save_png_gpu(&mut self, path: impl AsRef<std::path::Path>) -> Result<(), String> {
        let mut gpu = crate::gpu::GpuRenderer::headless().ok_or("no GPU adapter available")?;
        self.rt.render_scene();
        let pm = self.rt.with_scene(|s, t| gpu.render_to_pixmap(s, t)).flatten().ok_or("GPU render failed")?;
        pm.save_png(path).map_err(|e| e.to_string())
    }

    /// RGBA pixel at logical coordinates (for assertions).
    ///
    /// # Panics
    /// If nothing was rendered yet or the point is outside the window; this is
    /// a test helper, so failing loudly is intended.
    pub fn pixel(&self, x: f32, y: f32) -> [u8; 4] {
        let pm = self.rt.pixmap().expect("frame");
        let s = pm.width() as f32 / self.rt_size().w;
        let px = pm.pixel((x * s) as u32, (y * s) as u32).expect("in bounds");
        let c = px.demultiply();
        [c.red(), c.green(), c.blue(), c.alpha()]
    }

    fn rt_size(&self) -> Size {
        let pm = self.rt.pixmap().expect("frame");
        // Logical size = physical / scale; derived from the runtime's state.
        let _ = pm;
        self.rt.logical_size()
    }
}

/// A windowless harness for apps with several windows ([`App::windows`]):
/// one [`Headless`] per open window around a shared app, kept in sync the
/// way the windowing shell does it.
///
/// ```
/// # use rust_ui::prelude::*;
/// # use rust_ui::headless::HeadlessApp;
/// # #[derive(Default)] struct A { open: bool }
/// # #[derive(Clone, Debug)] enum Msg { Open, Close }
/// # impl App for A {
/// #     type Msg = Msg;
/// #     fn update(&mut self, m: Msg, _: &mut Cx<Msg>) { self.open = matches!(m, Msg::Open) }
/// #     fn view(&self) -> Element<Msg> { button("Open").id("open").on_click(Msg::Open) }
/// #     fn windows(&self) -> Vec<WindowSpec<Msg>> {
/// #         if self.open { vec![WindowSpec::new("tool", "Tool", Msg::Close)] } else { vec![] }
/// #     }
/// # }
/// let mut h = HeadlessApp::new(A::default(), 400.0, 300.0);
/// let r = h.main().rt.rect_of("open").unwrap();
/// h.click(None, r.center().x, r.center().y);
/// assert!(h.window("tool").is_some());
/// h.close("tool");
/// assert!(h.window("tool").is_none());
/// ```
pub struct HeadlessApp<A: App> {
    app: Rc<RefCell<A>>,
    main: Headless<Shared<A>>,
    windows: Vec<OpenWindow<A>>,
}

/// An extra window of a [`HeadlessApp`]: its spec and harness.
type OpenWindow<A> = (WindowSpec<<A as App>::Msg>, Headless<Shared<A>>);

impl<A: App> HeadlessApp<A> {
    pub fn new(app: A, width: f32, height: f32) -> Self {
        let app = Rc::new(RefCell::new(app));
        let main = Headless::new(Shared::new(app.clone(), None), width, height, 1.0);
        let mut h = Self { app, main, windows: Vec::new() };
        h.sync();
        h
    }

    /// The shared app state.
    pub fn app(&self) -> Ref<'_, A> {
        self.app.borrow()
    }

    pub fn app_mut(&mut self) -> RefMut<'_, A> {
        self.app.borrow_mut()
    }

    pub fn main(&mut self) -> &mut Headless<Shared<A>> {
        &mut self.main
    }

    /// An open extra window.
    pub fn window(&mut self, key: &str) -> Option<&mut Headless<Shared<A>>> {
        self.windows.iter_mut().find(|(s, _)| s.key == key).map(|(_, h)| h)
    }

    /// Keys of the open extra windows, in declaration order.
    pub fn window_keys(&self) -> Vec<String> {
        self.windows.iter().map(|(s, _)| s.key.clone()).collect()
    }

    fn get(&mut self, key: Option<&str>) -> &mut Headless<Shared<A>> {
        match key {
            None => &mut self.main,
            Some(k) => self.window(k).unwrap_or_else(|| panic!("no window {k:?}")),
        }
    }

    /// Click in a window (`None` = main).
    pub fn click(&mut self, window: Option<&str>, x: f32, y: f32) {
        self.get(window).click(x, y);
        self.sync();
    }

    /// Deliver an event to a window (`None` = main).
    pub fn event(&mut self, window: Option<&str>, e: Event) {
        self.get(window).event(e);
        self.sync();
    }

    /// Type text into a window's focused input.
    pub fn type_text(&mut self, window: Option<&str>, s: &str) {
        self.get(window).type_text(s);
        self.sync();
    }

    /// The user closes an extra window (its `on_close` message is sent).
    pub fn close(&mut self, key: &str) {
        if let Some((spec, _)) = self.windows.iter().find(|(s, _)| s.key == key) {
            let m = spec.on_close.clone();
            self.main.rt.send(m);
            self.main.rt.poll();
        }
        self.sync();
    }

    /// Advance every window's clock.
    pub fn advance(&mut self, secs: f64) {
        self.main.advance(secs);
        for (_, w) in &mut self.windows {
            w.advance(secs);
        }
        self.sync();
    }

    /// Propagate app updates to every window and open/close windows to
    /// match [`App::windows`].
    pub fn sync(&mut self) {
        let mut updated = self.main.rt.take_updated();
        for (_, w) in &mut self.windows {
            updated |= w.rt.take_updated();
        }
        let specs = self.app.borrow().windows();
        let mut next = Vec::with_capacity(specs.len());
        for spec in specs {
            let h = match self.windows.iter().position(|(s, _)| s.key == spec.key) {
                Some(i) => self.windows.remove(i).1,
                None => {
                    Headless::new(Shared::new(self.app.clone(), Some(spec.key.clone())), spec.width, spec.height, 1.0)
                }
            };
            next.push((spec, h));
        }
        self.windows = next;
        if updated {
            self.main.rt.invalidate();
            self.main.rt.render();
            for (_, w) in &mut self.windows {
                w.rt.invalidate();
                w.rt.render();
            }
        }
    }
}
