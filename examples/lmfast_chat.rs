//! A recreation of lmfast-rs's Chat screen (originally built with iced) to
//! compare look and feature parity. Streaming is mocked.
//!
//! Run:        cargo run --release --example lmfast_chat
//! Screenshot: cargo run --release --example lmfast_chat -- --screenshot chat.png
//!             [--streaming] [--settings] [--light] [--accent violet] [--gray slate]
//!
//! The whole look is generated from a handful of `ThemeConfig` knobs, which the
//! Settings screen changes live: mode, accent, neutral tint, corners, density
//! and text size. Nothing in the views hard-codes a color.

use std::rc::Rc;
use std::time::{Duration, Instant};

use futures::StreamExt;
use rust_ui::prelude::*;
use rust_ui::TaskHandle;

const LOG_CAP: usize = 20_000;

fn clock(secs: u32) -> String {
    format!("{:02}:{:02}:{:02}", secs / 3600 % 24, secs / 60 % 60, secs % 60)
}

const GRAYS: [(&str, GrayTint); 7] = [
    ("Zinc", GrayTint::Zinc),
    ("Slate", GrayTint::Slate),
    ("Gray", GrayTint::Gray),
    ("Mauve", GrayTint::Mauve),
    ("Sand", GrayTint::Sand),
    ("Sage", GrayTint::Sage),
    ("Tinted", GrayTint::Accent),
];

/// One engine log line (`HH:MM:SS  [LEVEL]  message`).
#[derive(Clone)]
struct LogLine {
    secs: u32,
    level: Level,
    msg: String,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

impl Level {
    fn label(self) -> &'static str {
        match self {
            Level::Debug => "DEBUG",
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
        }
    }
}

/// A model on disk (what lmfast's Models screen lists).
struct ModelInfo {
    name: String,
    arch: &'static str,
    params: f32,
    publisher: &'static str,
    quant: &'static str,
    size_gb: f32,
    caps: &'static [&'static str],
}

fn mock_models() -> Vec<ModelInfo> {
    let base: [(&str, &str, f32, &str, &[&str]); 12] = [
        ("Qwen2.5-7B-Instruct", "qwen2", 7.6, "Qwen", &["Tools"]),
        ("Qwen2.5-VL-7B-Instruct", "qwen2vl", 8.3, "Qwen", &["Vision", "Tools"]),
        ("Llama-3.1-8B-Instruct", "llama", 8.0, "meta-llama", &["Tools"]),
        ("Llama-3.2-3B-Instruct", "llama", 3.2, "meta-llama", &[]),
        ("gemma-3-12b-it", "gemma3", 12.2, "google", &["Vision"]),
        ("gemma-3-27b-it", "gemma3", 27.4, "google", &["Vision"]),
        ("DeepSeek-R1-Distill-Qwen-14B", "qwen2", 14.8, "deepseek-ai", &["Reasoning"]),
        ("Mistral-Small-3.1-24B-Instruct", "mistral3", 24.0, "mistralai", &["Vision", "Tools"]),
        ("Phi-4-mini-instruct", "phi3", 3.8, "microsoft", &["Tools"]),
        ("gpt-oss-20b", "gpt-oss", 20.9, "openai", &["Reasoning", "Tools"]),
        ("Qwen3-30B-A3B", "qwen3moe", 30.5, "Qwen", &["Reasoning", "Tools"]),
        ("whisper-large-v3-turbo", "whisper", 0.8, "openai", &["Audio"]),
    ];
    let quants = [("Q4_K_M", 0.60), ("Q5_K_M", 0.71), ("Q8_0", 1.06)];
    let mut out = Vec::new();
    for (qi, (q, bytes_per_param)) in quants.iter().enumerate() {
        for (i, (name, arch, params, publisher, caps)) in base.iter().enumerate() {
            if (i + qi) % 3 == 2 {
                continue;
            }
            out.push(ModelInfo {
                name: format!("{name}-{q}"),
                arch,
                params: *params,
                publisher,
                quant: q,
                size_gb: params * bytes_per_param,
                caps,
            });
        }
    }
    out
}

/// Deterministic fake llama-server output.
fn log_line(n: usize) -> LogLine {
    const MSGS: [&str; 8] = [
        "slot update_slots: id 0 | task 412 | prompt processing progress, n_past = 2048, n_tokens = 512",
        "srv  params_from_: Chat format: Hermes 2 Pro",
        "slot launch_slot_: id 0 | task 413 | processing task",
        "srv  log_server_r: request: POST /v1/chat/completions 127.0.0.1 200",
        "llama_kv_cache_unified: CUDA0 KV buffer size = 448.00 MiB",
        "slot print_timing: id 0 | task 412 | prompt eval time = 38.21 ms / 512 tokens (13399.6 tokens per second)",
        "srv  update_slots: all slots are idle",
        "ggml_cuda_init: found 1 CUDA devices: NVIDIA GeForce RTX 4090, compute capability 8.9",
    ];
    let level = match n % 23 {
        0 => Level::Error,
        5 | 13 => Level::Warn,
        2 | 7 | 9 | 16 | 20 => Level::Debug,
        _ => Level::Info,
    };
    let msg = match level {
        Level::Error => "srv  send_error: task id = 414, error: context size exceeded (n_ctx = 8192)".to_string(),
        Level::Warn => "common_init_from_params: warming up the model with an empty run - please wait".to_string(),
        _ => MSGS[n % MSGS.len()].to_string(),
    };
    LogLine { secs: 9 * 3600 + 41 * 60 + (n / 4) as u32, level, msg }
}

/// Whole-app looks built only from theme style classes.
#[derive(Clone, Copy, PartialEq, Debug)]
enum StylePreset {
    Default,
    Pill,
    Sharp,
    Flat,
}

