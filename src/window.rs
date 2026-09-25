//! Native window shell built on winit + softbuffer.

use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Ime, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key as WKey, ModifiersState, NamedKey};
use winit::window::{CursorIcon, ResizeDirection, Window, WindowId};

use crate::element::{Key, KeyEvent, Modifiers};
use crate::geometry::{Point, Size};
use crate::runtime::{App, Event, MouseButton, ResizeEdge, Runtime, WindowRequest};
use crate::style::Cursor;

/// Options for the native window.
#[derive(Debug, Clone)]
pub struct WindowOptions {
    pub title: String,
    pub width: f32,
    pub height: f32,
    pub min_width: f32,
    pub min_height: f32,
    /// Draw no OS title bar/borders; the app provides its own chrome
    /// (see [`titlebar`](crate::widgets::titlebar)). Edges remain resizable.
    pub frameless: bool,
    pub resizable: bool,
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self {
            title: "rust-ui".into(),
            width: 1200.0,
            height: 780.0,
            min_width: 420.0,
            min_height: 300.0,
            frameless: false,
            resizable: true,
        }
    }
}

impl WindowOptions {
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), ..Default::default() }
    }
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.width = w;
        self.height = h;
        self
    }
    pub fn min_size(mut self, w: f32, h: f32) -> Self {
        self.min_width = w;
        self.min_height = h;
        self
    }
    pub fn frameless(mut self, f: bool) -> Self {
        self.frameless = f;
        self
    }
}

struct Gfx {
    window: Rc<Window>,
    surface: softbuffer::Surface<Rc<Window>, Rc<Window>>,
    _context: softbuffer::Context<Rc<Window>>,
}

struct Shell<A: App> {
    rt: Runtime<A>,
    opts: WindowOptions,
    gfx: Option<Gfx>,
    start: Instant,
    mods: ModifiersState,
    last_frame: Instant,
    #[cfg(feature = "clipboard")]
    clipboard: Option<arboard::Clipboard>,
    error: Option<String>,
}

/// Open a window and run the app until it is closed.
pub fn run<A: App>(app: A, opts: WindowOptions) -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    let mut rt = Runtime::new(app);
    rt.frameless = opts.frameless;
    let mut shell = Shell {
        rt,
        opts,
        gfx: None,
        start: Instant::now(),
        mods: ModifiersState::empty(),
        last_frame: Instant::now(),
        #[cfg(feature = "clipboard")]
        clipboard: arboard::Clipboard::new().ok(),
        error: None,
    };
    event_loop.run_app(&mut shell)?;
    match shell.error {
        Some(e) => Err(e.into()),
        None => Ok(()),
    }
}

fn map_cursor(c: Cursor) -> CursorIcon {
    match c {
        Cursor::Default => CursorIcon::Default,
        Cursor::Pointer => CursorIcon::Pointer,
        Cursor::Text => CursorIcon::Text,
        Cursor::Grab => CursorIcon::Grab,
        Cursor::Grabbing => CursorIcon::Grabbing,
        Cursor::Move => CursorIcon::Move,
        Cursor::NotAllowed => CursorIcon::NotAllowed,
        Cursor::ResizeCol => CursorIcon::ColResize,
        Cursor::ResizeRow => CursorIcon::RowResize,
        Cursor::ResizeNs => CursorIcon::NsResize,
        Cursor::ResizeEw => CursorIcon::EwResize,
        Cursor::ResizeNwse => CursorIcon::NwseResize,
        Cursor::ResizeNesw => CursorIcon::NeswResize,
    }
}

