//! Commands end to end: key bindings and chords in the runtime, keymap
//! overrides shown in menus, the command palette, the key binding
//! recorder, and focusing dock panels by number.

use rust_ui::commands::{pending_chord, CommandPalette, KeymapEditor, KeymapMsg, PaletteMsg};
use rust_ui::prelude::*;
use rust_ui::Event;

struct Ed {
    log: Vec<&'static str>,
    keymap: Keymap,
    palette: CommandPalette,
    keys: KeymapEditor,
    dock: Dock<&'static str>,
    text: String,
}

#[derive(Clone)]
enum Msg {
    Run(&'static str),
    Palette(PaletteMsg),
    Keys(KeymapMsg),
    Focus(usize),
    Dock(DockMsg),
    Text(String),
}

impl App for Ed {
    type Msg = Msg;
    fn update(&mut self, msg: Msg, cx: &mut Cx<Msg>) {
        match msg {
            Msg::Run(s) => self.log.push(s),
            Msg::Palette(m) => {
                if let Some(run) = self.palette.update(m, &self.commands()) {
                    self.update(run, cx);
                }
            }
            Msg::Keys(m) => self.keys.update(m, &mut self.keymap),
            Msg::Focus(n) => {
                if let Some(id) = self.dock.focus_id("d", n) {
                    cx.focus(&id);
                }
            }
            Msg::Dock(m) => self.dock.update(m),
            Msg::Text(t) => self.text = t,
        }
    }
    fn commands(&self) -> Commands<Msg> {
        Commands::new(vec![
            Command::new("file.save", "Save", Msg::Run("save")).category("File").key("Ctrl+S"),
            Command::new("file.saveAll", "Save All", Msg::Run("saveAll")).category("File").key("Ctrl+K S"),
            Command::new("prefs.keys", "Keyboard Shortcuts", Msg::Run("keys"))
                .category("Preferences")
                .key("Ctrl+K Ctrl+S"),
            Command::new("palette", "Command Palette", Msg::Palette(PaletteMsg::Open)).key("Ctrl+Shift+P"),
            Command::new("view.focus1", "Focus Panel 1", Msg::Focus(0)).category("View").key("Ctrl+1"),
            Command::new("view.focus2", "Focus Panel 2", Msg::Focus(1)).category("View").key("Ctrl+2"),
            Command::new("off", "Disabled Thing", Msg::Run("off")).key("F4").enabled(false),
        ])
        .with_keymap(&self.keymap)
    }
    fn menu(&self) -> Vec<Menu<Msg>> {
        let c = self.commands();
        vec![Menu::new("File", vec![c.menu_item("file.save"), c.menu_item("prefs.keys")])]
    }
    fn view(&self) -> Element<Msg> {
        col()
            .size_full()
            .child(menubar(self.menu()))
            .child(text_input(self.text.clone(), Msg::Text).id("field"))
            .child(self.dock.view("d", |t| t.to_string(), |t| text(format!("content {t}")), Msg::Dock).h(300.0))
            .children(self.keys.recording().map(|_| self.keys.view(&self.commands(), &self.keymap, Msg::Keys).h(300.0)))
            .children(self.palette.view(&self.commands(), Msg::Palette))
    }
}

fn setup() -> Headless<Ed> {
    let dock =
        Dock::new(DockNode::hsplit(vec![(1.0, DockNode::tabs(vec!["one"])), (1.0, DockNode::tabs(vec!["two"]))]));
    let app = Ed {
        log: Vec::new(),
        keymap: Keymap::new(),
        palette: CommandPalette::new(),
        keys: KeymapEditor::new(),
        dock,
        text: String::new(),
    };
    let mut h = Headless::new(app, 1000.0, 800.0, 1.0);
    h.settle();
    h
}

fn key(h: &mut Headless<Ed>, key: Key, ctrl: bool, shift: bool) {
    h.event(Event::Key(KeyEvent { key, mods: Modifiers { ctrl, shift, ..Default::default() }, repeat: false }));
    h.settle();
}

fn click_text(h: &mut Headless<Ed>, s: &str) {
    let r = h.rt.rect_of_text(s).unwrap_or_else(|| panic!("no {s:?}"));
    h.click(r.center().x, r.center().y);
    h.settle();
}

#[test]
fn bindings_and_chords() {
    let mut h = setup();
    key(&mut h, Key::Char('s'), true, false);
    assert_eq!(h.rt.app.log, ["save"]);
    // Chord: the first stroke waits (and says so), the second runs.
    key(&mut h, Key::Char('k'), true, false);
    assert!(pending_chord().is_some_and(|p| p.contains('K')), "{:?}", pending_chord());
    key(&mut h, Key::Other, false, false);
    assert!(pending_chord().is_some(), "a lone modifier keeps waiting");
    key(&mut h, Key::Char('s'), true, false);
    assert_eq!(h.rt.app.log, ["save", "keys"]);
    assert_eq!(pending_chord(), None);
    key(&mut h, Key::Char('k'), true, false);
    key(&mut h, Key::Char('s'), false, false);
    assert_eq!(h.rt.app.log, ["save", "keys", "saveAll"]);
    // An unbound second stroke cancels the chord and is swallowed.
    key(&mut h, Key::Char('k'), true, false);
    key(&mut h, Key::Char('x'), false, false);
    assert_eq!(h.rt.app.log.len(), 3);
    assert_eq!(pending_chord(), None);
    // Disabled commands don't run.
    key(&mut h, Key::F(4), false, false);
    assert_eq!(h.rt.app.log.len(), 3);
    // The second stroke of a chord reaches the bindings even in a text field.
    let f = h.rt.rect_of("field").unwrap().center();
    h.click(f.x, f.y);
    key(&mut h, Key::Char('k'), true, false);
    key(&mut h, Key::Char('s'), false, false);
    assert_eq!(h.rt.app.log.last(), Some(&"saveAll"));
    assert_eq!(h.rt.app.text, "", "the 's' didn't go into the field");
}

#[test]
fn keymap_overrides_and_menus() {
    let mut h = setup();
    let label = |s: &str| if cfg!(target_os = "macos") { s.replace("Ctrl+", "⌃") } else { s.to_string() };
    h.rt.app.keymap.set("file.save", vec![KeyBinding::parse("F2").unwrap()]);
    h.rt.invalidate();
    h.settle();
    key(&mut h, Key::Char('s'), true, false);
    assert!(h.rt.app.log.is_empty(), "old binding gone");
    key(&mut h, Key::F(2), false, false);
    assert_eq!(h.rt.app.log, ["save"]);
    // The menu shows the new key, and the chord.
    click_text(&mut h, "File");
    assert!(h.rt.rect_of_text("F2").is_some());
    assert!(h.rt.rect_of_text(&label("Ctrl+K Ctrl+S")).is_some());
}

#[test]
fn palette() {
    let mut h = setup();
    key(&mut h, Key::Char('p'), true, true);
    assert!(h.rt.app.palette.is_open());
    // Focus is in its field: typing filters, ↓ selects, Enter runs.
    h.type_text("save");
    h.settle();
    assert_eq!(h.rt.app.palette.query(), "save");
    assert!(h.rt.rect_of_text("Save All").is_some());
    key(&mut h, Key::Down, false, false);
    key(&mut h, Key::Enter, false, false);
    assert_eq!(h.rt.app.log, ["saveAll"]);
    assert!(!h.rt.app.palette.is_open());
    // Click a row; Escape closes.
    key(&mut h, Key::Char('p'), true, true);
    click_text(&mut h, "Keyboard Shortcuts");
    assert_eq!(h.rt.app.log, ["saveAll", "keys"]);
    key(&mut h, Key::Char('p'), true, true);
    key(&mut h, Key::Escape, false, false);
    assert!(!h.rt.app.palette.is_open());
}

#[test]
fn recorder_captures_keys() {
    let mut h = setup();
    h.rt.send(Msg::Keys(KeymapMsg::Record("prefs.keys".into())));
    h.settle();
    assert!(h.rt.rect_of_text("Change Key Binding").is_some());
    // Ctrl+S is recorded, not run as Save.
    key(&mut h, Key::Char('s'), true, false);
    key(&mut h, Key::Char('s'), true, true);
    assert!(h.rt.app.log.is_empty());
    key(&mut h, Key::Enter, false, false);
    assert_eq!(h.rt.app.keys.recording(), None);
    let c = h.rt.app.commands();
    assert_eq!(c.get("prefs.keys").unwrap().keys[0].to_text(), "Ctrl+S Ctrl+Shift+S");
    // Now in conflict with Save's Ctrl+S (it blocks the chord).
    assert!(!c.conflicts().is_empty());
    // Reset brings the default back.
    h.rt.send(Msg::Keys(KeymapMsg::Reset("prefs.keys".into())));
    h.settle();
    assert_eq!(h.rt.app.commands().get("prefs.keys").unwrap().keys[0].to_text(), "Ctrl+K Ctrl+S");
    // Escape cancels a recording.
    h.rt.send(Msg::Keys(KeymapMsg::Record("file.save".into())));
    h.settle();
    key(&mut h, Key::F(9), false, false);
    key(&mut h, Key::Escape, false, false);
    assert_eq!(h.rt.app.keys.recording(), None);
    assert!(!h.rt.app.keymap.is_customized("file.save"));
}

#[test]
fn focus_panel_n() {
    let mut h = setup();
    key(&mut h, Key::Char('2'), true, false);
    // Focus is on panel 2's tab: Shift+F10 opens that tab's menu.
    key(&mut h, Key::F(10), false, true);
    assert!(h.rt.rect_of_text("Close Others").is_some());
    key(&mut h, Key::Escape, false, false);
    // Enter on the focused tab of panel 1 activates it (and it has focus).
    key(&mut h, Key::Char('1'), true, false);
    key(&mut h, Key::F(10), false, true);
    let menu = h.rt.rect_of_text("Close Others").unwrap();
    let tab1 = h.rt.rect_of_text("one").unwrap();
    assert!((menu.x - tab1.x).abs() < 40.0, "menu under panel 1's tab: {menu:?} vs {tab1:?}");
}
