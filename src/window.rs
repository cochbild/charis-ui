//! Native window shell built on winit + softbuffer.

use std::num::NonZeroU32;
use std::sync::Arc;
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
    /// Window icon as straight RGBA8 pixels (width, height, data).
    pub icon: Option<(u32, u32, Vec<u8>)>,
    /// Extra fonts (TTF/OTF/TTC bytes) loaded before the first frame.
    pub fonts: Vec<Vec<u8>>,
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
            icon: None,
            fonts: Vec::new(),
        }
    }
}

impl WindowOptions {
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), ..Default::default() }
    }
    /// Load a font (TTF/OTF/TTC bytes) before the first frame, e.g.
    /// `include_bytes!("Geist.ttf")`. Use it by family name: set
    /// `ThemeConfig::font` to `FontFamily::Named("Geist".into())`, or
    /// `.font(...)` on any element.
    pub fn font(mut self, data: impl Into<Vec<u8>>) -> Self {
        self.fonts.push(data.into());
        self
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
    /// Set the window icon from straight RGBA8 pixels.
    pub fn icon_rgba(mut self, width: u32, height: u32, rgba: Vec<u8>) -> Self {
        self.icon = Some((width, height, rgba));
        self
    }

    /// Set the window icon from PNG bytes (e.g. `include_bytes!("icon.png")`).
    pub fn icon_png(mut self, png: &[u8]) -> Self {
        if let Ok(pm) = tiny_skia::Pixmap::decode_png(png) {
            let (w, h) = (pm.width(), pm.height());
            let data = pm.take_demultiplied();
            self.icon = Some((w, h, data));
        }
        self
    }

    pub fn frameless(mut self, f: bool) -> Self {
        self.frameless = f;
        self
    }
}

enum Presenter {
    #[cfg(feature = "gpu")]
    Gpu(Box<crate::gpu::GpuSurface>),
    Cpu {
        surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
        _context: softbuffer::Context<Arc<Window>>,
    },
}

struct Gfx {
    window: Arc<Window>,
    presenter: Presenter,
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
    gpu_failed: bool,
    chrome: crate::platform::SharedChrome,
    native_chrome: bool,
    dark: Option<bool>,
    /// Last IME state sent to the OS (enabled, caret area).
    ime: (bool, Option<crate::geometry::Rect>),
    /// Screen reader bridge (created with the window).
    #[cfg(feature = "accessibility")]
    a11y: Option<accesskit_winit::Adapter>,
    #[cfg(feature = "accessibility")]
    a11y_proxy: winit::event_loop::EventLoopProxy<UserEvent>,
}

