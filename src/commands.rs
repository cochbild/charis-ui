//! Commands and a rebindable keymap (VS Code / JetBrains style).
//!
//! A [`Command`] is a named action (`"file.save"`, "Save") with a message
//! and default key bindings. The app lists its commands in
//! [`App::commands`](crate::App::commands); from there:
//! - their key bindings work in every window, including two-stroke chords
//!   such as `Ctrl+K Ctrl+S` ([`pending_chord`] tells the UI that the
//!   first stroke was pressed);
//! - menus show them with [`Commands::menu_item`], so the menu, the key and
//!   the palette always agree;
//! - users can search and run them from a [`CommandPalette`], and rebind
//!   them with a [`Keymap`] (a list of overrides, serializable with the
//!   `serde` feature) edited in a [`keymap_editor`].
//!
//! ```no_run
//! use rust_ui::prelude::*;
//! use rust_ui::commands::{Command, Commands, Keymap};
//!
//! #[derive(Clone)] enum Msg { Save, Find }
//! struct Editor { keymap: Keymap }
//!
//! impl App for Editor {
//!     type Msg = Msg;
//!     fn update(&mut self, _: Msg, _: &mut Cx<Msg>) {}
//!     fn view(&self) -> Element<Msg> { menubar(self.menu()) }
//!     fn commands(&self) -> Commands<Msg> {
//!         Commands::new(vec![
//!             Command::new("file.save", "Save", Msg::Save).category("File").key("Mod+S"),
//!             Command::new("edit.find", "Find", Msg::Find).category("Edit").key("Mod+F"),
//!         ])
//!         .with_keymap(&self.keymap)
//!     }
//!     fn menu(&self) -> Vec<Menu<Msg>> {
//!         let c = self.commands();
//!         vec![Menu::new("File", vec![c.menu_item("file.save")])]
//!     }
//! }
//! # let _ = Editor { keymap: Keymap::default() };
//! ```

use std::collections::BTreeMap;

use crate::element::*;
use crate::icons::Icon;
use crate::menu::Shortcut;
use crate::semantics::Role;
use crate::style::*;
use crate::theme::theme;
use crate::widgets::{backdrop, button, icon_button, modal, primary_button, text_input, MenuItem};

/// A key binding: one keystroke, or a chord of two (`Ctrl+K Ctrl+S`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyBinding(pub Vec<Shortcut>);

impl KeyBinding {
    /// Parse `"Mod+S"`, `"F5"` or a chord `"Ctrl+K Ctrl+S"` (strokes
    /// separated by spaces; see [`Shortcut::parse`]).
    pub fn parse(s: &str) -> Option<KeyBinding> {
        let strokes: Option<Vec<Shortcut>> = s.split_whitespace().map(Shortcut::parse).collect();
        let strokes = strokes?;
        (!strokes.is_empty() && strokes.len() <= 2).then_some(KeyBinding(strokes))
    }

    /// A single keystroke.
    pub fn single(s: Shortcut) -> Self {
        KeyBinding(vec![s])
    }

    /// How it's shown in menus and lists: `Ctrl+K Ctrl+S`, `⌘K ⌘S` on macOS.
    pub fn label(&self) -> String {
        self.0.iter().map(Shortcut::label).collect::<Vec<_>>().join(" ")
    }

    /// Does this binding start with the keystrokes `keys` (all but possibly
    /// the last of its strokes)?
    fn starts_with(&self, keys: &[KeyEvent]) -> bool {
        keys.len() <= self.0.len() && self.0.iter().zip(keys).all(|(s, k)| s.matches(k))
    }

    /// Is it exactly the keystrokes `keys`?
    fn is(&self, keys: &[KeyEvent]) -> bool {
        keys.len() == self.0.len() && self.starts_with(keys)
    }

