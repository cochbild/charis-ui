//! Native window shell built on winit + softbuffer.

use std::cell::RefCell;
use std::num::NonZeroU32;
use std::rc::Rc;
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
use crate::runtime::{App, Event, MouseButton, ResizeEdge, Runtime, Shared, WindowRequest};
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

/// One OS window: its runtime (UI state) and graphics.
struct Win<A: App> {
    /// `None` for the main window.
    key: Option<String>,
    rt: Runtime<Shared<A>>,
    opts: WindowOptions,
    position: Option<(f32, f32)>,
    /// Sent when the user closes an extra window.
    on_close: Option<A::Msg>,
    gfx: Option<Gfx>,
    mods: ModifiersState,
    last_frame: Instant,
    gpu_failed: bool,
    chrome: crate::platform::SharedChrome,
    native_chrome: bool,
    dark: Option<bool>,
    /// Last IME state sent to the OS (enabled, caret area).
    ime: (bool, Option<crate::geometry::Rect>),
    /// Screen reader bridge (created with the window).
    #[cfg(feature = "accessibility")]
    a11y: Option<accesskit_winit::Adapter>,
}

struct Shell<A: App> {
    app: Rc<RefCell<A>>,
    /// Open windows; the main window is first.
    wins: Vec<Win<A>>,
    fonts: Vec<Vec<u8>>,
    start: Instant,
    #[cfg(feature = "clipboard")]
    clipboard: Option<arboard::Clipboard>,
    error: Option<String>,
    proxy: winit::event_loop::EventLoopProxy<UserEvent>,
    /// The main window closed; the event loop is exiting.
    exiting: bool,
}

