//! A settings app built on rust-ui: a sidebar of sections, forms that apply
//! as you change them (the theme updates live), a search box, a reset with
//! confirmation, and settings saved to a small text file in the user's
//! config directory.
//!
//! Start here:
//! - `Settings` holds the values, with `load` and `save`.
//! - `Section` lists the pages; `Prefs::section_view` draws each one.

use std::path::{Path, PathBuf};

use rust_ui::prelude::*;

/// The settings the app stores.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub display_name: String,
    pub language: usize,
    pub open_at_login: bool,
    pub check_updates: bool,

    pub mode: usize, // 0 system, 1 light, 2 dark
    pub accent: usize,
    pub radius: f32,
    pub density: usize,
    pub high_contrast: bool,

    pub notify_messages: bool,
    pub notify_mentions: bool,
    pub notify_sounds: bool,
    pub quiet_hours: bool,
    pub quiet_from: f64,
    pub quiet_to: f64,

    pub cache_mb: f64,
    pub proxy: String,
    pub telemetry: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            display_name: String::new(),
            language: 0,
            open_at_login: false,
            check_updates: true,
            mode: 0,
            accent: 0,
            radius: 6.0,
            density: 1,
            high_contrast: false,
            notify_messages: true,
            notify_mentions: true,
            notify_sounds: false,
            quiet_hours: false,
            quiet_from: 22.0,
            quiet_to: 7.0,
            cache_mb: 512.0,
            proxy: String::new(),
            telemetry: false,
        }
    }
}

pub const LANGUAGES: [&str; 5] = ["English", "Deutsch", "Español", "Français", "日本語"];

impl Settings {
    /// `key = value` lines. Unknown keys and bad values are skipped, so old
    /// and new versions of the app can share a file.
    pub fn from_text(text: &str) -> Settings {
        let mut s = Settings::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            let b = || v == "true";
            let n = || v.parse::<f64>().ok();
            match k {
                "display_name" => s.display_name = v.to_string(),
                "language" => s.language = n().map_or(s.language, |x| (x as usize).min(LANGUAGES.len() - 1)),
                "open_at_login" => s.open_at_login = b(),
                "check_updates" => s.check_updates = b(),
                "mode" => s.mode = n().map_or(s.mode, |x| (x as usize).min(2)),
                "accent" => s.accent = n().map_or(s.accent, |x| (x as usize).min(Accent::ALL.len() - 1)),
                "radius" => s.radius = n().map_or(s.radius, |x| x.clamp(0.0, 16.0) as f32),
                "density" => s.density = n().map_or(s.density, |x| (x as usize).min(2)),
                "high_contrast" => s.high_contrast = b(),
                "notify_messages" => s.notify_messages = b(),
                "notify_mentions" => s.notify_mentions = b(),
                "notify_sounds" => s.notify_sounds = b(),
                "quiet_hours" => s.quiet_hours = b(),
                "quiet_from" => s.quiet_from = n().unwrap_or(s.quiet_from),
                "quiet_to" => s.quiet_to = n().unwrap_or(s.quiet_to),
                "cache_mb" => s.cache_mb = n().unwrap_or(s.cache_mb),
                "proxy" => s.proxy = v.to_string(),
                "telemetry" => s.telemetry = b(),
                _ => {}
            }
        }
        s
    }

    pub fn to_text(&self) -> String {
        let s = self;
        let fields: Vec<(&str, String)> = vec![
            ("display_name", s.display_name.replace('\n', " ")),
            ("language", s.language.to_string()),
            ("open_at_login", s.open_at_login.to_string()),
            ("check_updates", s.check_updates.to_string()),
            ("mode", s.mode.to_string()),
            ("accent", s.accent.to_string()),
            ("radius", s.radius.to_string()),
            ("density", s.density.to_string()),
            ("high_contrast", s.high_contrast.to_string()),
            ("notify_messages", s.notify_messages.to_string()),
            ("notify_mentions", s.notify_mentions.to_string()),
            ("notify_sounds", s.notify_sounds.to_string()),
            ("quiet_hours", s.quiet_hours.to_string()),
            ("quiet_from", s.quiet_from.to_string()),
            ("quiet_to", s.quiet_to.to_string()),
            ("cache_mb", s.cache_mb.to_string()),
            ("proxy", s.proxy.replace('\n', " ")),
            ("telemetry", s.telemetry.to_string()),
        ];
        fields.iter().map(|(k, v)| format!("{k} = {v}\n")).collect()
    }

    pub fn load(path: &Path) -> Settings {
        std::fs::read_to_string(path).map(|t| Settings::from_text(&t)).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, self.to_text())
    }
}