impl StylePreset {
    const ALL: [(&'static str, StylePreset); 4] = [
        ("Default", StylePreset::Default),
        ("Pill", StylePreset::Pill),
        ("Sharp", StylePreset::Sharp),
        ("Flat", StylePreset::Flat),
    ];

    fn apply(self, t: Theme) -> Theme {
        match self {
            StylePreset::Default => t,
            StylePreset::Pill => t
                .style_class("button", |e| e.pill().px(16.0))
                .style_class("icon-button", |e| e.pill())
                .style_class("input", |e| e.pill().px(14.0))
                .style_class("text-area", |e| e.rounded(18.0))
                .style_class("segmented", |e| e.pill())
                .style_class("segmented-item", |e| e.pill())
                .style_class("card", |e| e.rounded(18.0))
                .style_class("tree-row", |e| e.pill()),
            StylePreset::Sharp => t
                .style_class("button", |e| e.rounded(0.0).no_shadow().border(1.0, theme().colors.text_faint))
                .style_class("button-primary", |e| e.border(1.0, theme().colors.accent))
                .style_class("icon-button", |e| e.rounded(0.0))
                .style_class("input", |e| e.rounded(0.0))
                .style_class("segmented", |e| e.rounded(0.0))
                .style_class("segmented-item", |e| e.rounded(0.0))
                .style_class("card", |e| e.rounded(0.0).no_shadow().border(1.0, theme().colors.border_strong))
                .style_class("tree-row", |e| e.rounded(0.0))
                .style_class("badge", |e| e.rounded(0.0))
                .style_class("table-row", |e| e.rounded(0.0)),
            StylePreset::Flat => t
                .style_class("button", |e| e.no_shadow().border(0.0, Color::TRANSPARENT))
                .style_class("button-secondary", |e| e.bg(theme().colors.hover))
                .style_class("input", |e| e.border(0.0, Color::TRANSPARENT).bg(theme().colors.hover))
                .style_class("card", |e| e.no_shadow().border(0.0, Color::TRANSPARENT).bg(theme().colors.panel))
                .style_class("segmented", |e| e.border(0.0, Color::TRANSPARENT)),
        }
    }
}

/// The user's appearance choices (what lmfast would persist in settings).
#[derive(Clone, Debug, PartialEq)]
struct Appearance {
    dark: bool,
    accent: Accent,
    gray: GrayTint,
    radius: f32,
    density: Density,
    scaling: f32,
    preset: StylePreset,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            dark: true,
            accent: Accent::Indigo,
            gray: GrayTint::Zinc,
            radius: 6.0,
            density: Density::Default,
            scaling: 1.0,
            preset: StylePreset::Default,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Screen {
    Chat,
    Discover,
    Models,
    Tune,
    Developer,
    Settings,
}

#[derive(Clone)]
struct Message {
    user: bool,
    text: String,
    reasoning: Option<(String, f32)>,
    show_reasoning: bool,
}

/// A sidebar folder: its open/closed state is UI-only, so it lives in a
/// component instead of the app.
struct Folder {
    name: &'static str,
    /// (conversation index, title, active) of the matching conversations.
    items: Vec<(usize, &'static str, bool)>,
    open_initially: bool,
}

#[derive(Clone)]
enum FolderEv {
    Toggle,
    Open(usize),
}

impl Component for Folder {
    type State = bool;
    type Event = FolderEv;
    type Output = Msg;

    fn init(&self) -> bool {
        self.open_initially
    }

    fn update(&self, open: &mut bool, e: FolderEv) -> Option<Msg> {
        match e {
            FolderEv::Toggle => {
                *open = !*open;
                None
            }
            FolderEv::Open(i) => Some(Msg::Select(i)),
        }
    }

    fn view(&self, open: &bool) -> Element<FolderEv> {
        col()
            .gap(1.0)
            .child(tree_row(0, Some(*open), Some(Icon::Folder), self.name, false).on_click(FolderEv::Toggle))
            .children(
                self.items
                    .iter()
                    .filter(|_| *open)
                    .map(|&(i, title, active)| tree_row(1, None, None, title, active).on_click(FolderEv::Open(i))),
            )
    }
}

struct Convo {
    title: &'static str,
    folder: Option<&'static str>,
}

struct LmFast {
    screen: Screen,
    models: Vec<&'static str>,
    model: Option<usize>,
    convos: Vec<Convo>,
    active: usize,
    search: String,
    messages: Vec<Message>,
    draft: String,
    streaming: Option<TaskHandle>,
    started: Option<Instant>,
    tokens: usize,
    tok_per_s: f32,
    confirm_quit: bool,
    show_params: bool,
    temperature: f32,
    look: Appearance,
    log: Rc<Vec<LogLine>>,
    library: Rc<Vec<ModelInfo>>,
    lib_search: String,
    lib_sort: (usize, SortDir),
    lib_selected: Option<usize>,
    log_filter: Option<Level>,
    log_live: bool,
    log_at_end: bool,
}

#[derive(Clone, Debug)]
enum Msg {
    Switch(Screen),
    Model(usize),
    Select(usize),
    Search(String),
    Draft(String),
    Send,
    Stop,
    Token(String),
    Done,
    Tick,
    ToggleReasoning(usize),
    ToggleParams,
    Temperature(f32),
    Dark(bool),
    Accent(Accent),
    Gray(GrayTint),
    Radius(f32),
    Density(Density),
    Scaling(f32),
    ResetLook,
    Preset(StylePreset),
    LogTick,
    LibSearch(String),
    LibSort(usize, SortDir),
    LibSelect(usize),
    LogLive(bool),
    LogFilter(Option<Level>),
    LogScrolled(ScrollInfo),
    LogJumpLatest,
    LogClear,
    LogCopy,
    CloseRequested,
    CancelQuit,
    Quit,
    Noop,
}

impl App for LmFast {
    type Msg = Msg;