    /// The canonical text form, which [`KeyBinding::parse`] reads back on
    /// the same platform (`Ctrl+Shift+S`, `Cmd+K Cmd+S`).
    pub fn to_text(&self) -> String {
        self.0
            .iter()
            .map(|s| {
                let mut parts = Vec::new();
                for (on, name) in
                    [(s.mods.ctrl, "Ctrl"), (s.mods.meta, "Cmd"), (s.mods.alt, "Alt"), (s.mods.shift, "Shift")]
                {
                    if on {
                        parts.push(name.to_string());
                    }
                }
                parts.push(match &s.key {
                    Key::Char('+') => "Plus".into(),
                    Key::Char(' ') | Key::Space => "Space".into(),
                    Key::Char(c) => c.to_uppercase().to_string(),
                    Key::F(n) => format!("F{n}"),
                    Key::Escape => "Esc".into(),
                    Key::PageUp => "PageUp".into(),
                    Key::PageDown => "PageDown".into(),
                    other => format!("{other:?}"),
                });
                parts.join("+")
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl std::fmt::Display for KeyBinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label())
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for KeyBinding {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_text())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for KeyBinding {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        KeyBinding::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("invalid key binding {s:?}")))
    }
}

/// A named action with default key bindings.
#[derive(Debug, Clone)]
pub struct Command<M> {
    /// Stable identifier, used by keymaps (`"view.focusPanel1"`).
    pub id: String,
    pub title: String,
    /// Shown before the title in the palette ("View: Focus Panel 1").
    pub category: Option<String>,
    pub msg: M,
    /// Current key bindings (the defaults, or the keymap's override).
    pub keys: Vec<KeyBinding>,
    /// The bindings the app declared (before any keymap override).
    pub default_keys: Vec<KeyBinding>,
    pub enabled: bool,
    /// For toggles: shown as a check item in menus.
    pub checked: Option<bool>,
}

impl<M> Command<M> {
    pub fn new(id: impl Into<String>, title: impl Into<String>, msg: M) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            category: None,
            msg,
            keys: Vec::new(),
            default_keys: Vec::new(),
            enabled: true,
            checked: None,
        }
    }

    pub fn category(mut self, c: impl Into<String>) -> Self {
        self.category = Some(c.into());
        self
    }

    /// Add a default key binding such as `"Mod+S"` or `"Ctrl+K Ctrl+S"`.
    pub fn key(mut self, binding: &str) -> Self {
        let b = KeyBinding::parse(binding);
        debug_assert!(b.is_some(), "unrecognized key binding {binding:?}");
        if let Some(b) = b {
            self.keys.push(b.clone());
            self.default_keys.push(b);
        }
        self
    }

    pub fn enabled(mut self, e: bool) -> Self {
        self.enabled = e;
        self
    }

    pub fn checked(mut self, c: bool) -> Self {
        self.checked = Some(c);
        self
    }

    /// "Category: Title", or the title.
    pub fn label(&self) -> String {
        match &self.category {
            Some(c) => format!("{c}: {}", self.title),
            None => self.title.clone(),
        }
    }
}

/// What a sequence of keystrokes means.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum KeyMatch<M> {
    /// Run this message.
    Run(M),
    /// The first stroke of a chord: wait for the next key.
    Pending,
    /// Nothing is bound to it.
    None,
}

/// The app's commands; see the [module docs](self).
#[derive(Debug, Clone)]
pub struct Commands<M> {
    list: Vec<Command<M>>,
}

impl<M> Default for Commands<M> {
    fn default() -> Self {
        Self { list: Vec::new() }
    }
}

impl<M> Commands<M> {
    pub fn new(list: Vec<Command<M>>) -> Self {
        Self { list }
    }

    /// Apply a user keymap's overrides.
    pub fn with_keymap(mut self, keymap: &Keymap) -> Self {
        for c in &mut self.list {
            if let Some(keys) = keymap.overrides.get(&c.id) {
                c.keys = keys.clone();
            }
        }
        self
    }

    pub(crate) fn push(&mut self, c: Command<M>) {
        self.list.push(c);
    }