fn map_key(k: &WKey) -> Key {
    match k {
        WKey::Named(n) => match n {
            NamedKey::Enter => Key::Enter,
            NamedKey::Tab => Key::Tab,
            NamedKey::Escape => Key::Escape,
            NamedKey::Backspace => Key::Backspace,
            NamedKey::Delete => Key::Delete,
            NamedKey::ArrowLeft => Key::Left,
            NamedKey::ArrowRight => Key::Right,
            NamedKey::ArrowUp => Key::Up,
            NamedKey::ArrowDown => Key::Down,
            NamedKey::Home => Key::Home,
            NamedKey::End => Key::End,
            NamedKey::PageUp => Key::PageUp,
            NamedKey::PageDown => Key::PageDown,
            NamedKey::Space => Key::Space,
            NamedKey::F1 => Key::F(1),
            NamedKey::F2 => Key::F(2),
            NamedKey::F3 => Key::F(3),
            NamedKey::F4 => Key::F(4),
            NamedKey::F5 => Key::F(5),
            NamedKey::F6 => Key::F(6),
            NamedKey::F7 => Key::F(7),
            NamedKey::F8 => Key::F(8),
            NamedKey::F9 => Key::F(9),
            NamedKey::F10 => Key::F(10),
            NamedKey::F11 => Key::F(11),
            NamedKey::F12 => Key::F(12),
            _ => Key::Other,
        },
        WKey::Character(s) => match s.chars().next() {
            Some(' ') => Key::Space,
            Some(c) => Key::Char(c),
            None => Key::Other,
        },
        _ => Key::Other,
    }
}