    fn theme(&self) -> Theme {
        // Every color, radius and size below is generated from these knobs.
        let l = &self.look;
        let t = Theme::from_config(ThemeConfig {
            dark: l.dark,
            accent: l.accent.color(l.dark),
            gray: l.gray,
            radius: l.radius,
            density: l.density,
            scaling: l.scaling,
            ..ThemeConfig::dark()
        });
        l.preset.apply(t)
    }

    fn subscriptions(&self) -> Subscriptions<Msg> {
        Subscriptions::none()
            .every_if(self.streaming.is_some(), Duration::from_millis(250), Msg::Tick)
            .every_if(self.log_live && self.screen == Screen::Developer, Duration::from_millis(80), Msg::LogTick)
            .on_close_request(Msg::CloseRequested)
    }

    fn update(&mut self, msg: Msg, cx: &mut Cx<Msg>) {
        match msg {
            Msg::Switch(s) => self.screen = s,
            Msg::Model(i) => self.model = Some(i),
            Msg::Select(i) => self.active = i,
            Msg::Search(s) => self.search = s,
            Msg::Draft(s) => self.draft = s,
            Msg::Send => {
                let prompt = self.draft.trim().to_string();
                if prompt.is_empty() || self.streaming.is_some() {
                    return;
                }
                self.draft.clear();
                self.messages.push(Message { user: true, text: prompt, reasoning: None, show_reasoning: false });
                self.messages.push(Message {
                    user: false,
                    text: String::new(),
                    reasoning: Some(("Planning the answer: explain, then show code…".into(), 1.8)),
                    show_reasoning: false,
                });
                self.tokens = 0;
                self.started = Some(Instant::now());
                // Mock token stream (in lmfast this is llama-server's SSE stream).
                let chunks: Vec<String> =
                    MOCK_REPLY.split_inclusive([' ', '\n']).map(str::to_string).collect::<Vec<_>>();
                let stream = futures::stream::iter(chunks).then(|t| async move {
                    std::thread::sleep(Duration::from_millis(18));
                    t
                });
                self.streaming =
                    Some(cx.run(stream.chain(futures::stream::once(async { String::from("\u{0}") })), |t| {
                        if t == "\u{0}" {
                            Msg::Done
                        } else {
                            Msg::Token(t)
                        }
                    }));
                cx.scroll_to_end("transcript");
            }
            Msg::Stop => {
                if let Some(h) = self.streaming.take() {
                    h.abort();
                }
            }
            Msg::Token(t) => {
                if let Some(m) = self.messages.last_mut() {
                    m.text.push_str(&t);
                    self.tokens += 1;
                }
            }
            Msg::Done => self.streaming = None,
            Msg::Tick => {
                if let Some(s) = self.started {
                    self.tok_per_s = self.tokens as f32 / s.elapsed().as_secs_f32().max(0.001);
                }
            }
            Msg::ToggleReasoning(i) => {
                if let Some(m) = self.messages.get_mut(i) {
                    m.show_reasoning = !m.show_reasoning;
                }
            }
            Msg::ToggleParams => self.show_params = !self.show_params,
            Msg::Temperature(v) => self.temperature = v,
            Msg::Dark(d) => self.look.dark = d,
            Msg::Accent(a) => self.look.accent = a,
            Msg::Gray(g) => self.look.gray = g,
            Msg::Radius(r) => self.look.radius = (r * 2.0).round() / 2.0,
            Msg::Density(d) => self.look.density = d,
            Msg::Scaling(k) => self.look.scaling = k,
            Msg::ResetLook => self.look = Appearance::default(),
            Msg::Preset(p) => self.look.preset = p,
            Msg::LibSearch(q) => self.lib_search = q,
            Msg::LibSort(c, d) => self.lib_sort = (c, d),
            Msg::LibSelect(i) => self.lib_selected = Some(i),
            Msg::LogTick => {
                // The whole buffer is kept (capped at 20k lines); only visible rows are built.
                let log = Rc::make_mut(&mut self.log);
                let n = log.len();
                log.extend((n..n + 3).map(log_line));
                if log.len() > LOG_CAP {
                    log.drain(..log.len() - LOG_CAP);
                }
            }
            Msg::LogLive(on) => self.log_live = on,
            Msg::LogFilter(f) => self.log_filter = f,
            Msg::LogScrolled(info) => self.log_at_end = info.at_end,
            Msg::LogJumpLatest => cx.scroll_to_end("engine-log"),
            Msg::LogClear => self.log = Rc::new(Vec::new()),
            Msg::LogCopy => {
                let all: Vec<String> =
                    self.log.iter().map(|l| format!("{} [{}] {}", clock(l.secs), l.level.label(), l.msg)).collect();
                cx.copy_to_clipboard(all.join("\n"));
            }
            Msg::CloseRequested => {
                if self.streaming.is_some() {
                    self.confirm_quit = true;
                } else {
                    cx.close_window();
                }
            }
            Msg::CancelQuit => self.confirm_quit = false,
            Msg::Quit => cx.close_window(),
            Msg::Noop => {}
        }
    }

