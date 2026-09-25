//! Headless rendering: drive an app without a window, e.g. for tests,
//! screenshots and CI.

use crate::geometry::{Point, Size};
use crate::runtime::{App, Event, MouseButton, Runtime};

/// A windowless harness around a [`Runtime`] with a manual clock.
pub struct Headless<A: App> {
    pub rt: Runtime<A>,
}

impl<A: App> Headless<A> {
    pub fn new(app: A, width: f32, height: f32, scale: f32) -> Self {
        let mut rt = Runtime::new(app);
        rt.resize(Size::new(width, height), scale);
        rt.render();
        Self { rt }
    }

    /// Advance the clock by `secs` and render.
    pub fn advance(&mut self, secs: f64) {
        let t = self.rt.time() + secs;
        self.rt.set_time(t);
        self.rt.render();
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