/// Where settings live: the platform's config directory.
pub fn config_path(app: &str) -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    };
    base.map(|b| b.join(app).join("settings.txt"))
}

/// The pages in the sidebar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Section {
    General,
    Appearance,
    Notifications,
    Advanced,
    About,
}

impl Section {
    pub const ALL: [Section; 5] =
        [Section::General, Section::Appearance, Section::Notifications, Section::Advanced, Section::About];

    fn title(self) -> &'static str {
        match self {
            Section::General => "General",
            Section::Appearance => "Appearance",
            Section::Notifications => "Notifications",
            Section::Advanced => "Advanced",
            Section::About => "About",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Section::General => Icon::Settings,
            Section::Appearance => Icon::Sun,
            Section::Notifications => Icon::Bell,
            Section::Advanced => Icon::Code,
            Section::About => Icon::Info,
        }
    }

    /// Words the search box matches, besides the title.
    fn keywords(self) -> &'static str {
        match self {
            Section::General => "name language login startup updates",
            Section::Appearance => "theme dark light mode accent color radius corners density contrast",
            Section::Notifications => "alerts messages mentions sounds quiet hours",
            Section::Advanced => "cache proxy network telemetry reset",
            Section::About => "version license",
        }
    }
}

pub struct Prefs {
    pub settings: Settings,
    /// Where to save; `None` keeps everything in memory (tests).
    pub path: Option<PathBuf>,
    pub section: Section,
    pub search: String,
    pub confirm_reset: bool,
    pub save_error: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Msg {
    Section(Section),
    Search(String),
    Set(Change),
    AskReset(bool),
    Reset,
}

/// One edit to the settings.
#[derive(Clone, Debug)]
pub enum Change {
    DisplayName(String),
    Language(usize),
    OpenAtLogin,
    CheckUpdates,
    Mode(usize),
    Accent(usize),
    Radius(f32),
    Density(usize),
    HighContrast,
    NotifyMessages,
    NotifyMentions,
    NotifySounds,
    QuietHours,
    QuietFrom(f64),
    QuietTo(f64),
    CacheMb(f64),
    Proxy(String),
    Telemetry,
}

impl Settings {
    fn apply(&mut self, c: Change) {
        match c {
            Change::DisplayName(v) => self.display_name = v,
            Change::Language(i) => self.language = i,
            Change::OpenAtLogin => self.open_at_login = !self.open_at_login,
            Change::CheckUpdates => self.check_updates = !self.check_updates,
            Change::Mode(i) => self.mode = i,
            Change::Accent(i) => self.accent = i,
            Change::Radius(r) => self.radius = r.round(),
            Change::Density(i) => self.density = i,
            Change::HighContrast => self.high_contrast = !self.high_contrast,
            Change::NotifyMessages => self.notify_messages = !self.notify_messages,
            Change::NotifyMentions => self.notify_mentions = !self.notify_mentions,
            Change::NotifySounds => self.notify_sounds = !self.notify_sounds,
            Change::QuietHours => self.quiet_hours = !self.quiet_hours,
            Change::QuietFrom(h) => self.quiet_from = h,
            Change::QuietTo(h) => self.quiet_to = h,
            Change::CacheMb(mb) => self.cache_mb = mb,
            Change::Proxy(v) => self.proxy = v,
            Change::Telemetry => self.telemetry = !self.telemetry,
        }
    }
}

impl Prefs {
    pub fn new(path: Option<PathBuf>) -> Prefs {
        let settings = path.as_deref().map(Settings::load).unwrap_or_default();
        Prefs {
            settings,
            path,
            section: Section::General,
            search: String::new(),
            confirm_reset: false,
            save_error: None,
        }
    }

    fn save(&mut self) {
        if let Some(path) = &self.path {
            self.save_error = self.settings.save(path).err().map(|e| format!("Couldn't save settings: {e}"));
        }
    }

    fn visible_sections(&self) -> Vec<Section> {
        let q = self.search.trim().to_lowercase();
        Section::ALL
            .into_iter()
            .filter(|s| q.is_empty() || s.title().to_lowercase().contains(&q) || s.keywords().contains(&q))
            .collect()
    }
}

impl App for Prefs {
    type Msg = Msg;