    pub fn get(&self, id: &str) -> Option<&Command<M>> {
        self.list.iter().find(|c| c.id == id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Command<M>> {
        self.list.iter()
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// The first binding of command `id`, as shown in menus.
    pub fn key_label(&self, id: &str) -> Option<String> {
        self.get(id).and_then(|c| c.keys.first()).map(KeyBinding::label)
    }

    /// Bindings used by more than one enabled command, with those commands'
    /// ids. A binding that is a prefix of a chord (`Ctrl+K` and `Ctrl+K
    /// Ctrl+S`) counts too: the chord could never be typed.
    pub fn conflicts(&self) -> Vec<(KeyBinding, Vec<String>)> {
        let mut out: Vec<(KeyBinding, Vec<String>)> = Vec::new();
        let all: Vec<(&KeyBinding, &str)> =
            self.list.iter().flat_map(|c| c.keys.iter().map(move |k| (k, c.id.as_str()))).collect();
        for (i, (a, ida)) in all.iter().enumerate() {
            for (b, idb) in &all[i + 1..] {
                let clash = a == b || a.0.len() != b.0.len() && a.0.iter().zip(&b.0).all(|(x, y)| x == y);
                if clash && ida != idb {
                    let key = if a.0.len() <= b.0.len() { (*a).clone() } else { (*b).clone() };
                    match out.iter_mut().find(|o| o.0 == key) {
                        Some(o) => {
                            for id in [ida, idb] {
                                if !o.1.iter().any(|x| x == id) {
                                    o.1.push(id.to_string());
                                }
                            }
                        }
                        None => out.push((key, vec![ida.to_string(), idb.to_string()])),
                    }
                }
            }
        }
        out
    }

    /// Commands matching a palette query (every word must appear in
    /// "Category: Title" or the id), best first: title prefix matches, then
    /// word starts, then anywhere.
    pub fn search(&self, query: &str) -> Vec<&Command<M>> {
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        let mut hits: Vec<(u8, usize, &Command<M>)> = self
            .list
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                let label = c.label().to_lowercase();
                let title = c.title.to_lowercase();
                let id = c.id.to_lowercase();
                if !words.iter().all(|w| label.contains(w.as_str()) || id.contains(w.as_str())) {
                    return None;
                }
                let first = words.first().map(String::as_str).unwrap_or("");
                let rank = if title.starts_with(first) {
                    0
                } else if title.split_whitespace().any(|t| t.starts_with(first)) {
                    1
                } else {
                    2
                };
                Some((rank, i, c))
            })
            .collect();
        hits.sort_by_key(|h| (h.0, h.1));
        hits.into_iter().map(|h| h.2).collect()
    }
}

impl<M: Clone> Commands<M> {
    /// A menu item for command `id`: its title, key and message, as a check
    /// item for toggles, disabled when the command is. A missing id gives a
    /// disabled item (and a debug assertion).
    pub fn menu_item(&self, id: &str) -> MenuItem<M> {
        let Some(c) = self.get(id) else {
            debug_assert!(false, "unknown command {id:?}");
            return MenuItem::Header(format!("unknown command {id}"));
        };
        let shortcut = c.keys.first().cloned();
        match c.checked {
            Some(checked) => {
                MenuItem::Check { label: c.title.clone(), checked, msg: c.msg.clone(), shortcut, disabled: !c.enabled }
            }
            None => MenuItem::Action {
                label: c.title.clone(),
                shortcut,
                icon: None,
                msg: c.msg.clone(),
                disabled: !c.enabled,
            },
        }
    }

    /// What the keystrokes `keys` (a chord's strokes so far, then the new
    /// one) mean. An exact binding wins over waiting for a chord.
    pub(crate) fn lookup(&self, keys: &[KeyEvent]) -> KeyMatch<M> {
        let enabled = || self.list.iter().filter(|c| c.enabled);
        if let Some(c) = enabled().find(|c| c.keys.iter().any(|k| k.is(keys))) {
            return KeyMatch::Run(c.msg.clone());
        }
        if enabled().any(|c| c.keys.iter().any(|k| k.0.len() > keys.len() && k.starts_with(keys))) {
            return KeyMatch::Pending;
        }
        KeyMatch::None
    }
}

/// The user's key binding overrides, by command id. Commands not in it keep
/// their defaults; an empty list removes a command's bindings.
///
/// With the `serde` feature it serializes as a map of id → list of
/// bindings in text form (`{"file.save": ["Ctrl+Alt+S"]}`).
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct Keymap {
    overrides: BTreeMap<String, Vec<KeyBinding>>,
}

impl Keymap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind command `id` to `keys` (replacing its bindings).
    pub fn set(&mut self, id: impl Into<String>, keys: Vec<KeyBinding>) {
        self.overrides.insert(id.into(), keys);
    }