    fn view(&self) -> Element<Msg> {
        let th = theme();
        let c = &th.colors;
        let mut root = row().size_full().bg(c.background).child(self.nav()).child(match self.screen {
            Screen::Chat => self.chat(),
            Screen::Developer => self.developer(),
            Screen::Models => self.models_screen(),
            Screen::Settings => self.settings(),
            _ => col().grow(1.0).center().color(c.text_faint).child(text("Not part of this demo")),
        });
        if self.confirm_quit {
            root = root.child(modal(
                "Quit while generating?",
                text("A reply is still streaming. Quitting stops it and ejects the model."),
                vec![button("Keep running").on_click(Msg::CancelQuit), danger_button("Quit").on_click(Msg::Quit)],
                Msg::CancelQuit,
            ));
        }
        root
    }
}

impl LmFast {
    fn nav(&self) -> Element<Msg> {
        let th = theme();
        let c = &th.colors;
        let nav_btn = |label: &'static str, s: Screen, i: Icon| {
            let active = self.screen == s;
            row()
                .items_center()
                .gap(10.0)
                .h(32.0)
                .px(10.0)
                .rounded(th.radius_sm)
                .color(if active { c.text } else { c.text_muted })
                .transition(0.12)
                .when(active, |r| {
                    r.bg(c.accent_soft).child(div().absolute().left(0.0).top(8.0).bottom(8.0).w(2.0).bg(c.accent))
                })
                .hover(|s| s.bg(c.hover).color(c.text))
                .on_click(Msg::Switch(s))
                .focusable()
                .aria_selected(active)
                .child(icon(i).font_size(15.0).color(if active { c.accent } else { c.text_faint }))
                .child(text(label).medium())
        };
        let status = match self.model {
            Some(m) => row()
                .gap(8.0)
                .items_center()
                .child(div().square(7.0).pill().bg(if self.streaming.is_some() { c.accent } else { c.success }))
                .child(text(self.models[m]).mono().font_size(11.0).ellipsis()),
            None => row().child(text("no model loaded").font_size(11.0).color(c.text_faint)),
        };
        col()
            .w(220.0)
            .shrink(0.0)
            .p(16.0)
            .gap(16.0)
            .bg(c.panel)
            .border_r(1.0, c.border)
            .child(
                col()
                    .gap(2.0)
                    .px(4.0)
                    .child(rich_text([span("LM").size(20.0).bold(), span("Fast").size(20.0).bold().color(c.accent)]))
                    .child(text("local LLMs · native speed").mono().font_size(11.0).color(c.text_faint)),
            )
            .child(
                col()
                    .gap(2.0)
                    .child(nav_btn("Chat", Screen::Chat, Icon::Code))
                    .child(nav_btn("Discover", Screen::Discover, Icon::Search))
                    .child(nav_btn("Models", Screen::Models, Icon::Layers))
                    .child(nav_btn("Tune", Screen::Tune, Icon::Settings))
                    .child(nav_btn("Developer", Screen::Developer, Icon::Terminal))
                    .child(nav_btn("Settings", Screen::Settings, Icon::Grid)),
            )
            .child(spacer())
            .child(status)
    }

    fn models_screen(&self) -> Element<Msg> {
        let th = theme();
        let c = th.colors.clone();
        let lib = self.library.clone();
        // Filter + sort indices (the data itself is never copied).
        let q = self.lib_search.to_lowercase();
        let mut shown: Vec<usize> = (0..lib.len())
            .filter(|&i| {
                q.is_empty() || lib[i].name.to_lowercase().contains(&q) || lib[i].publisher.to_lowercase().contains(&q)
            })
            .collect();
        let (sc, dir) = self.lib_sort;
        shown.sort_by(|&a, &b| {
            let (a, b) = (&lib[a], &lib[b]);
            let o = match sc {
                0 => a.arch.cmp(b.arch),
                1 => a.params.total_cmp(&b.params),
                2 => a.publisher.to_lowercase().cmp(&b.publisher.to_lowercase()),
                3 => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                _ => a.size_gb.total_cmp(&b.size_gb),
            };
            if dir == SortDir::Asc {
                o
            } else {
                o.reverse()
            }
        });
        let shown = Rc::new(shown);
        let selected_row = self.lib_selected.and_then(|m| shown.iter().position(|&i| i == m));
        let loaded = self.model.map(|m| self.models[m].to_lowercase());
        let (rows, data, cc) = (shown.clone(), lib.clone(), c.clone());
        let chip = move |label: String, cc: &Palette| {
            div()
                .px(7.0)
                .py(1.0)
                .rounded(4.0)
                .bg(cc.accent_soft)
                .color(cc.code_text)
                .font_size(11.0)
                .child(text(label).nowrap())
        };
        let t = table(
            "library",
            vec![
                Column::new("Arch").weight(11.0).sortable(),
                Column::new("Params").weight(8.0).sortable().align_end(),
                Column::new("Publisher").weight(16.0).sortable(),
                Column::new("Model").weight(30.0).sortable().min_width(160.0),
                Column::new("Capabilities").weight(21.0),
                Column::new("Size").weight(9.0).sortable().align_end(),
            ],
            shown.len(),
            move |r, col| {
                let m = &data[rows[r]];
                match col {
                    0 => chip(m.arch.to_string(), &cc),
                    1 => cell_text(format!("{:.1}B", m.params)).color(cc.text_muted),
                    2 => cell_text(m.publisher).color(cc.text_muted),
                    3 => {
                        let is_loaded = loaded.as_deref() == Some(m.name.to_lowercase().as_str());
                        row()
                            .gap(6.0)
                            .items_center()
                            .min_w(0.0)
                            .child(cell_text(m.name.clone()))
                            .child_if(is_loaded, || div().square(7.0).pill().bg(cc.success).shrink(0.0))
                    }
                    4 => row().gap(4.0).children(m.caps.iter().map(|cap| chip(cap.to_string(), &cc))),
                    _ => cell_text(format!("{:.1} GB", m.size_gb)).mono().color(cc.text_muted),
                }
            },
        )
        .sort(sc, dir, Msg::LibSort)
        .on_row_click({
            let rows = shown.clone();
            move |r| Msg::LibSelect(rows[r])
        })
        .selected(selected_row)
        .empty_state(text("No models match the filter."));

        let detail = match self.lib_selected {
            Some(i) => {
                let m = &lib[i];
                let kv = |k: &str, v: String| {
                    row()
                        .gap(12.0)
                        .py(6.0)
                        .border_b(1.0, c.border)
                        .child(text(k.to_string()).w(96.0).shrink(0.0).color(c.text_faint))
                        .child(text(v).selectable().grow(1.0).min_w(0.0))
                };
                col()
                    .gap(12.0)
                    .p(18.0)
                    .child(text(m.name.clone()).font_size(th.font_size_lg).bold())
                    .child(
                        row()
                            .gap(8.0)
                            .child(primary_button("Load model").with_icon(Icon::Play))
                            .child(button("Show in folder").with_icon(Icon::Folder)),
                    )
                    .child(
                        col()
                            .child(kv("Architecture", m.arch.into()))
                            .child(kv("Parameters", format!("{:.1}B", m.params)))
                            .child(kv("Quantization", m.quant.into()))
                            .child(kv("File size", format!("{:.2} GB", m.size_gb)))
                            .child(kv("Publisher", m.publisher.into()))
                            .child(kv(
                                "Capabilities",
                                if m.caps.is_empty() { "Text".into() } else { m.caps.join(", ") },
                            )),
                    )
            }
            None => col().grow(1.0).center().color(c.text_faint).child(text("Select a model to see details")),
        };

        hsplit(
            "models-split",
            vec![
                Pane::fill(
                    col()
                        .min_w(0.0)
                        .p(20.0)
                        .gap(14.0)
                        .child(
                            row()
                                .items_center()
                                .gap(12.0)
                                .child(text("Models").font_size(th.font_size_lg * 1.4).bold())
                                .child(badge(format!("{} of {}", shown.len(), lib.len())))
                                .child(spacer())
                                .child(search_input(self.lib_search.clone(), Msg::LibSearch).w(260.0)),
                        )
                        .child(
                            text("CPU 16 cores · RAM 64 GB · VRAM 24 GB (RTX 4090)")
                                .font_size(th.font_size_sm)
                                .color(c.text_faint),
                        )
                        .child(card().p(0.0).gap(0.0).grow(1.0).min_h(0.0).child(Element::from(t).grow(1.0))),
                ),
                Pane::fixed(340.0, detail.bg(c.panel).h_full()).min(260.0).max(520.0).collapsible(true),
            ],
        )
    }