    fn theme(&self) -> Theme {
        let s = &self.settings;
        let dark = match s.mode {
            1 => false,
            2 => true,
            _ => system_prefs().dark.unwrap_or(true),
        };
        let base = if dark { ThemeConfig::dark() } else { ThemeConfig::light() };
        let high = s.high_contrast || system_prefs().high_contrast;
        Theme::from_config(ThemeConfig {
            accent: Accent::ALL[s.accent].color(dark),
            radius: s.radius,
            density: [Density::Compact, Density::Default, Density::Comfortable][s.density],
            contrast: if high { Contrast::High } else { Contrast::Normal },
            ..base
        })
    }

    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        match msg {
            Msg::Section(s) => self.section = s,
            Msg::Search(q) => {
                self.search = q;
                if let Some(first) = self.visible_sections().first() {
                    if !self.visible_sections().contains(&self.section) {
                        self.section = *first;
                    }
                }
            }
            Msg::Set(c) => {
                self.settings.apply(c);
                self.save();
            }
            Msg::AskReset(open) => self.confirm_reset = open,
            Msg::Reset => {
                self.settings = Settings::default();
                self.confirm_reset = false;
                self.save();
            }
        }
    }

    fn view(&self) -> Element<Msg> {
        let th = theme();
        let c = &th.colors;
        let sections = self.visible_sections();
        let sidebar = col()
            .w(240.0)
            .h_full()
            .shrink(0.0)
            .bg(c.panel)
            .border_r(1.0, c.border)
            .p(12.0)
            .gap(12.0)
            .child(search_input(self.search.clone(), Msg::Search).id("search"))
            .child(
                col().gap(2.0).children(
                    sections
                        .iter()
                        .map(|&s| list_item(Some(s.icon()), s.title(), s == self.section).on_click(Msg::Section(s))),
                ),
            )
            .child_if(sections.is_empty(), || text("No matching settings").color(c.text_muted).px(8.0));

        let page = col().grow(1.0).h_full().scroll_y().child(
            col()
                .max_w(720.0)
                .w_full()
                .px(32.0)
                .py(28.0)
                .gap(24.0)
                .child(text(self.section.title()).font_size(22.0).bold())
                .children(self.save_error.clone().map(|e| text(e).color(c.danger)))
                .children(self.section_view()),
        );

        let mut root = row().size_full().bg(c.background).child(sidebar).child(page);
        if self.confirm_reset {
            root = root.child(modal(
                "Reset all settings?",
                text("Every setting goes back to its default. This can't be undone.").color(c.text_muted),
                vec![
                    button("Cancel").on_click(Msg::AskReset(false)),
                    danger_button("Reset").id("confirm-reset").on_click(Msg::Reset),
                ],
                Msg::AskReset(false),
            ));
        }
        root
    }
}

/// A group of rows under a heading.
fn group(title: &str, rows: Vec<Element<Msg>>) -> Element<Msg> {
    let th = theme();
    let mut body = card().p(0.0).gap(0.0);
    let n = rows.len();
    for (i, r) in rows.into_iter().enumerate() {
        body = body.child(r.when(i + 1 < n, |e| e.border_b(1.0, th.colors.border)));
    }
    col().gap(8.0).child(text(title).semibold().color(th.colors.text_muted).font_size(th.font_size_sm)).child(body)
}

/// A labeled setting: title and description on the left, the control on the right.
fn setting(title: &str, description: &str, control: impl Into<Element<Msg>>) -> Element<Msg> {
    let th = theme();
    row()
        .px(16.0)
        .py(12.0)
        .gap(16.0)
        .items_center()
        .child(col().grow(1.0).gap(2.0).child(text(title)).child_if(!description.is_empty(), || {
            text(description).color(th.colors.text_muted).font_size(th.font_size_sm)
        }))
        .child(control.into().shrink(0.0))
}

fn toggle(on: bool, change: Change) -> Element<Msg> {
    switch(on).on_click(Msg::Set(change))
}

fn choices(labels: &[&str], selected: usize, change: fn(usize) -> Change) -> Element<Msg> {
    segmented(labels.iter().enumerate().map(|(i, l)| (l.to_string(), i == selected, Msg::Set(change(i)))).collect())
}