    /// Add a binding to command `id`, starting from `current` (its bindings
    /// now, e.g. the defaults).
    pub fn add(&mut self, id: &str, current: &[KeyBinding], key: KeyBinding) {
        let mut keys = self.overrides.get(id).cloned().unwrap_or_else(|| current.to_vec());
        if !keys.contains(&key) {
            keys.push(key);
        }
        self.overrides.insert(id.to_string(), keys);
    }

    /// Remove every binding of command `id`.
    pub fn clear(&mut self, id: &str) {
        self.overrides.insert(id.to_string(), Vec::new());
    }

    /// Back to the command's default bindings.
    pub fn reset(&mut self, id: &str) {
        self.overrides.remove(id);
    }

    pub fn is_customized(&self, id: &str) -> bool {
        self.overrides.contains_key(id)
    }

    /// The overridden command ids and their bindings.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &[KeyBinding])> {
        self.overrides.iter().map(|(k, v)| (k.as_str(), v.as_slice()))
    }
}

// ------------------------------------------------------------ command palette

/// Messages of a [`CommandPalette`]. Forward them to [`CommandPalette::update`].
#[derive(Debug, Clone, PartialEq)]
pub enum PaletteMsg {
    Open,
    Close,
    Query(String),
    /// Move the selection by this many rows.
    Move(i32),
    /// Run the command with this id.
    Run(String),
    /// Run the selected command.
    RunSelected,
}

/// A searchable list of every command, with its key (VS Code's
/// Ctrl+Shift+P). Typing filters, ↑/↓ select, Enter runs, Escape closes.
///
/// ```ignore
/// // update:
/// Msg::Palette(m) => {
///     if let Some(run) = self.palette.update(m, &self.commands()) {
///         self.update(run, cx); // the chosen command's message
///     }
/// }
/// // view: .children(self.palette.view(&self.commands(), Msg::Palette))
/// ```
#[derive(Debug, Clone, Default)]
pub struct CommandPalette {
    open: bool,
    query: String,
    selected: usize,
}

/// Rows shown at most (the query narrows the rest).
const PALETTE_ROWS: usize = 60;

impl CommandPalette {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    /// Apply a message; returns the message of the command to run.
    pub fn update<M: Clone>(&mut self, msg: PaletteMsg, cmds: &Commands<M>) -> Option<M> {
        match msg {
            PaletteMsg::Open => {
                self.open = true;
                self.query.clear();
                self.selected = 0;
            }
            PaletteMsg::Close => self.open = false,
            PaletteMsg::Query(q) => {
                self.query = q;
                self.selected = 0;
            }
            PaletteMsg::Move(d) => {
                let n = cmds.search(&self.query).len().min(PALETTE_ROWS);
                if n > 0 {
                    self.selected = (self.selected as i64 + d as i64).rem_euclid(n as i64) as usize;
                }
            }
            PaletteMsg::Run(id) => {
                let c = cmds.get(&id).filter(|c| c.enabled)?;
                self.open = false;
                return Some(c.msg.clone());
            }
            PaletteMsg::RunSelected => {
                let c = cmds.search(&self.query).get(self.selected).filter(|c| c.enabled).map(|c| c.msg.clone())?;
                self.open = false;
                return Some(c);
            }
        }
        None
    }