    fn developer(&self) -> Element<Msg> {
        let th = theme();
        let c = th.colors.clone();
        // Indices of the lines passing the level filter (cheap even for 20k lines).
        let shown: Rc<Vec<u32>> = Rc::new(
            (0..self.log.len() as u32)
                .filter(|&i| self.log_filter.is_none_or(|f| self.log[i as usize].level == f))
                .collect(),
        );
        let log = self.log.clone();
        let rows = shown.clone();
        let cc = c.clone();
        let list = virtual_list(shown.len(), move |i| {
            let l = &log[rows[i] as usize];
            let level_color = match l.level {
                Level::Debug => cc.text_faint,
                Level::Info => cc.accent,
                Level::Warn => cc.warning,
                Level::Error => cc.danger,
            };
            row()
                .gap(12.0)
                .px(12.0)
                .min_h(20.0)
                .items(Align::Start)
                .rounded(3.0)
                .hover(|s| s.bg(cc.hover))
                .mono()
                .font_size(12.0)
                .child(text(clock(l.secs)).color(cc.text_faint).shrink(0.0))
                .child(text(l.level.label()).color(level_color).w(44.0).shrink(0.0))
                .child(text(l.msg.clone()).color(cc.text_muted).grow(1.0).min_w(0.0).selectable())
        })
        .id("engine-log")
        .item_height(20.0)
        .py(6.0)
        .follow_end()
        .on_scroll(Msg::LogScrolled)
        .grow(1.0);
        let filter = segmented(
            [
                ("All", None),
                ("Debug", Some(Level::Debug)),
                ("Info", Some(Level::Info)),
                ("Warn", Some(Level::Warn)),
                ("Error", Some(Level::Error)),
            ]
            .into_iter()
            .map(|(n, f)| (n.to_string(), self.log_filter == f, Msg::LogFilter(f)))
            .collect(),
        );
        let header = row()
            .items_center()
            .gap(10.0)
            .px(14.0)
            .h(48.0)
            .border_b(1.0, c.border)
            .child(text("Engine logs").medium())
            .child(badge(format!("{} lines", shown.len())))
            .child(spacer())
            .child(filter)
            .child(
                row()
                    .items_center()
                    .gap(6.0)
                    .child(switch(self.log_live).on_click(Msg::LogLive(!self.log_live)))
                    .child(text("Live").font_size(th.font_size_sm).color(c.text_muted)),
            )
            .child(ghost_button("Copy").with_icon(Icon::Files).on_click(Msg::LogCopy))
            .child(ghost_button("Clear").on_click(Msg::LogClear));
        let jump = (!self.log_at_end).then(|| {
            primary_button("Jump to latest")
                .with_icon(Icon::ChevronDown)
                .absolute()
                .bottom(16.0)
                .right(24.0)
                .shadows(th.shadow_popover.clone())
                .on_click(Msg::LogJumpLatest)
        });
        col()
            .grow(1.0)
            .min_w(0.0)
            .p(24.0)
            .gap(16.0)
            .child(
                col()
                    .gap(4.0)
                    .child(text("Developer · OpenAI-compatible server").font_size(th.font_size_lg * 1.4).bold())
                    .child(
                        row()
                            .gap(8.0)
                            .items_center()
                            .child(div().square(7.0).pill().bg(c.success))
                            .child(text("Running").color(c.text_muted))
                            .child(text("http://127.0.0.1:1234/v1").mono().color(c.text_muted)),
                    ),
            )
            .child(card().p(0.0).gap(0.0).grow(1.0).min_h(0.0).child(header).child(list).children(jump))
    }