impl Prefs {
    fn section_view(&self) -> Vec<Element<Msg>> {
        let s = &self.settings;
        let set = |f: fn(String) -> Change| move |v: String| Msg::Set(f(v));
        match self.section {
            Section::General => vec![
                group(
                    "Profile",
                    vec![
                        setting(
                            "Display name",
                            "Shown to other people.",
                            text_input(s.display_name.clone(), set(Change::DisplayName)).id("display-name").w(220.0),
                        ),
                        setting(
                            "Language",
                            "",
                            pick_list(LANGUAGES, Some(s.language), |i| Msg::Set(Change::Language(i))).w(220.0),
                        ),
                    ],
                ),
                group(
                    "Startup",
                    vec![
                        setting(
                            "Open at login",
                            "Start the app when you sign in.",
                            toggle(s.open_at_login, Change::OpenAtLogin),
                        ),
                        setting(
                            "Check for updates",
                            "",
                            toggle(s.check_updates, Change::CheckUpdates).id("check-updates"),
                        ),
                    ],
                ),
            ],
            Section::Appearance => vec![
                group(
                    "Theme",
                    vec![
                        setting(
                            "Mode",
                            "System follows your OS setting.",
                            choices(&["System", "Light", "Dark"], s.mode, Change::Mode),
                        ),
                        setting(
                            "Accent color",
                            "",
                            row().gap(6.0).children(Accent::ALL.iter().enumerate().map(|(i, a)| {
                                color_swatch(a.color(true), i == s.accent)
                                    .tooltip(a.name())
                                    .on_click(Msg::Set(Change::Accent(i)))
                            })),
                        ),
                        setting(
                            "High contrast",
                            "Stronger borders and text. Also follows the OS setting.",
                            toggle(s.high_contrast, Change::HighContrast),
                        ),
                    ],
                ),
                group(
                    "Layout",
                    vec![
                        setting(
                            "Corner radius",
                            "",
                            row()
                                .gap(12.0)
                                .items_center()
                                .child(slider(s.radius, 0.0, 16.0).w(180.0).on_change(|r| Msg::Set(Change::Radius(r))))
                                .child(text(format!("{:.0} px", s.radius)).w(40.0)),
                        ),
                        setting(
                            "Density",
                            "Height of controls and rows.",
                            choices(&["Compact", "Default", "Comfortable"], s.density, Change::Density),
                        ),
                    ],
                ),
            ],
            Section::Notifications => vec![group(
                "Notify me about",
                vec![
                    setting("Messages", "", toggle(s.notify_messages, Change::NotifyMessages)),
                    setting(
                        "Mentions",
                        "When someone mentions you by name.",
                        toggle(s.notify_mentions, Change::NotifyMentions),
                    ),
                    setting("Play sounds", "", toggle(s.notify_sounds, Change::NotifySounds)),
                    setting("Quiet hours", "Hold notifications overnight.", toggle(s.quiet_hours, Change::QuietHours)),
                    setting(
                        "Quiet from / to",
                        "Hours, 0 to 23.",
                        row()
                            .gap(8.0)
                            .items_center()
                            .child(
                                Element::from(
                                    number_input("quiet-from", s.quiet_from, |h| Msg::Set(Change::QuietFrom(h)))
                                        .range(0.0, 23.0),
                                )
                                .w(110.0),
                            )
                            .child(text("to"))
                            .child(
                                Element::from(
                                    number_input("quiet-to", s.quiet_to, |h| Msg::Set(Change::QuietTo(h)))
                                        .range(0.0, 23.0),
                                )
                                .w(110.0),
                            )
                            .when(!s.quiet_hours, |e| e.opacity(0.5).pointer_events(false)),
                    ),
                ],
            )],
            Section::Advanced => vec![
                group(
                    "Storage and network",
                    vec![
                        setting(
                            "Cache size",
                            "Megabytes kept for offline use.",
                            Element::from(
                                number_input("cache", s.cache_mb, |mb| Msg::Set(Change::CacheMb(mb)))
                                    .range(64.0, 8192.0)
                                    .step(64.0),
                            )
                            .w(140.0),
                        ),
                        setting(
                            "Proxy",
                            "Leave empty to connect directly.",
                            text_input(s.proxy.clone(), set(Change::Proxy)).w(220.0),
                        ),
                        setting("Send usage data", "Anonymous statistics.", toggle(s.telemetry, Change::Telemetry)),
                    ],
                ),
                group(
                    "Reset",
                    vec![setting(
                        "Reset all settings",
                        "Restore every default.",
                        danger_button("Reset…").id("reset").on_click(Msg::AskReset(true)),
                    )],
                ),
            ],
            Section::About => vec![group(
                "About",
                vec![
                    setting("Version", "", text(env!("CARGO_PKG_VERSION")).color(theme().colors.text_muted)),
                    setting(
                        "Settings file",
                        "",
                        text(self.path.as_ref().map_or("Not saved".into(), |p| p.display().to_string()))
                            .color(theme().colors.text_muted)
                            .selectable(),
                    ),
                ],
            )],
        }
    }
}

fn main() {
    let app = Prefs::new(config_path(env!("CARGO_PKG_NAME")));
    let opts = WindowOptions::new("Settings").size(960.0, 680.0).min_size(640.0, 420.0);
    if let Err(e) = rust_ui::run(app, opts) {
        eprintln!("error: {e}");
    }
}