    /// The palette while it's open.
    pub fn view<M: Clone + 'static>(
        &self,
        cmds: &Commands<M>,
        map: impl Fn(PaletteMsg) -> M + 'static,
    ) -> Option<Element<M>> {
        if !self.open {
            return None;
        }
        let th = theme();
        let c = th.colors.clone();
        let map = std::rc::Rc::new(map);
        let hits = cmds.search(&self.query);
        let (m1, m2) = (map.clone(), map.clone());
        let input = text_input(self.query.clone(), move |q| m1(PaletteMsg::Query(q)))
            .id("command-palette/input")
            .placeholder("Type a command")
            .aria_label("Command palette")
            .autofocus()
            .on_key_capture(move |k| {
                let m = match (&k.key, k.mods.ctrl || k.mods.meta || k.mods.alt) {
                    (Key::Up, false) => PaletteMsg::Move(-1),
                    (Key::Down, false) => PaletteMsg::Move(1),
                    (Key::PageUp, false) => PaletteMsg::Move(-8),
                    (Key::PageDown, false) => PaletteMsg::Move(8),
                    (Key::Enter, false) => PaletteMsg::RunSelected,
                    (Key::Escape, _) => PaletteMsg::Close,
                    _ => return None,
                };
                Some(m2(m))
            });
        let mut list = col().gap(1.0).py(4.0).max_h(360.0).scroll_y().role(Role::List);
        for (i, cmd) in hits.iter().take(PALETTE_ROWS).enumerate() {
            let sel = i == self.selected;
            let mut r = row()
                .key(("palette-row", cmd.id.as_str()))
                .items_center()
                .gap(8.0)
                .h(30.0)
                .px(10.0)
                .rounded(th.radius_sm)
                .role(Role::ListItem)
                .aria_selected(sel)
                .aria_label(cmd.label())
                .cursor(Cursor::Default);
            if let Some(cat) = &cmd.category {
                r = r.child(text(format!("{cat}:")).nowrap().color(if sel { c.text } else { c.text_muted }));
            }
            r = r.child(text(cmd.title.clone()).nowrap().grow(1.0).min_w(0.0));
            r = r.children(cmd.keys.first().map(|k| key_chips(k, sel)));
            if sel {
                r = r.bg(c.accent_soft);
            } else {
                r = r.hover(|s| s.bg(c.hover));
            }
            r = if cmd.enabled { r.on_click(map(PaletteMsg::Run(cmd.id.clone()))) } else { r.opacity(0.45) };
            list = list.child(r);
        }
        if hits.is_empty() {
            list = list.child(text("No matching commands").color(c.text_faint).px(10.0).py(6.0));
        }
        let panel =
            col().fixed().top(pct(12.0)).left(0.0).right(0.0).z_index(100).items_center().pointer_events(false).child(
                col()
                    .pointer_events(true)
                    .w(600.0)
                    .max_w(pct(92.0))
                    .p(6.0)
                    .gap(4.0)
                    .bg(c.elevated)
                    .border(1.0, c.border_strong)
                    .rounded(th.radius_lg)
                    .shadows(th.shadow_popover.clone())
                    .aria_modal()
                    .aria_label("Command palette")
                    .child(input)
                    .child(list),
            );
        Some(div().child(backdrop(map(PaletteMsg::Close), false)).child(panel).class("command-palette"))
    }
}

/// A key binding as keycap chips (`Ctrl` `K`  `Ctrl` `S`).
pub fn key_chips<M: 'static>(k: &KeyBinding, strong: bool) -> Element<M> {
    let th = theme();
    let c = &th.colors;
    let mut r = row().gap(8.0).items_center().shrink(0.0);
    for stroke in &k.0 {
        let label = stroke.label();
        let parts: Vec<String> = if cfg!(target_os = "macos") {
            vec![label]
        } else {
            label.split('+').map(str::to_string).filter(|p| !p.is_empty()).collect()
        };
        let mut s = row().gap(3.0);
        for p in parts {
            s = s.child(
                text(p)
                    .nowrap()
                    .font_size(11.5)
                    .px(5.0)
                    .h(20.0)
                    .items_center()
                    .flex_row()
                    .rounded(4.0)
                    .bg(c.surface)
                    .border(1.0, c.border_strong)
                    .color(if strong { c.text } else { c.text_muted }),
            );
        }
        r = r.child(s);
    }
    r.aria_label(k.label())
}

// ------------------------------------------------------------ keymap editor

/// Messages of a [`KeymapEditor`]. Forward them to [`KeymapEditor::update`].
#[derive(Debug, Clone, PartialEq)]
pub enum KeymapMsg {
    Query(String),
    /// Start recording a new binding for this command.
    Record(String),
    /// A keystroke while recording.
    Stroke(Shortcut),
    /// Save the recorded binding.
    Accept,
    CancelRecord,
    /// Back to the command's default bindings.
    Reset(String),
    /// Remove the command's bindings.
    Clear(String),
}

