//! The app menu ([`App::menu`](crate::App::menu)) as a native menu bar, via
//! muda: the global menu bar on macOS, a Win32 menu bar on Windows.

use std::hash::{Hash, Hasher};

use muda::accelerator::{Accelerator, Code, Modifiers as MMods};
use muda::{CheckMenuItem, IsMenuItem, MenuId, MenuItem as MItem, PredefinedMenuItem, Submenu};

use crate::commands::KeyBinding;
use crate::element::Key;
use crate::fxhash::FxHashMap;
use crate::widgets::{Menu, MenuItem};

/// A built native menu and the messages of its items.
pub(crate) struct NativeMenu<M> {
    pub menu: muda::Menu,
    pub msgs: FxHashMap<MenuId, M>,
    /// Hash of what it was built from, to rebuild only on changes.
    pub sig: u64,
}

/// Hash of everything a native menu shows.
pub(crate) fn signature<M>(menus: &[Menu<M>]) -> u64 {
    fn items<M>(its: &[MenuItem<M>], h: &mut impl Hasher) {
        for it in its {
            match it {
                MenuItem::Action { label, shortcut, disabled, .. } => (0u8, label, shortcut, disabled).hash(h),
                MenuItem::Check { label, checked, shortcut, disabled, .. } => {
                    (1u8, label, checked, shortcut, disabled).hash(h)
                }
                MenuItem::Submenu { label, items: sub, disabled } => {
                    (2u8, label, disabled).hash(h);
                    items(sub, h);
                    3u8.hash(h);
                }
                MenuItem::Separator => 4u8.hash(h),
                MenuItem::Header(l) => (5u8, l).hash(h),
            }
        }
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for m in menus {
        m.title.hash(&mut h);
        items(&m.items, &mut h);
    }
    h.finish()
}

fn code(k: &Key) -> Option<Code> {
    Some(match k {
        Key::Char(c) => match c.to_ascii_lowercase() {
            'a' => Code::KeyA,
            'b' => Code::KeyB,
            'c' => Code::KeyC,
            'd' => Code::KeyD,
            'e' => Code::KeyE,
            'f' => Code::KeyF,
            'g' => Code::KeyG,
            'h' => Code::KeyH,
            'i' => Code::KeyI,
            'j' => Code::KeyJ,
            'k' => Code::KeyK,
            'l' => Code::KeyL,
            'm' => Code::KeyM,
            'n' => Code::KeyN,
            'o' => Code::KeyO,
            'p' => Code::KeyP,
            'q' => Code::KeyQ,
            'r' => Code::KeyR,
            's' => Code::KeyS,
            't' => Code::KeyT,
            'u' => Code::KeyU,
            'v' => Code::KeyV,
            'w' => Code::KeyW,
            'x' => Code::KeyX,
            'y' => Code::KeyY,
            'z' => Code::KeyZ,
            '0' => Code::Digit0,
            '1' => Code::Digit1,
            '2' => Code::Digit2,
            '3' => Code::Digit3,
            '4' => Code::Digit4,
            '5' => Code::Digit5,
            '6' => Code::Digit6,
            '7' => Code::Digit7,
            '8' => Code::Digit8,
            '9' => Code::Digit9,
            ',' => Code::Comma,
            '.' => Code::Period,
            '/' => Code::Slash,
            ';' => Code::Semicolon,
            '\'' => Code::Quote,
            '[' => Code::BracketLeft,
            ']' => Code::BracketRight,
            '\\' => Code::Backslash,
            '-' => Code::Minus,
            '=' | '+' => Code::Equal,
            '`' => Code::Backquote,
            _ => return None,
        },
        Key::F(n) => match n {
            1 => Code::F1,
            2 => Code::F2,
            3 => Code::F3,
            4 => Code::F4,
            5 => Code::F5,
            6 => Code::F6,
            7 => Code::F7,
            8 => Code::F8,
            9 => Code::F9,
            10 => Code::F10,
            11 => Code::F11,
            12 => Code::F12,
            _ => return None,
        },
        Key::Enter => Code::Enter,
        Key::Escape => Code::Escape,
        Key::Tab => Code::Tab,
        Key::Space => Code::Space,
        Key::Delete => Code::Delete,
        Key::Backspace => Code::Backspace,
        Key::Up => Code::ArrowUp,
        Key::Down => Code::ArrowDown,
        Key::Left => Code::ArrowLeft,
        Key::Right => Code::ArrowRight,
        Key::Home => Code::Home,
        Key::End => Code::End,
        Key::PageUp => Code::PageUp,
        Key::PageDown => Code::PageDown,
        Key::Other => return None,
    })
}

/// Native accelerators are single keystrokes: chords are handled by the
/// runtime (and not shown in the native menu).
fn accelerator(s: &Option<KeyBinding>) -> Option<Accelerator> {
    let s = match s.as_ref()?.0.as_slice() {
        [s] => s,
        _ => return None,
    };
    let mut m = MMods::empty();
    for (on, f) in [
        (s.mods.ctrl, MMods::CONTROL),
        (s.mods.shift, MMods::SHIFT),
        (s.mods.alt, MMods::ALT),
        (s.mods.meta, MMods::META),
    ] {
        if on {
            m |= f;
        }
    }
    Some(Accelerator::new(m, code(&s.key)?))
}

struct Builder<M> {
    msgs: FxHashMap<MenuId, M>,
    next: u32,
}

impl<M: Clone> Builder<M> {
    fn id(&mut self) -> MenuId {
        self.next += 1;
        MenuId::new(format!("rust-ui-{}", self.next))
    }

    fn fill(&mut self, sub: &Submenu, items: &[MenuItem<M>]) {
        for it in items {
            let r = match it {
                MenuItem::Action { label, shortcut, msg, disabled, .. } => {
                    let id = self.id();
                    self.msgs.insert(id.clone(), msg.clone());
                    sub.append(&MItem::with_id(id, label, !disabled, accelerator(shortcut)))
                }
                MenuItem::Check { label, checked, msg, shortcut, disabled } => {
                    let id = self.id();
                    self.msgs.insert(id.clone(), msg.clone());
                    sub.append(&CheckMenuItem::with_id(id, label, !disabled, *checked, accelerator(shortcut)))
                }
                MenuItem::Submenu { label, items, disabled } => {
                    let child = Submenu::with_id(self.id(), label, !disabled);
                    self.fill(&child, items);
                    sub.append(&child)
                }
                MenuItem::Separator => sub.append(&PredefinedMenuItem::separator()),
                // Native menus have no headers: a disabled item stands in.
                MenuItem::Header(l) => sub.append(&MItem::with_id(self.id(), l, false, None)),
            };
            if let Err(e) = r {
                debug_assert!(false, "native menu: {e}");
            }
        }
    }
}

/// Build the native menu. On macOS, the standard app menu (About, Services,
/// Hide, Quit…) comes first, as the platform expects.
pub(crate) fn build<M: Clone>(menus: &[Menu<M>], app_name: &str) -> NativeMenu<M> {
    let menu = muda::Menu::new();
    let mut b = Builder { msgs: FxHashMap::default(), next: 0 };
    if cfg!(target_os = "macos") {
        let app = Submenu::new(app_name, true);
        let items: [&dyn IsMenuItem; 9] = [
            &PredefinedMenuItem::about(None, None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::services(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(None),
            &PredefinedMenuItem::hide_others(None),
            &PredefinedMenuItem::show_all(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::quit(None),
        ];
        for i in items {
            let _ = app.append(i);
        }
        let _ = menu.append(&app);
    }
    for m in menus {
        let sub = Submenu::with_id(b.id(), &m.title, true);
        b.fill(&sub, &m.items);
        #[cfg(target_os = "macos")]
        match m.title.as_str() {
            "Window" => sub.set_as_windows_menu_for_nsapp(),
            "Help" => sub.set_as_help_menu_for_nsapp(),
            _ => {}
        }
        let _ = menu.append(&sub);
    }
    NativeMenu { menu, msgs: b.msgs, sig: signature(menus) }
}