impl<A: App> Shell<A> {
    fn now(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    fn apply_requests(&mut self, el: &ActiveEventLoop) {
        let Some(g) = &self.gfx else { return };
        for r in self.rt.take_requests() {
            match r {
                WindowRequest::DragMove => {
                    let _ = g.window.drag_window();
                }
                WindowRequest::DragResize(e) => {
                    let d = match e {
                        ResizeEdge::N => ResizeDirection::North,
                        ResizeEdge::S => ResizeDirection::South,
                        ResizeEdge::E => ResizeDirection::East,
                        ResizeEdge::W => ResizeDirection::West,
                        ResizeEdge::NE => ResizeDirection::NorthEast,
                        ResizeEdge::NW => ResizeDirection::NorthWest,
                        ResizeEdge::SE => ResizeDirection::SouthEast,
                        ResizeEdge::SW => ResizeDirection::SouthWest,
                    };
                    let _ = g.window.drag_resize_window(d);
                }
                WindowRequest::Minimize => g.window.set_minimized(true),
                WindowRequest::ToggleMaximize => g.window.set_maximized(!g.window.is_maximized()),
                WindowRequest::Close => el.exit(),
                WindowRequest::SetTitle(t) => g.window.set_title(&t),
                WindowRequest::SetClipboard(_s) =>
                {
                    #[cfg(feature = "clipboard")]
                    if let Some(cb) = &mut self.clipboard {
                        let _ = cb.set_text(_s);
                    }
                }
            }
        }
    }

    fn redraw(&mut self) {
        let now = self.now();
        self.rt.set_time(now);
        let Some(g) = &mut self.gfx else { return };
        let size = g.window.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else { return };
        let scale = g.window.scale_factor() as f32;
        self.rt.maximized = g.window.is_maximized();
        crate::runtime::set_window_info(crate::runtime::WindowInfo { maximized: self.rt.maximized, focused: true });
        self.rt.resize(Size::new(size.width as f32 / scale, size.height as f32 / scale), scale);
        let pm = self.rt.render();
        if g.surface.resize(w, h).is_err() {
            return;
        }
        let Ok(mut buf) = g.surface.buffer_mut() else { return };
        let data = pm.data();
        let pw = pm.width() as usize;
        let ph = pm.height() as usize;
        let (bw, bh) = (w.get() as usize, h.get() as usize);
        for y in 0..bh.min(ph) {
            let row = &data[y * pw * 4..(y * pw + pw.min(bw)) * 4];
            let out = &mut buf[y * bw..y * bw + pw.min(bw)];
            for (o, px) in out.iter_mut().zip(row.chunks_exact(4)) {
                *o = (px[0] as u32) << 16 | (px[1] as u32) << 8 | px[2] as u32;
            }
        }
        let _ = buf.present();
        g.window.set_cursor(map_cursor(self.rt.cursor()));
        self.last_frame = Instant::now();
    }

    fn modifiers(&self) -> Modifiers {
        Modifiers {
            shift: self.mods.shift_key(),
            ctrl: self.mods.control_key(),
            alt: self.mods.alt_key(),
            meta: self.mods.super_key(),
        }
    }
}

impl<A: App> ApplicationHandler for Shell<A> {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title(self.opts.title.clone())
            .with_inner_size(LogicalSize::new(self.opts.width as f64, self.opts.height as f64))
            .with_min_inner_size(LogicalSize::new(self.opts.min_width as f64, self.opts.min_height as f64))
            .with_decorations(!self.opts.frameless)
            .with_resizable(self.opts.resizable);
        let window = match el.create_window(attrs) {
            Ok(w) => Rc::new(w),
            Err(e) => {
                self.error = Some(e.to_string());
                el.exit();
                return;
            }
        };
        window.set_ime_allowed(true);
        let context = match softbuffer::Context::new(window.clone()) {
            Ok(c) => c,
            Err(e) => {
                self.error = Some(e.to_string());
                el.exit();
                return;
            }
        };
        let surface = match softbuffer::Surface::new(&context, window.clone()) {
            Ok(s) => s,
            Err(e) => {
                self.error = Some(e.to_string());
                el.exit();
                return;
            }
        };
        window.request_redraw();
        self.gfx = Some(Gfx { window, surface, _context: context });
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let now = self.now();
        self.rt.set_time(now);
        let scale = self.gfx.as_ref().map(|g| g.window.scale_factor() as f32).unwrap_or(1.0);
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::RedrawRequested => self.redraw(),
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                self.rt.invalidate();
                // Redraw synchronously for smooth live resizing.
                self.redraw();
            }
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::CursorMoved { position, .. } => {
                self.rt.handle(Event::PointerMove(Point::new(position.x as f32 / scale, position.y as f32 / scale)));
            }
            WindowEvent::CursorLeft { .. } => self.rt.handle(Event::PointerLeave),
            WindowEvent::MouseInput { state, button, .. } => {
                let b = match button {
                    winit::event::MouseButton::Left => MouseButton::Left,
                    winit::event::MouseButton::Right => MouseButton::Right,
                    winit::event::MouseButton::Middle => MouseButton::Middle,
                    _ => return,
                };
                // Position comes from the last move event.
                let p = self.rt.pointer_pos().unwrap_or_default();
                self.rt.handle(match state {
                    ElementState::Pressed => Event::PointerDown(p, b),
                    ElementState::Released => Event::PointerUp(p, b),
                });
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let d = match delta {
                    MouseScrollDelta::LineDelta(x, y) => Point::new(-x * 48.0, -y * 48.0),
                    MouseScrollDelta::PixelDelta(p) => Point::new(-p.x as f32 / scale, -p.y as f32 / scale),
                };
                let d = if self.mods.shift_key() && d.x == 0.0 { Point::new(d.y, 0.0) } else { d };
                let p = self.rt.pointer_pos().unwrap_or_default();
                self.rt.handle(Event::Wheel(p, d));
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }
                let mods = self.modifiers();
                let key = map_key(&event.logical_key);
                #[cfg(feature = "clipboard")]
                if mods.command() && key == Key::Char('v') {
                    if let Some(t) = self.clipboard.as_mut().and_then(|c| c.get_text().ok()) {
                        self.rt.handle(Event::Paste(t));
                        self.apply_requests(el);
                        return;
                    }
                }
                self.rt.handle(Event::Key(KeyEvent { key, mods, repeat: event.repeat }));
                if !mods.ctrl && !mods.meta {
                    if let Some(t) = &event.text {
                        self.rt.handle(Event::Text(t.to_string()));
                    }
                }
            }
            WindowEvent::Ime(Ime::Commit(t)) => self.rt.handle(Event::Text(t)),
            WindowEvent::Focused(f) => self.rt.handle(Event::WindowFocus(f)),
            _ => {}
        }
        self.apply_requests(el);
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        let now = self.now();
        self.rt.set_time(now);
        let Some(g) = &self.gfx else { return };
        match self.rt.next_frame() {
            Some(t) if t <= now => {
                // Cap continuous animation at ~120 fps.
                let next = self.last_frame + Duration::from_millis(8);
                if Instant::now() >= next {
                    g.window.request_redraw();
                    el.set_control_flow(ControlFlow::Wait);
                } else {
                    el.set_control_flow(ControlFlow::WaitUntil(next));
                }
            }
            Some(t) => {
                el.set_control_flow(ControlFlow::WaitUntil(self.start + Duration::from_secs_f64(t)));
            }
            None => el.set_control_flow(ControlFlow::Wait),
        }
    }
}