/// A searchable list of commands and their key bindings, where users
/// record new ones (like VS Code's Keyboard Shortcuts editor). It edits
/// your [`Keymap`]; conflicts are flagged.
#[derive(Debug, Clone, Default)]
pub struct KeymapEditor {
    query: String,
    recording: Option<String>,
    strokes: Vec<Shortcut>,
}

impl KeymapEditor {
    pub fn new() -> Self {
        Self::default()
    }

    /// The command whose binding is being recorded.
    pub fn recording(&self) -> Option<&str> {
        self.recording.as_deref()
    }

    pub fn update(&mut self, msg: KeymapMsg, keymap: &mut Keymap) {
        match msg {
            KeymapMsg::Query(q) => self.query = q,
            KeymapMsg::Record(id) => {
                self.recording = Some(id);
                self.strokes.clear();
            }
            KeymapMsg::Stroke(s) => {
                // Two strokes at most: a third starts over.
                if self.strokes.len() >= 2 {
                    self.strokes.clear();
                }
                self.strokes.push(s);
            }
            KeymapMsg::Accept => {
                if let (Some(id), false) = (self.recording.take(), self.strokes.is_empty()) {
                    keymap.set(id, vec![KeyBinding(std::mem::take(&mut self.strokes))]);
                }
            }
            KeymapMsg::CancelRecord => self.recording = None,
            KeymapMsg::Reset(id) => keymap.reset(&id),
            KeymapMsg::Clear(id) => keymap.clear(&id),
        }
    }