    fn settings(&self) -> Element<Msg> {
        let th = theme();
        let c = &th.colors;
        let l = &self.look;
        let setting = |label: &'static str, hint: &'static str, control: Element<Msg>| {
            row()
                .items_center()
                .gap(24.0)
                .py(14.0)
                .border_b(1.0, c.border)
                .child(
                    col()
                        .w(200.0)
                        .shrink(0.0)
                        .gap(2.0)
                        .child(text(label).medium())
                        .child(text(hint).font_size(th.font_size_sm).color(c.text_faint)),
                )
                .child(div().grow(1.0).min_w(0.0).child(control))
        };
        let mut swatches = row().flex_wrap().gap(6.0);
        for a in Accent::ALL {
            swatches =
                swatches.child(color_swatch(a.color(l.dark), a == l.accent).tooltip(a.name()).on_click(Msg::Accent(a)));
        }
        let grays = segmented(GRAYS.iter().map(|(n, g)| (n.to_string(), *g == l.gray, Msg::Gray(*g))).collect());
        let densities = segmented(
            [("Compact", Density::Compact), ("Default", Density::Default), ("Comfortable", Density::Comfortable)]
                .into_iter()
                .map(|(n, d)| (n.to_string(), d == l.density, Msg::Density(d)))
                .collect(),
        );
        let sizes = segmented(
            [("90%", 0.9), ("100%", 1.0), ("110%", 1.1), ("125%", 1.25)]
                .into_iter()
                .map(|(n, k)| (n.to_string(), (k - l.scaling).abs() < 0.01, Msg::Scaling(k)))
                .collect(),
        );
        let preview = card()
            .gap(12.0)
            .child(text("Preview").medium())
            .child(
                row()
                    .flex_wrap()
                    .gap(8.0)
                    .items_center()
                    .child(primary_button("Load model"))
                    .child(button("Eject"))
                    .child(ghost_button("Details"))
                    .child(danger_button("Delete"))
                    .child(badge("Q4_K_M"))
                    .child(tag("running", c.success)),
            )
            .child(
                row()
                    .gap(12.0)
                    .items_center()
                    .child(switch(true))
                    .child(checkbox("Flash attention", true))
                    .child(progress(0.62).grow(1.0)),
            )
            .child(text_input(String::new(), |_| Msg::Noop).placeholder("Ask anything…"));
        col().grow(1.0).min_w(0.0).scroll_y().child(
            col()
                .w_full()
                .max_w(860.0)
                .px(32.0)
                .py(28.0)
                .gap(6.0)
                .child(
                    row()
                        .items_center()
                        .child(
                            col()
                                .grow(1.0)
                                .gap(4.0)
                                .child(text("Appearance").font_size(th.font_size_lg * 1.4).bold())
                                .child(
                                text(
                                    "Everything is generated from these knobs; nothing in the app hard-codes a color.",
                                )
                                .color(c.text_muted),
                            ),
                        )
                        .child(ghost_button("Reset").on_click(Msg::ResetLook)),
                )
                .child(setting(
                    "Mode",
                    "Light or dark surfaces",
                    segmented(vec![
                        ("Dark".into(), l.dark, Msg::Dark(true)),
                        ("Light".into(), !l.dark, Msg::Dark(false)),
                    ]),
                ))
                .child(setting(
                    "Style",
                    "Widget shapes, from theme style classes",
                    segmented(
                        StylePreset::ALL
                            .iter()
                            .map(|(n, p)| (n.to_string(), *p == l.preset, Msg::Preset(*p)))
                            .collect(),
                    ),
                ))
                .child(setting("Accent", "Buttons, focus, selection", swatches))
                .child(setting("Neutrals", "Tint of backgrounds and borders", grays))
                .child(setting(
                    "Corners",
                    "Base radius in px",
                    row()
                        .items_center()
                        .gap(12.0)
                        .child(slider(l.radius, 0.0, 12.0).step(0.5).on_change(Msg::Radius).w(220.0))
                        .child(text(format!("{:.1}px", l.radius)).mono().color(c.text_muted)),
                ))
                .child(setting("Density", "Control and row heights", densities))
                .child(setting("Text size", "Scales text and controls", sizes))
                .child(div().h(18.0))
                .child(preview),
        )
    }

    fn chat(&self) -> Element<Msg> {
        hsplit(
            "chat",
            vec![
                Pane::fixed(230.0, self.conversations()).min(180.0).max(360.0),
                Pane::fill(
                    col()
                        .min_w(0.0)
                        .child(self.header())
                        .child(self.transcript())
                        .child_if(self.show_params, || self.params())
                        .child(self.composer()),
                ),
            ],
        )
    }

