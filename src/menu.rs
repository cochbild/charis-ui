//! Application menus and keyboard shortcuts.
//!
//! Declare the menus once in [`App::menu`](crate::App::menu); the same
//! definition gives you:
//! - keyboard shortcuts that work in every window (after a focused text
//!   input has had its chance, so Ctrl+C in a text field still copies);
//! - an in-window menu bar with [`menubar`](crate::widgets::menubar), for
//!   Windows and Linux apps and custom title bars;
//! - the native menu bar on macOS (the global menu, with the standard app
//!   menu added), and optionally a native Win32 menu bar
//!   ([`WindowOptions::native_menu`](crate::WindowOptions::native_menu)).
//!
//! ```
//! # use rust_ui::prelude::*;
//! # #[derive(Clone)] enum Msg { Open, Save, Undo, Wrap(bool) }
//! # struct Editor { wrap: bool }
//! # impl Editor {
//! fn menu(&self) -> Vec<Menu<Msg>> {
//!     vec![
//!         Menu::new("File", vec![
//!             MenuItem::action("Open…", Msg::Open).shortcut("Mod+O"),
//!             MenuItem::action("Save", Msg::Save).shortcut("Mod+S"),
//!         ]),
//!         Menu::new("Edit", vec![MenuItem::action("Undo", Msg::Undo).shortcut("Mod+Z")]),
//!         Menu::new("View", vec![MenuItem::check("Word wrap", self.wrap, Msg::Wrap(!self.wrap)).shortcut("Alt+Z")]),
//!     ]
//! }
//! # }
//! ```

use crate::element::{Key, KeyEvent, Modifiers};

thread_local! {
    static NATIVE_BAR: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// True when the app menu is shown as a native menu bar (then
/// [`menubar`](crate::widgets::menubar) renders nothing, so apps can always
/// include it).
pub fn native_menu_bar() -> bool {
    NATIVE_BAR.with(|b| b.get())
}

#[cfg_attr(not(all(feature = "native-menu", any(target_os = "macos", windows))), allow(dead_code))]
pub(crate) fn set_native_menu_bar(on: bool) {
    NATIVE_BAR.with(|b| b.set(on));
}

/// A keyboard shortcut such as `Ctrl+S`.
///
/// Parse one with [`Shortcut::parse`]: modifiers `Ctrl`, `Shift`, `Alt`
/// (or `Option`), `Cmd` (or `Meta`, `Super`, `Win`) and `Mod` (Cmd on macOS,
/// Ctrl elsewhere, the usual choice for cross-platform shortcuts), then a
/// key: a character, `F1`–`F12`, `Enter`, `Esc`, `Tab`, `Space`, `Delete`,
/// `Backspace`, `Up`, `Down`, `Left`, `Right`, `Home`, `End`, `PageUp`,
/// `PageDown`, or `Plus`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Shortcut {
    pub mods: Modifiers,
    pub key: Key,
}

impl Shortcut {
    pub fn new(mods: Modifiers, key: Key) -> Self {
        Self { mods, key }
    }