    /// The editor: a search field and one row per command.
    pub fn view<M: Clone + 'static>(
        &self,
        cmds: &Commands<M>,
        keymap: &Keymap,
        map: impl Fn(KeymapMsg) -> M + 'static,
    ) -> Element<M> {
        let th = theme();
        let c = th.colors.clone();
        let map = std::rc::Rc::new(map);
        let conflicts = cmds.conflicts();
        let conflicted = |id: &str| conflicts.iter().any(|(_, ids)| ids.iter().any(|x| x == id));
        let m = map.clone();
        let search = text_input(self.query.clone(), move |q| m(KeymapMsg::Query(q)))
            .id("keymap-editor/search")
            .placeholder("Search commands or keys")
            .aria_label("Search key bindings")
            .shrink(0.0);
        let q = self.query.to_lowercase();
        let mut list = col().grow(1.0).min_h(0.0).scroll_y().role(Role::Table);
        for cmd in cmds.iter() {
            let keys = cmd.keys.iter().map(KeyBinding::label).collect::<Vec<_>>().join(", ");
            if !q.is_empty() && !cmd.label().to_lowercase().contains(&q) && !keys.to_lowercase().contains(&q) {
                continue;
            }
            let custom = keymap.is_customized(&cmd.id);
            let mut binding = row().gap(10.0).items_center().w(230.0).shrink(0.0);
            if cmd.keys.is_empty() {
                binding = binding.child(text("—").color(c.text_faint));
            }
            for k in &cmd.keys {
                binding = binding.child(key_chips(k, true));
            }
            if conflicted(&cmd.id) {
                binding = binding.child(
                    icon(Icon::Warning).font_size(13.0).color(c.warning).tooltip("Also bound to another command"),
                );
            }
            let id = cmd.id.clone();
            let actions = row()
                .gap(2.0)
                .w(170.0)
                .shrink(0.0)
                .justify(Justify::End)
                .child(
                    button("Change")
                        .on_click(map(KeymapMsg::Record(id.clone())))
                        .aria_label(format!("Change key for {}", cmd.label())),
                )
                .child_if(custom, || button("Reset").on_click(map(KeymapMsg::Reset(id.clone()))))
                .child_if(!cmd.keys.is_empty(), || {
                    icon_button(Icon::Close)
                        .tooltip("Remove key binding")
                        .aria_label(format!("Remove key for {}", cmd.label()))
                        .on_click(map(KeymapMsg::Clear(id.clone())))
                });
            list = list.child(
                row()
                    .key(("keymap-row", cmd.id.as_str()))
                    .items_center()
                    .gap(12.0)
                    .h(48.0)
                    .shrink(0.0)
                    .px(12.0)
                    .border_b(1.0, c.border)
                    .role(Role::Row)
                    .hover(|s| s.bg(c.hover))
                    .on_double_click(map(KeymapMsg::Record(cmd.id.clone())))
                    .child(
                        col().grow(1.0).min_w(0.0).child(text(cmd.title.clone()).nowrap()).child(
                            text(match (&cmd.category, custom) {
                                (Some(cat), true) => format!("{cat} · {} · customized", cmd.id),
                                (Some(cat), false) => format!("{cat} · {}", cmd.id),
                                (None, true) => format!("{} · customized", cmd.id),
                                (None, false) => cmd.id.clone(),
                            })
                            .nowrap()
                            .font_size(th.font_size_sm)
                            .color(c.text_faint),
                        ),
                    )
                    .child(binding)
                    .child(actions),
            );
        }
        let mut root = col().grow(1.0).min_h(0.0).gap(8.0).child(search).child(list).class("keymap-editor");
        if let Some(cmd) = self.recording.as_ref().and_then(|id| cmds.get(id)) {
            root = root.child(self.recorder(cmd, cmds, map.clone()));
        }
        root
    }

    fn recorder<M: Clone + 'static>(
        &self,
        cmd: &Command<M>,
        cmds: &Commands<M>,
        map: std::rc::Rc<dyn Fn(KeymapMsg) -> M>,
    ) -> Element<M> {
        let th = theme();
        let c = th.colors.clone();
        let rec = KeyBinding(self.strokes.clone());
        let others: Vec<String> = if self.strokes.is_empty() {
            Vec::new()
        } else {
            cmds.iter()
                .filter(|o| {
                    o.id != cmd.id
                        && o.keys.iter().any(|k| k == &rec || k.0.starts_with(&rec.0) || rec.0.starts_with(&k.0))
                })
                .map(|o| o.label())
                .collect()
        };
        let m = map.clone();
        // The capture box owns the keyboard: every key is recorded, except
        // Enter (accept) and Escape (cancel).
        let capture = col()
            .id("keymap-editor/recorder")
            .autofocus()
            .items_center()
            .justify(Justify::Center)
            .h(56.0)
            .rounded(th.radius)
            .border(1.0, c.accent)
            .bg(c.input)
            .aria_label("Press the keys, then Enter")
            .on_key_capture(move |k| {
                let msg = match &k.key {
                    Key::Enter if k.mods == Default::default() => KeymapMsg::Accept,
                    Key::Escape if k.mods == Default::default() => KeymapMsg::CancelRecord,
                    Key::Other => return None,
                    key => {
                        let key = match key {
                            Key::Char(ch) => Key::Char(ch.to_ascii_lowercase()),
                            other => other.clone(),
                        };
                        KeymapMsg::Stroke(Shortcut::new(k.mods, key))
                    }
                };
                Some(m(msg))
            })
            .child(if self.strokes.is_empty() {
                text("Press the keys…").color(c.text_faint)
            } else {
                key_chips(&rec, true)
            });
        let body = col()
            .gap(10.0)
            .child(text(format!("Press the new key binding for “{}”, then Enter.", cmd.label())))
            .child(capture)
            .child(
                text("A second stroke makes a chord, like Ctrl+K Ctrl+S.")
                    .font_size(th.font_size_sm)
                    .color(c.text_faint),
            )
            .child_if(!others.is_empty(), || {
                text(format!("Also used by: {}", others.join(", "))).font_size(th.font_size_sm).color(c.warning)
            });
        let save = primary_button("Save").disabled(self.strokes.is_empty());
        let save = if self.strokes.is_empty() { save } else { save.on_click(map(KeymapMsg::Accept)) };
        modal(
            "Change Key Binding",
            body,
            vec![button("Cancel").on_click(map(KeymapMsg::CancelRecord)), save],
            map(KeymapMsg::CancelRecord),
        )
    }
}