    fn conversations(&self) -> Element<Msg> {
        let th = theme();
        let c = &th.colors;
        let q = self.search.to_lowercase();
        let mut list = col().gap(1.0).py(4.0);
        let mut folders: Vec<&str> = self.convos.iter().filter_map(|x| x.folder).collect();
        folders.dedup();
        for f in folders {
            let items = self
                .convos
                .iter()
                .enumerate()
                .filter(|(_, x)| x.folder == Some(f) && x.title.to_lowercase().contains(&q))
                .map(|(i, cv)| (i, cv.title, i == self.active))
                .collect();
            list = list.child(component(("folder", f), Folder { name: f, items, open_initially: f == "Research" }));
        }
        for (i, cv) in self.convos.iter().enumerate().filter(|(_, x)| x.folder.is_none()) {
            if cv.title.to_lowercase().contains(&q) {
                list = list.child(tree_row(0, None, None, cv.title, i == self.active).on_click(Msg::Select(i)));
            }
        }
        col()
            .bg(c.background)
            .child(
                col()
                    .p(10.0)
                    .gap(8.0)
                    .child(
                        row()
                            .gap(6.0)
                            .child(primary_button("New chat").with_icon(Icon::Plus).grow(1.0))
                            .child(tooltip_icon_button(Icon::Folder, "New folder")),
                    )
                    .child(search_input(self.search.clone(), Msg::Search)),
            )
            .child(list.grow(1.0).scroll_y())
    }

    fn header(&self) -> Element<Msg> {
        let th = theme();
        let c = &th.colors;
        row()
            .items_center()
            .gap(8.0)
            .h(48.0)
            .px(16.0)
            .shrink(0.0)
            .border_b(1.0, c.border)
            .child(
                combo_box(self.models.clone(), self.model, Msg::Model).dropdown_placeholder("Choose a model").w(280.0),
            )
            .child(spacer())
            .child_if(self.streaming.is_some(), || {
                text(format!("{:.0} tok/s", self.tok_per_s)).mono().font_size(12.0).color(c.accent)
            })
            .child(ghost_button("Params").with_icon(Icon::Settings).on_click(Msg::ToggleParams))
            .child(tooltip_icon_button(Icon::File, "Export chat as HTML"))
    }

    fn transcript(&self) -> Element<Msg> {
        let mut t = col().id("transcript").grow(1.0).scroll_y().follow_end().px(24.0).py(20.0).gap(18.0);
        for (i, m) in self.messages.iter().enumerate() {
            let last = i + 1 == self.messages.len();
            let streaming = last && self.streaming.is_some();
            // Finished messages don't rebuild their markdown every frame:
            // only a message whose text or state changed is rebuilt.
            let deps =
                (&m.text, m.reasoning.as_ref().map(|(r, secs)| (r, secs.to_bits())), m.show_reasoning, streaming);
            t = t.child(lazy(("message", i), deps, || self.message(i, m, streaming)));
        }
        t
    }

    fn message(&self, i: usize, m: &Message, streaming: bool) -> Element<Msg> {
        let th = theme();
        let c = th.colors.clone();
        {
            if m.user {
                return row().justify(Justify::End).child(
                    rich_text([span(m.text.clone())])
                        .selectable()
                        .max_w(pct(75.0))
                        .px(14.0)
                        .py(10.0)
                        .rounded(th.radius_lg)
                        .bg(c.accent.with_alpha(0.12))
                        .border(1.0, c.accent.with_alpha(0.28)),
                );
            }
            let mut body = col().gap(10.0).min_w(0.0).max_w(820.0);
            if let Some((reasoning, secs)) = &m.reasoning {
                body = body.child(
                    col()
                        .rounded(th.radius)
                        .border(1.0, c.border)
                        .bg(c.panel)
                        .child(
                            row()
                                .items_center()
                                .gap(8.0)
                                .h(30.0)
                                .px(10.0)
                                .color(c.text_muted)
                                .on_click(Msg::ToggleReasoning(i))
                                .child(
                                    icon(if m.show_reasoning { Icon::ChevronDown } else { Icon::ChevronRight })
                                        .font_size(13.0),
                                )
                                .child(text(format!("Thought for {secs:.1}s")).font_size(12.0)),
                        )
                        .child_if(m.show_reasoning, || {
                            text(reasoning.clone()).selectable().color(c.text_muted).italic().px(12.0).pb(10.0)
                        }),
                );
            }
            if m.text.is_empty() && streaming {
                body = body.child(text("typing…").color(c.text_faint).italic());
            } else {
                body = body.child(markdown(&m.text));
            }
            let actions = row()
                .gap(2.0)
                .color(c.text_faint)
                .child(tooltip_icon_button(Icon::Files, "Copy message").copy_on_click(m.text.clone()))
                .child(tooltip_icon_button(Icon::GitBranch, "Fork from here"))
                .child(tooltip_icon_button(Icon::Refresh, "Regenerate"))
                .child(tooltip_icon_button(Icon::Play, "Speak"));
            row()
                .gap(12.0)
                .items(Align::Start)
                .child(avatar("AI", c.accent.darken(0.2)))
                .child(body.child_if(!streaming, || actions).grow(1.0))
        }
    }

    fn params(&self) -> Element<Msg> {
        let th = theme();
        row()
            .gap(16.0)
            .items_center()
            .px(24.0)
            .py(10.0)
            .shrink(0.0)
            .border_t(1.0, th.colors.border)
            .child(text("Temperature").color(th.colors.text_muted))
            .child(slider(self.temperature, 0.0, 2.0).step(0.05).on_change(Msg::Temperature).w(220.0))
            .child(text(format!("{:.2}", self.temperature)).mono())
            .child(pick_list(["Balanced", "Precise", "Creative"], Some(0), |_| Msg::Noop).w(150.0))
    }