/// Open a window and run the app until it is closed. Extra windows declared
/// by [`App::windows`] open and close as the app's state changes.
pub fn run<A: App>(app: A, mut opts: WindowOptions) -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    let fonts = std::mem::take(&mut opts.fonts);
    let mut shell = Shell {
        app: Rc::new(RefCell::new(app)),
        wins: Vec::new(),
        fonts,
        start: Instant::now(),
        #[cfg(feature = "clipboard")]
        clipboard: arboard::Clipboard::new().ok(),
        error: None,
        proxy,
        exiting: false,
    };
    let main = shell.new_win(None, opts, None, None);
    shell.wins.push(main);
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

    fn new_win(
        &self,
        key: Option<String>,
        opts: WindowOptions,
        position: Option<(f32, f32)>,
        on_close: Option<A::Msg>,
    ) -> Win<A> {
        let mut rt = Runtime::new(Shared::new(self.app.clone(), key.clone()));
        for f in &self.fonts {
            rt.load_font(f.clone());
        }
        rt.frameless = opts.frameless;
        let proxy = self.proxy.clone();
        rt.set_waker(move || {
            let _ = proxy.send_event(UserEvent::Wake);
        });
        rt.set_time(self.now());
        Win {
            key,
            rt,
            opts,
            position,
            on_close,
            gfx: None,
            mods: ModifiersState::empty(),
            last_frame: Instant::now(),
            gpu_failed: false,
            chrome: Default::default(),
            native_chrome: false,
            dark: None,
            ime: (false, None),
            #[cfg(feature = "accessibility")]
            a11y: None,
        }
    }

    fn index_of(&self, id: WindowId) -> Option<usize> {
        self.wins.iter().position(|w| w.gfx.as_ref().is_some_and(|g| g.window.id() == id))
    }

    /// Create the OS window for `wins[i]`.
    fn open(&mut self, el: &ActiveEventLoop, i: usize) {
        let w = &mut self.wins[i];
        if w.gfx.is_some() {
            return;
        }
        let o = &w.opts;
        let mut attrs = Window::default_attributes()
            .with_title(o.title.clone())
            .with_inner_size(LogicalSize::new(o.width as f64, o.height as f64))
            .with_min_inner_size(LogicalSize::new(o.min_width as f64, o.min_height as f64))
            // On Windows, frameless windows keep their native styles (for snap,
            // shadow and resizing); the platform layer hides the frame.
            .with_decorations(!o.frameless || cfg!(windows))
            .with_resizable(o.resizable)
            .with_window_icon(
                o.icon.as_ref().and_then(|(w, h, d)| winit::window::Icon::from_rgba(d.clone(), *w, *h).ok()),
            );
        if let Some((x, y)) = w.position {
            attrs = attrs.with_position(winit::dpi::LogicalPosition::new(x as f64, y as f64));
        }
        // The accessibility adapter must be attached before the window is first shown.
        #[cfg(feature = "accessibility")]
        let attrs = attrs.with_visible(false);
        let window = match el.create_window(attrs) {
            Ok(win) => Arc::new(win),
            Err(e) => {
                if w.key.is_none() {
                    self.error = Some(e.to_string());
                    el.exit();
                }
                return;
            }
        };
        #[cfg(feature = "accessibility")]
        {
            w.a11y = Some(accesskit_winit::Adapter::with_event_loop_proxy(el, &window, self.proxy.clone()));
            window.set_visible(true);
        }
        // Enabled on demand when a text input gets focus (see `redraw`).
        window.set_ime_allowed(false);
        if w.opts.frameless && crate::platform::install_frameless(&window, w.chrome.clone()) {
            w.native_chrome = true;
            // The OS now handles edge resizing and dragging.
            w.rt.frameless = false;
        }
        let presenter = match create_presenter(&window, true) {
            Ok(p) => p,
            Err(e) => {
                if w.key.is_none() {
                    self.error = Some(e);
                    el.exit();
                }
                return;
            }
        };
        window.request_redraw();
        w.gfx = Some(Gfx { window, presenter });
    }

    /// After app updates: re-render every window, and open or close extra
    /// windows to match [`App::windows`].
    fn sync(&mut self, el: &ActiveEventLoop) {
        self.sync_with(el, false);
    }

    fn sync_with(&mut self, el: &ActiveEventLoop, force: bool) {
        let mut updated = force;
        for w in &mut self.wins {
            updated |= w.rt.take_updated();
        }
        if !updated {
            return;
        }
        for w in &mut self.wins {
            w.rt.invalidate();
            if let Some(g) = &w.gfx {
                g.window.request_redraw();
            }
        }
        let specs = self.app.borrow().windows();
        // Close windows the app no longer declares (dropping one closes it).
        self.wins.retain(|w| w.key.is_none() || specs.iter().any(|s| Some(&s.key) == w.key.as_ref()));
        for spec in specs {
            match self.wins.iter_mut().find(|w| w.key.as_ref() == Some(&spec.key)) {
                Some(w) => {
                    if w.opts.title != spec.title {
                        w.opts.title = spec.title.clone();
                        if let Some(g) = &w.gfx {
                            g.window.set_title(&spec.title);
                        }
                    }
                    w.on_close = Some(spec.on_close);
                }
                None => {
                    let opts = WindowOptions {
                        title: spec.title.clone(),
                        width: spec.width,
                        height: spec.height,
                        min_width: spec.min_width,
                        min_height: spec.min_height,
                        frameless: spec.frameless,
                        ..WindowOptions::default()
                    };
                    let win = self.new_win(Some(spec.key.clone()), opts, spec.position, Some(spec.on_close));
                    self.wins.push(win);
                    let i = self.wins.len() - 1;
                    self.open(el, i);
                }
            }
        }
    }

    /// The user (or the app, through `Cx::close_window`) closes window `i`.
    fn close(&mut self, el: &ActiveEventLoop, i: usize) {
        if i == 0 {
            if self.wins[0].rt.request_close() {
                self.exiting = true;
                el.exit();
            }
        } else if let Some(m) = self.wins[i].on_close.clone() {
            self.wins[i].rt.send(m);
            self.wins[i].rt.poll();
        }
    }

    fn apply_requests(&mut self, el: &ActiveEventLoop) {
        for i in 0..self.wins.len() {
            let requests = self.wins[i].rt.take_requests();
            for r in requests {
                let Some(g) = &self.wins[i].gfx else { continue };
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
                    WindowRequest::Close => {
                        if i == 0 {
                            self.exiting = true;
                            el.exit();
                        } else {
                            self.close(el, i);
                        }
                    }
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
        self.sync(el);
    }

    fn redraw(&mut self, i: usize) {
        let now = self.now();
        let w = &mut self.wins[i];
        w.rt.set_time(now);
        let Some(g) = &mut w.gfx else { return };
        let size = g.window.inner_size();
        let (Some(pw), Some(ph)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else { return };
        let scale = g.window.scale_factor() as f32;
        w.rt.maximized = g.window.is_maximized();
        crate::runtime::set_window_info(crate::runtime::WindowInfo { maximized: w.rt.maximized, focused: true });
        w.rt.resize(Size::new(size.width as f32 / scale, size.height as f32 / scale), scale);
        match &mut g.presenter {
            #[cfg(feature = "gpu")]
            Presenter::Gpu(gs) => {
                w.rt.render_scene();
                w.rt.with_scene(|scene, text| gs.present(scene, text));
                if !gs.renderer.is_healthy() {
                    // Device lost or GPU error: continue on the CPU renderer.
                    w.gpu_failed = true;
                    w.rt.invalidate();
                }
            }
            Presenter::Cpu { surface, .. } => {
                let pm = w.rt.render();
                if surface.resize(pw, ph).is_err() {
                    return;
                }
                let Ok(mut buf) = surface.buffer_mut() else { return };
                let data = pm.data();
                let pmw = pm.width() as usize;
                let pmh = pm.height() as usize;
                let (bw, bh) = (pw.get() as usize, ph.get() as usize);
                for y in 0..bh.min(pmh) {
                    let row = &data[y * pmw * 4..(y * pmw + pmw.min(bw)) * 4];
                    let out = &mut buf[y * bw..y * bw + pmw.min(bw)];
                    for (o, px) in out.iter_mut().zip(row.chunks_exact(4)) {
                        *o = (px[0] as u32) << 16 | (px[1] as u32) << 8 | px[2] as u32;
                    }
                }
                let _ = buf.present();
            }
        }
        g.window.set_cursor(map_cursor(w.rt.cursor()));
        // IME only while a text input is focused; keep the candidate window at the caret.
        let allowed = w.rt.text_input_focused();
        if allowed != w.ime.0 {
            g.window.set_ime_allowed(allowed);
            w.ime = (allowed, None);
        }
        if allowed {
            let area = w.rt.ime_cursor_area();
            if area != w.ime.1 {
                if let Some(r) = area {
                    g.window.set_ime_cursor_area(
                        winit::dpi::LogicalPosition::new(r.x as f64, r.y as f64),
                        winit::dpi::LogicalSize::new(r.w.max(1.0) as f64, r.h as f64),
                    );
                }
                w.ime.1 = area;
            }
        }
        w.last_frame = Instant::now();
        if w.native_chrome {
            if let Ok(mut m) = w.chrome.lock() {
                *m = w.rt.chrome_map();
            }
        }
        let dark = w.rt.current_theme().dark;
        if w.dark != Some(dark) {
            w.dark = Some(dark);
            crate::platform::set_dark_mode(&g.window, dark);
        }
        #[cfg(feature = "accessibility")]
        push_a11y_tree(w);
        if w.gpu_failed {
            w.gpu_failed = false;
            if let Some(g) = &mut w.gfx {
                if let Ok(p) = create_presenter(&g.window, false) {
                    g.presenter = p;
                    g.window.request_redraw();
                }
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
fn push_a11y_tree<A: App>(w: &mut Win<A>) {
    if let Some(ad) = &mut w.a11y {
        let rt = &mut w.rt;
        ad.update_if_active(|| rt.accessibility_tree());
    }
}

fn modifiers(m: ModifiersState) -> Modifiers {
    Modifiers { shift: m.shift_key(), ctrl: m.control_key(), alt: m.alt_key(), meta: m.super_key() }
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
        let now = self.now();
        match ev {
            UserEvent::Wake => {
                for w in &mut self.wins {
                    w.rt.set_time(now);
                    w.rt.poll();
                }
            }
            #[cfg(feature = "accessibility")]
            UserEvent::A11y(e) => {
                use accesskit_winit::WindowEvent as A;
                if let Some(i) = self.index_of(e.window_id) {
                    let w = &mut self.wins[i];
                    w.rt.set_time(now);
                    match e.window_event {
                        A::InitialTreeRequested => push_a11y_tree(w),
                        A::ActionRequested(req) => {
                            w.rt.accessibility_action(req);
                            if let Some(g) = &w.gfx {
                                g.window.request_redraw();
                            }
                        }
                        A::AccessibilityDeactivated => {}
                    }
                }
            }
        }
        self.apply_requests(el);
    }

    fn resumed(&mut self, el: &ActiveEventLoop) {
        for i in 0..self.wins.len() {
            self.open(el, i);
        }
        // Windows the app declares from the start (e.g. a restored layout).
        self.sync_with(el, true);
    }

    fn window_event(&mut self, el: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(i) = self.index_of(id) else { return };
        let now = self.now();
        {
            let w = &mut self.wins[i];
            w.rt.set_time(now);
            #[cfg(feature = "accessibility")]
            if let (Some(ad), Some(g)) = (&mut w.a11y, &w.gfx) {
                ad.process_event(&g.window, &event);
            }
        }
        let scale = self.wins[i].gfx.as_ref().map(|g| g.window.scale_factor() as f32).unwrap_or(1.0);
        match event {
            WindowEvent::CloseRequested => self.close(el, i),
            WindowEvent::RedrawRequested => self.redraw(i),
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                self.wins[i].rt.invalidate();
                // Redraw synchronously for smooth live resizing.
                self.redraw(i);
            }
            WindowEvent::ModifiersChanged(m) => self.wins[i].mods = m.state(),
            WindowEvent::CursorMoved { position, .. } => {
                self.wins[i]
                    .rt
                    .handle(Event::PointerMove(Point::new(position.x as f32 / scale, position.y as f32 / scale)));
            }
            WindowEvent::CursorLeft { .. } => self.wins[i].rt.handle(Event::PointerLeave),
            WindowEvent::MouseInput { state, button, .. } => {
                let b = match button {
                    winit::event::MouseButton::Left => MouseButton::Left,
                    winit::event::MouseButton::Right => MouseButton::Right,
                    winit::event::MouseButton::Middle => MouseButton::Middle,
                    _ => return,
                };
                let rt = &mut self.wins[i].rt;
                // Position comes from the last move event.
                let p = rt.pointer_pos().unwrap_or_default();
                rt.handle(match state {
                    ElementState::Pressed => Event::PointerDown(p, b),
                    ElementState::Released => Event::PointerUp(p, b),
                });
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let w = &mut self.wins[i];
                let d = match delta {
                    MouseScrollDelta::LineDelta(x, y) => Point::new(-x * 48.0, -y * 48.0),
                    MouseScrollDelta::PixelDelta(p) => Point::new(-p.x as f32 / scale, -p.y as f32 / scale),
                };
                let d = if w.mods.shift_key() && d.x == 0.0 { Point::new(d.y, 0.0) } else { d };
                let p = w.rt.pointer_pos().unwrap_or_default();
                w.rt.handle(Event::Wheel(p, d));
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if std::env::var("RUI_DEBUG_EVENTS").is_ok() {
                    eprintln!("key {:?} text {:?} state {:?}", event.logical_key, event.text, event.state);
                }
                if event.state != ElementState::Pressed {
                    return;
                }
                let mods = modifiers(self.wins[i].mods);
                let key = map_key(&event.logical_key);
                #[cfg(feature = "clipboard")]
                if mods.command() && key == Key::Char('v') {
                    if let Some(t) = self.clipboard.as_mut().and_then(|c| c.get_text().ok()) {
                        self.wins[i].rt.handle(Event::Paste(t));
                        self.apply_requests(el);
                        return;
                    }
                }
                let rt = &mut self.wins[i].rt;
                rt.handle(Event::Key(KeyEvent { key, mods, repeat: event.repeat }));
                if !mods.ctrl && !mods.meta {
                    if let Some(t) = &event.text {
                        rt.handle(Event::Text(t.to_string()));
                    }
                }
            }
            WindowEvent::Ime(ime) => {
                if std::env::var("RUI_DEBUG_EVENTS").is_ok() {
                    eprintln!("ime {ime:?}");
                }
                let rt = &mut self.wins[i].rt;
                match ime {
                    Ime::Commit(t) => rt.handle(Event::Text(t)),
                    Ime::Preedit(text, cursor) => rt.handle(Event::Preedit { text, cursor }),
                    Ime::Disabled => rt.handle(Event::Preedit { text: String::new(), cursor: None }),
                    Ime::Enabled => {}
                }
            }
            WindowEvent::Focused(f) => {
                if f {
                    // OS accessibility settings may have changed while away.
                    if crate::system::system_prefs() != crate::system::refresh_system_prefs() {
                        for w in &mut self.wins {
                            w.rt.invalidate();
                            if let Some(g) = &w.gfx {
                                g.window.request_redraw();
                            }
                        }
                    }
                }
                self.wins[i].rt.handle(Event::WindowFocus(f))
            }
            _ => {}
        }
        self.apply_requests(el);
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if self.exiting {
            return;
        }
        let now = self.now();
        // Deliver background messages and due timers.
        for w in &mut self.wins {
            w.rt.set_time(now);
            w.rt.poll();
        }
        self.apply_requests(el);
        let mut wait: Option<Instant> = None;
        let mut soonest = |t: Instant| wait = Some(wait.map_or(t, |w: Instant| w.min(t)));
        for w in &self.wins {
            let Some(g) = &w.gfx else { continue };
            match w.rt.next_frame() {
                Some(t) if t <= now => {
                    // Cap continuous animation at ~120 fps.
                    let next = w.last_frame + Duration::from_millis(8);
                    if Instant::now() >= next {
                        g.window.request_redraw();
                    } else {
                        soonest(next);
                    }
                }
                Some(t) => soonest(self.start + Duration::from_secs_f64(t)),
                None => {}
            }
        }
        el.set_control_flow(match wait {
            Some(t) => ControlFlow::WaitUntil(t),
            None => ControlFlow::Wait,
        });
    }
}