    /// Parse `"Mod+Shift+Z"`, `"F5"`, `"Alt+Enter"`… (case-insensitive).
    pub fn parse(s: &str) -> Option<Shortcut> {
        let mut mods = Modifiers::default();
        let parts: Vec<&str> = s.split('+').map(str::trim).collect();
        // "Ctrl++" style: a trailing empty part means the plus key.
        let (key_part, mod_parts) = match parts.as_slice() {
            [rest @ .., "", ""] => ("+", rest),
            [rest @ .., last] => (*last, rest),
            [] => return None,
        };
        for m in mod_parts {
            match m.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => mods.ctrl = true,
                "shift" => mods.shift = true,
                "alt" | "option" | "opt" => mods.alt = true,
                "cmd" | "command" | "meta" | "super" | "win" => mods.meta = true,
                "mod" | "cmdorctrl" | "commandorcontrol" => {
                    if cfg!(target_os = "macos") {
                        mods.meta = true
                    } else {
                        mods.ctrl = true
                    }
                }
                "" => {}
                _ => return None,
            }
        }
        let lower = key_part.to_ascii_lowercase();
        let key = match lower.as_str() {
            "enter" | "return" => Key::Enter,
            "esc" | "escape" => Key::Escape,
            "tab" => Key::Tab,
            "space" => Key::Space,
            "delete" | "del" => Key::Delete,
            "backspace" => Key::Backspace,
            "up" => Key::Up,
            "down" => Key::Down,
            "left" => Key::Left,
            "right" => Key::Right,
            "home" => Key::Home,
            "end" => Key::End,
            "pageup" | "pgup" => Key::PageUp,
            "pagedown" | "pgdn" => Key::PageDown,
            "plus" => Key::Char('+'),
            f if f.len() > 1 && f.starts_with('f') && f[1..].parse::<u8>().is_ok_and(|n| (1..=24).contains(&n)) => {
                Key::F(f[1..].parse().ok()?)
            }
            _ => {
                let mut chars = key_part.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => Key::Char(c.to_ascii_lowercase()),
                    _ => return None,
                }
            }
        };
        Some(Shortcut { mods, key })
    }

    /// Does this key press trigger the shortcut?
    pub fn matches(&self, e: &KeyEvent) -> bool {
        let key_eq = match (&self.key, &e.key) {
            (Key::Char(a), Key::Char(b)) => a.eq_ignore_ascii_case(b),
            (a, b) => a == b,
        };
        key_eq
            && e.mods.ctrl == self.mods.ctrl
            && e.mods.alt == self.mods.alt
            && e.mods.meta == self.mods.meta
            // Shift may be implied by the character itself (e.g. `+`, `?`).
            && (e.mods.shift == self.mods.shift || (!self.mods.shift && matches!(self.key, Key::Char(c) if !c.is_ascii_alphanumeric())))
    }

    /// How the shortcut is shown in menus: `⇧⌘S` on macOS, `Ctrl+Shift+S` elsewhere.
    pub fn label(&self) -> String {
        let key = match &self.key {
            Key::Char(c) => c.to_uppercase().to_string(),
            Key::F(n) => format!("F{n}"),
            Key::Enter => if cfg!(target_os = "macos") { "↩" } else { "Enter" }.into(),
            Key::Escape => "Esc".into(),
            Key::Tab => "Tab".into(),
            Key::Space => "Space".into(),
            Key::Delete => if cfg!(target_os = "macos") { "⌦" } else { "Del" }.into(),
            Key::Backspace => if cfg!(target_os = "macos") { "⌫" } else { "Backspace" }.into(),
            Key::Up => "↑".into(),
            Key::Down => "↓".into(),
            Key::Left => "←".into(),
            Key::Right => "→".into(),
            Key::Home => "Home".into(),
            Key::End => "End".into(),
            Key::PageUp => "PgUp".into(),
            Key::PageDown => "PgDn".into(),
            Key::Other => "?".into(),
        };
        let m = &self.mods;
        if cfg!(target_os = "macos") {
            let mut s = String::new();
            for (on, sym) in [(m.ctrl, "⌃"), (m.alt, "⌥"), (m.shift, "⇧"), (m.meta, "⌘")] {
                if on {
                    s.push_str(sym);
                }
            }
            s + &key
        } else {
            let mut parts = Vec::new();
            for (on, name) in [(m.ctrl, "Ctrl"), (m.meta, "Win"), (m.alt, "Alt"), (m.shift, "Shift")] {
                if on {
                    parts.push(name.to_string());
                }
            }
            parts.push(key);
            parts.join("+")
        }
    }
}

impl std::fmt::Display for Shortcut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(key: Key, ctrl: bool, shift: bool, alt: bool, meta: bool) -> KeyEvent {
        KeyEvent { key, mods: Modifiers { ctrl, shift, alt, meta }, repeat: false }
    }

    #[test]
    fn parse_and_match() {
        let s = Shortcut::parse("Ctrl+Shift+Z").unwrap();
        assert!(s.matches(&ev(Key::Char('Z'), true, true, false, false)));
        assert!(!s.matches(&ev(Key::Char('z'), true, false, false, false)), "shift required");
        assert!(!s.matches(&ev(Key::Char('z'), true, true, true, false)), "extra modifier");
        let f5 = Shortcut::parse("F5").unwrap();
        assert_eq!(f5.key, Key::F(5));
        assert!(f5.matches(&ev(Key::F(5), false, false, false, false)));
        let plus = Shortcut::parse("Ctrl++").unwrap();
        assert_eq!(plus.key, Key::Char('+'));
        assert!(plus.matches(&ev(Key::Char('+'), true, true, false, false)), "shift implied by '+'");
        assert!(Shortcut::parse("Ctrl+Hyper+X").is_none());
        assert!(Shortcut::parse("Ctrl+ab").is_none());
        let m = Shortcut::parse("Mod+S").unwrap();
        assert_eq!(m.mods.ctrl, !cfg!(target_os = "macos"));
        assert_eq!(m.mods.meta, cfg!(target_os = "macos"));
    }

    #[test]
    fn labels() {
        let s = Shortcut::parse("Ctrl+Shift+S").unwrap();
        if cfg!(target_os = "macos") {
            assert_eq!(s.label(), "⌃⇧S");
        } else {
            assert_eq!(s.label(), "Ctrl+Shift+S");
        }
    }
}