thread_local! {
    static PENDING: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// The first stroke of a chord that was pressed, waiting for the second
/// (e.g. `"Ctrl+K"`), for a status bar hint like VS Code's "(Ctrl+K) was
/// pressed. Waiting for second key of chord…".
pub fn pending_chord() -> Option<String> {
    PENDING.with(|p| p.borrow().clone())
}

pub(crate) fn set_pending_chord(p: Option<String>) {
    PENDING.with(|x| *x.borrow_mut() = p);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::Modifiers;

    fn ev(key: Key, ctrl: bool, shift: bool) -> KeyEvent {
        KeyEvent { key, mods: Modifiers { ctrl, shift, ..Default::default() }, repeat: false }
    }

    fn cmds() -> Commands<u8> {
        Commands::new(vec![
            Command::new("save", "Save", 1).category("File").key("Ctrl+S"),
            Command::new("saveAll", "Save All", 2).category("File").key("Ctrl+K S"),
            Command::new("shortcuts", "Keyboard Shortcuts", 3).category("Preferences").key("Ctrl+K Ctrl+S"),
            Command::new("off", "Disabled", 4).key("F4").enabled(false),
        ])
    }

    #[test]
    fn bindings_and_chords() {
        let c = cmds();
        let ctrl_k = ev(Key::Char('k'), true, false);
        assert_eq!(c.lookup(&[ev(Key::Char('s'), true, false)]), KeyMatch::Run(1));
        assert_eq!(c.lookup(std::slice::from_ref(&ctrl_k)), KeyMatch::Pending);
        assert_eq!(c.lookup(&[ctrl_k.clone(), ev(Key::Char('s'), true, false)]), KeyMatch::Run(3));
        assert_eq!(c.lookup(&[ctrl_k.clone(), ev(Key::Char('s'), false, false)]), KeyMatch::Run(2));
        assert_eq!(c.lookup(&[ctrl_k, ev(Key::Char('x'), false, false)]), KeyMatch::None);
        assert_eq!(c.lookup(&[ev(Key::F(4), false, false)]), KeyMatch::None, "disabled");
        assert_eq!(KeyBinding::parse("Ctrl+K Ctrl+S").unwrap().to_text(), "Ctrl+K Ctrl+S");
        assert!(KeyBinding::parse("A B C").is_none(), "at most two strokes");
        assert_eq!(c.key_label("save").as_deref(), Some(if cfg!(target_os = "macos") { "⌃S" } else { "Ctrl+S" }));
    }

    #[test]
    fn keymap_overrides() {
        let mut km = Keymap::new();
        km.set("save", vec![KeyBinding::parse("F2").unwrap()]);
        km.clear("saveAll");
        let c = cmds().with_keymap(&km);
        assert_eq!(c.lookup(&[ev(Key::F(2), false, false)]), KeyMatch::Run(1));
        assert_eq!(c.lookup(&[ev(Key::Char('s'), true, false)]), KeyMatch::None, "old binding gone");
        assert!(c.get("saveAll").unwrap().keys.is_empty());
        assert_eq!(c.get("save").unwrap().default_keys, vec![KeyBinding::parse("Ctrl+S").unwrap()]);
        km.reset("save");
        assert!(!km.is_customized("save"));
        // Conflicts: two commands on one key, and a key that blocks a chord.
        let mut km = Keymap::new();
        km.set("shortcuts", vec![KeyBinding::parse("Ctrl+S").unwrap()]);
        km.set("off", vec![KeyBinding::parse("Ctrl+K").unwrap()]);
        let c = cmds().with_keymap(&km);
        let conflicts = c.conflicts();
        assert!(conflicts.iter().any(|(k, ids)| k.to_text() == "Ctrl+S" && ids == &["save", "shortcuts"]));
        assert!(conflicts.iter().any(|(k, ids)| k.to_text() == "Ctrl+K" && ids.contains(&"off".to_string())));
    }

    #[test]
    fn search() {
        let c = cmds();
        let ids = |q: &str| c.search(q).iter().map(|c| c.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids("save"), ["save", "saveAll"]);
        assert_eq!(ids("file all"), ["saveAll"]);
        assert_eq!(ids("short"), ["shortcuts"]);
        assert_eq!(ids("").len(), 4);
    }
}