/// Open a window and run the app until it is closed.
pub fn run<A: App>(app: A, mut opts: WindowOptions) -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    let loop_proxy = event_loop.create_proxy();
    #[cfg(feature = "accessibility")]
    let a11y_proxy = event_loop.create_proxy();
    let mut rt = Runtime::new(app);
    for f in std::mem::take(&mut opts.fonts) {
        rt.load_font(f);
    }
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
        gpu_failed: false,
        chrome: Default::default(),
        native_chrome: false,
        dark: None,
        ime: (false, None),
        #[cfg(feature = "accessibility")]
        a11y: None,
        #[cfg(feature = "accessibility")]
        a11y_proxy,
    };
    shell.rt.set_waker(move || {
        let _ = loop_proxy.send_event(UserEvent::Wake);
    });
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
        match &mut g.presenter {
            #[cfg(feature = "gpu")]
            Presenter::Gpu(gs) => {
                self.rt.render_scene();
                self.rt.with_scene(|scene, text| gs.present(scene, text));
                if !gs.renderer.is_healthy() {
                    // Device lost or GPU error: continue on the CPU renderer.
                    self.gpu_failed = true;
                    self.rt.invalidate();
                }
            }
            Presenter::Cpu { surface, .. } => {
                let pm = self.rt.render();
                if surface.resize(w, h).is_err() {
                    return;
                }
                let Ok(mut buf) = surface.buffer_mut() else { return };
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
            }
        }
        g.window.set_cursor(map_cursor(self.rt.cursor()));
        // IME only while a text input is focused; keep the candidate window at the caret.
        let allowed = self.rt.text_input_focused();
        if allowed != self.ime.0 {
            g.window.set_ime_allowed(allowed);
            self.ime = (allowed, None);
        }
        if allowed {
            let area = self.rt.ime_cursor_area();
            if area != self.ime.1 {
                if let Some(r) = area {
                    g.window.set_ime_cursor_area(
                        winit::dpi::LogicalPosition::new(r.x as f64, r.y as f64),
                        winit::dpi::LogicalSize::new(r.w.max(1.0) as f64, r.h as f64),
                    );
                }
                self.ime.1 = area;
            }
        }
        self.last_frame = Instant::now();
        if self.native_chrome {
            if let Ok(mut m) = self.chrome.lock() {
                *m = self.rt.chrome_map();
            }
        }
        let dark = self.rt.current_theme().dark;
        if self.dark != Some(dark) {
            self.dark = Some(dark);
            crate::platform::set_dark_mode(&g.window, dark);
        }
        #[cfg(feature = "accessibility")]
        self.push_a11y_tree();
        if self.gpu_failed {
            self.gpu_failed = false;
            if let Some(g) = &mut self.gfx {
                if let Ok(p) = Self::create_presenter(&g.window, false) {
                    g.presenter = p;
                    g.window.request_redraw();
                }
            }
        }
    }

    /// GPU by default; falls back to the CPU renderer when no adapter is
    /// available or `RUI_RENDERER=cpu` is set.
    #[cfg_attr(not(feature = "gpu"), allow(unused_variables))]
    fn create_presenter(window: &Arc<Window>, allow_gpu: bool) -> Result<Presenter, String> {
        #[cfg(feature = "gpu")]
        if allow_gpu && std::env::var("RUI_RENDERER").map(|v| v != "cpu").unwrap_or(true) {
            let size = window.inner_size();
            if let Some(gs) = crate::gpu::GpuSurface::new(window.clone(), size.width, size.height) {
                if std::env::var("RUI_PROFILE").is_ok() {
                    eprintln!("rust-ui: GPU renderer on {}", gs.renderer.adapter_name);
                }
                return Ok(Presenter::Gpu(Box::new(gs)));
            }
        }
        let context = softbuffer::Context::new(window.clone()).map_err(|e| e.to_string())?;
        let surface = softbuffer::Surface::new(&context, window.clone()).map_err(|e| e.to_string())?;
        if std::env::var("RUI_PROFILE").is_ok() {
            eprintln!("rust-ui: CPU renderer");
        }
        Ok(Presenter::Cpu { surface, _context: context })
    }

    /// Send the current accessibility tree (a no-op unless a screen reader is active).
    #[cfg(feature = "accessibility")]
    fn push_a11y_tree(&mut self) {
        if let Some(ad) = &mut self.a11y {
            let rt = &mut self.rt;
            ad.update_if_active(|| rt.accessibility_tree());
        }
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

/// Events sent to the event loop from other threads.
#[derive(Debug)]
enum UserEvent {
    /// Background tasks posted messages.
    Wake,
    /// A screen reader connected, disconnected, or requested an action.
    #[cfg(feature = "accessibility")]
    A11y(accesskit_winit::Event),
}

#[cfg(feature = "accessibility")]
impl From<accesskit_winit::Event> for UserEvent {
    fn from(e: accesskit_winit::Event) -> Self {
        UserEvent::A11y(e)
    }
}

impl<A: App> ApplicationHandler<UserEvent> for Shell<A> {
    fn user_event(&mut self, el: &ActiveEventLoop, ev: UserEvent) {
        self.rt.set_time(self.now());
        match ev {
            UserEvent::Wake => self.rt.poll(),
            #[cfg(feature = "accessibility")]
            UserEvent::A11y(e) => {
                use accesskit_winit::WindowEvent as A;
                match e.window_event {
                    A::InitialTreeRequested => self.push_a11y_tree(),
                    A::ActionRequested(req) => {
                        self.rt.accessibility_action(req);
                        if let Some(g) = &self.gfx {
                            g.window.request_redraw();
                        }
                    }
                    A::AccessibilityDeactivated => {}
                }
            }
        }
        self.apply_requests(el);
    }

    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title(self.opts.title.clone())
            .with_inner_size(LogicalSize::new(self.opts.width as f64, self.opts.height as f64))
            .with_min_inner_size(LogicalSize::new(self.opts.min_width as f64, self.opts.min_height as f64))
            // On Windows, frameless windows keep their native styles (for snap,
            // shadow and resizing); the platform layer hides the frame.
            .with_decorations(!self.opts.frameless || cfg!(windows))
            .with_resizable(self.opts.resizable)
            .with_window_icon(
                self.opts.icon.as_ref().and_then(|(w, h, d)| winit::window::Icon::from_rgba(d.clone(), *w, *h).ok()),
            );
        // The accessibility adapter must be attached before the window is first shown.
        #[cfg(feature = "accessibility")]
        let attrs = attrs.with_visible(false);
        let window = match el.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                self.error = Some(e.to_string());
                el.exit();
                return;
            }
        };
        #[cfg(feature = "accessibility")]
        {
            self.a11y = Some(accesskit_winit::Adapter::with_event_loop_proxy(el, &window, self.a11y_proxy.clone()));
            window.set_visible(true);
        }
        // Enabled on demand when a text input gets focus (see `redraw`).
        window.set_ime_allowed(false);
        if self.opts.frameless && crate::platform::install_frameless(&window, self.chrome.clone()) {
            self.native_chrome = true;
            // The OS now handles edge resizing and dragging.
            self.rt.frameless = false;
        }
        let presenter = match Self::create_presenter(&window, true) {
            Ok(p) => p,
            Err(e) => {
                self.error = Some(e);
                el.exit();
                return;
            }
        };
        window.request_redraw();
        self.gfx = Some(Gfx { window, presenter });
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let now = self.now();
        self.rt.set_time(now);
        let scale = self.gfx.as_ref().map(|g| g.window.scale_factor() as f32).unwrap_or(1.0);
        #[cfg(feature = "accessibility")]
        if let (Some(ad), Some(g)) = (&mut self.a11y, &self.gfx) {
            ad.process_event(&g.window, &event);
        }
        match event {
            WindowEvent::CloseRequested => {
                if self.rt.request_close() {
                    el.exit();
                }
            }
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
                if std::env::var("RUI_DEBUG_EVENTS").is_ok() {
                    eprintln!("key {:?} text {:?} state {:?}", event.logical_key, event.text, event.state);
                }
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
            WindowEvent::Ime(ime) => {
                if std::env::var("RUI_DEBUG_EVENTS").is_ok() {
                    eprintln!("ime {ime:?}");
                }
                match ime {
                    Ime::Commit(t) => self.rt.handle(Event::Text(t)),
                    Ime::Preedit(text, cursor) => self.rt.handle(Event::Preedit { text, cursor }),
                    Ime::Disabled => self.rt.handle(Event::Preedit { text: String::new(), cursor: None }),
                    Ime::Enabled => {}
                }
            }
            WindowEvent::Focused(f) => self.rt.handle(Event::WindowFocus(f)),
            _ => {}
        }
        self.apply_requests(el);
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        let now = self.now();
        self.rt.set_time(now);
        // Deliver background messages and due timers.
        self.rt.poll();
        self.apply_requests(el);
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