    fn composer(&self) -> Element<Msg> {
        let th = theme();
        let c = &th.colors;
        let send = if self.streaming.is_some() {
            danger_button("Stop").with_icon(Icon::Minus).on_click(Msg::Stop)
        } else {
            primary_button("Send")
                .with_icon(Icon::ChevronRight)
                .on_click(Msg::Send)
                .disabled(self.draft.trim().is_empty())
        };
        col()
            .shrink(0.0)
            .px(24.0)
            .pt(8.0)
            .pb(16.0)
            .gap(8.0)
            .child(
                row()
                    .gap(8.0)
                    .items(Align::End)
                    .p(8.0)
                    .rounded(th.radius_lg)
                    .bg(c.panel)
                    .border(1.0, c.border_strong)
                    .child(
                        text_area(self.draft.clone(), Msg::Draft)
                            .id("composer")
                            .placeholder("Message the model — Enter to send, Shift+Enter for a new line")
                            .rows(1, 8)
                            .submit_on_enter(Msg::Send)
                            .grow(1.0)
                            .border(0.0, c.border)
                            .bg(c.panel),
                    )
                    .child(send),
            )
            .child(
                row()
                    .gap(4.0)
                    .color(c.text_faint)
                    .child(ghost_button("Document").with_icon(Icon::File).font_size(12.0).h(26.0))
                    .child(ghost_button("Image").with_icon(Icon::Layers).font_size(12.0).h(26.0))
                    .child(ghost_button("Audio").with_icon(Icon::Bell).font_size(12.0).h(26.0))
                    .child(spacer())
                    .child(text(format!("{} messages", self.messages.len())).font_size(11.5).mono()),
            )
    }
}

const MOCK_REPLY: &str = "Here's how to **stream tokens** into the UI without blocking it:\n\n1. Spawn the request as a stream with `cx.run`.\n2. Append each chunk in `update`.\n3. Keep the transcript pinned with `follow_end()`.\n\n```rust\nlet handle = cx.run(token_stream, Msg::Token);\n// later, on Stop:\nhandle.abort();\n```\n\n| Model | Speed |\n|---|---|\n| qwen2.5-7b | 92 tok/s |\n| llama-3.1-8b | 85 tok/s |\n\n> Tip: select any part of this reply and press **Ctrl+C**.\n";

fn initial() -> LmFast {
    LmFast {
        screen: Screen::Chat,
        models: vec![
            "qwen2.5-7b-instruct-q4_k_m",
            "llama-3.1-8b-instruct-q8_0",
            "qwen2.5-coder-32b-q4_k_m",
            "phi-4-q6_k",
        ],
        model: Some(0),
        convos: vec![
            Convo { title: "Streaming UI in Rust", folder: None },
            Convo { title: "GGUF quant comparison", folder: Some("Research") },
            Convo { title: "Speculative decoding", folder: Some("Research") },
            Convo { title: "Weekend plans", folder: None },
            Convo { title: "Ollama API shim", folder: Some("Work") },
        ],
        active: 0,
        search: String::new(),
        messages: vec![
            Message {
                user: true,
                text: "How do I stream LLM tokens into a native Rust UI?".into(),
                reasoning: None,
                show_reasoning: false,
            },
            Message {
                user: false,
                text: MOCK_REPLY.into(),
                reasoning: Some(("The user wants a streaming pattern. Show tasks, update, and scrolling.".into(), 2.4)),
                show_reasoning: true,
            },
        ],
        draft: String::new(),
        streaming: None,
        started: None,
        tokens: 0,
        tok_per_s: 0.0,
        confirm_quit: false,
        show_params: false,
        temperature: 0.7,
        look: Appearance::default(),
        log: Rc::new((0..LOG_CAP / 2).map(log_line).collect()),
        library: Rc::new(mock_models()),
        lib_search: String::new(),
        lib_sort: (3, SortDir::Asc),
        lib_selected: Some(0),
        log_filter: None,
        log_live: true,
        log_at_end: true,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(pos) = args.iter().position(|a| a == "--screenshot") {
        let out = args.get(pos + 1).cloned().unwrap_or_else(|| "lmfast_chat.png".into());
        let arg =
            |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(|s| s.to_lowercase());
        let mut app = initial();
        if args.iter().any(|a| a == "--light") {
            app.look.dark = false;
        }
        if let Some(a) = arg("--accent") {
            app.look.accent = *Accent::ALL.iter().find(|x| x.name().to_lowercase() == a).expect("unknown accent");
        }
        if let Some(g) = arg("--gray") {
            app.look.gray = GRAYS.iter().find(|(n, _)| n.to_lowercase() == g).expect("unknown gray").1;
        }
        if let Some(p) = arg("--style") {
            app.look.preset = StylePreset::ALL.iter().find(|(n, _)| n.to_lowercase() == p).expect("unknown style").1;
        }
        if let Some(r) = arg("--radius") {
            app.look.radius = r.parse().expect("radius");
        }
        if args.iter().any(|a| a == "--settings") {
            app.screen = Screen::Settings;
        }
        if args.iter().any(|a| a == "--models") {
            app.screen = Screen::Models;
        }
        if args.iter().any(|a| a == "--developer") {
            app.screen = Screen::Developer;
        }
        let mut h = Headless::new(app, 1360.0, 860.0, 1.0);
        h.settle();
        if args.iter().any(|a| a == "--streaming") {
            h.rt.send(Msg::Draft("Show me a table of model speeds".into()));
            h.rt.send(Msg::Send);
            h.wait_until(Duration::from_secs(5), |a| a.messages.last().is_some_and(|m| m.text.len() > 160));
            h.advance(0.3);
        }
        if args.iter().any(|a| a == "--scrolled") {
            // Scroll the log up while it keeps streaming: the view must stay put.
            for _ in 0..12 {
                h.event(rust_ui::runtime::Event::Wheel(Point::new(700.0, 500.0), Point::new(0.0, -120.0)));
                h.advance(0.05);
            }
            h.advance(1.0);
        }
        h.settle();
        h.save_png(&out).expect("save");
        println!("saved {out}");
        return;
    }
    let opts = WindowOptions::new("LM Fast").size(1360.0, 860.0).min_size(940.0, 600.0);
    rust_ui::run(initial(), opts).expect("run");
}
