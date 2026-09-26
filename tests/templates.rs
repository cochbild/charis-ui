//! The `cargo generate` templates in `templates/` compile against this
//! version of the crate and work headlessly.

#[path = "../templates/ide-shell/src/main.rs"]
#[allow(dead_code)]
mod ide_shell;

use rust_ui::prelude::*;

fn click_text<A: App>(h: &mut Headless<A>, s: &str) {
    let r = h.rt.rect_of_text(s).unwrap_or_else(|| panic!("no text {s:?}"));
    h.click(r.center().x, r.center().y);
    h.settle();
}

#[test]
fn ide_shell_opens_files_from_the_project_tree() {
    let mut h = Headless::new(ide_shell::Shell::default(), 1280.0, 800.0, 1.0);
    h.settle();
    assert!(h.rt.rect_of_text("main.rs").is_some(), "the default editor tab");
    // Double-click a file in the project tree to open it in the dock.
    let r = h.rt.rect_of_text("app.rs").expect("app.rs in the tree");
    h.click(r.center().x, r.center().y);
    h.click(r.center().x, r.center().y);
    h.settle();
    let open: Vec<_> = h.rt.app.dock.main.groups().iter().flat_map(|g| g.tabs.clone()).collect();
    assert!(open.contains(&ide_shell::Panel::Editor("src/app.rs".into())), "{open:?}");
    if let Some(dir) = std::env::var_os("TEMPLATE_SHOTS") {
        h.save_png(std::path::Path::new(&dir).join("ide-shell.png")).unwrap();
    }
}

#[test]
fn ide_shell_palette_runs_commands() {
    let mut h = Headless::new(ide_shell::Shell::default(), 1280.0, 800.0, 1.0);
    h.settle();
    h.rt.send(ide_shell::Msg::Palette(rust_ui::commands::PaletteMsg::Open));
    h.settle();
    h.type_text("dark theme");
    h.settle();
    h.event(rust_ui::Event::Key(KeyEvent { key: Key::Enter, mods: Modifiers::default(), repeat: false }));
    h.settle();
    assert!(!h.rt.app.dark, "the palette ran Dark Theme");
    click_text(&mut h, "View");
    assert!(h.rt.rect_of_text("Command Palette…").is_some());
}

#[path = "../templates/settings-app/src/main.rs"]
#[allow(dead_code)]
mod settings_app;

fn click_id<A: App>(h: &mut Headless<A>, id: &str) {
    let r = h.rt.rect_of(id).unwrap_or_else(|| panic!("no element {id:?}"));
    h.click(r.center().x, r.center().y);
    h.settle();
}

#[test]
fn settings_app_edits_saves_and_reloads() {
    let dir = std::env::temp_dir().join(format!("rui-settings-{}", std::process::id()));
    let path = dir.join("settings.txt");
    let _ = std::fs::remove_file(&path);

    let mut h = Headless::new(settings_app::Prefs::new(Some(path.clone())), 960.0, 680.0, 1.0);
    h.settle();
    click_id(&mut h, "display-name");
    h.type_text("Ada");
    click_id(&mut h, "check-updates");
    assert_eq!(h.rt.app.settings.display_name, "Ada");
    assert!(!h.rt.app.settings.check_updates);

    // The theme follows the settings live.
    click_text(&mut h, "Appearance");
    click_text(&mut h, "Light");
    assert_eq!(h.rt.app.settings.mode, 1);
    assert!(h.pixel(600.0, 10.0)[0] > 200, "light background");
    if let Some(dir) = std::env::var_os("TEMPLATE_SHOTS") {
        h.save_png(std::path::Path::new(&dir).join("settings-app.png")).unwrap();
    }

    // Saved on every change, and read back on start.
    let again = settings_app::Prefs::new(Some(path.clone()));
    assert_eq!(again.settings, h.rt.app.settings);

    // Reset asks first.
    click_text(&mut h, "Advanced");
    click_id(&mut h, "reset");
    assert!(h.rt.app.confirm_reset);
    click_id(&mut h, "confirm-reset");
    assert_eq!(h.rt.app.settings, settings_app::Settings::default());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn settings_app_search_filters_sections() {
    let mut h = Headless::new(settings_app::Prefs::new(None), 960.0, 680.0, 1.0);
    h.settle();
    click_id(&mut h, "search");
    h.type_text("accent");
    h.settle();
    assert_eq!(h.rt.app.section, settings_app::Section::Appearance);
    assert!(h.rt.rect_of_text("Notifications").is_none());
}

#[test]
fn settings_text_round_trips_and_tolerates_junk() {
    let s =
        settings_app::Settings { display_name: "Grace = Hopper".into(), radius: 11.0, accent: 3, ..Default::default() };
    assert_eq!(settings_app::Settings::from_text(&s.to_text()), s);
    let junk = settings_app::Settings::from_text("radius = lots\naccent = 999\nunknown = 1\nno equals sign\n");
    assert_eq!(junk.radius, settings_app::Settings::default().radius);
    assert_eq!(junk.accent, Accent::ALL.len() - 1);
}
