//! A recreation of lmfast-rs's Chat screen (originally built with iced) to
//! compare look and feature parity. Streaming is mocked.
//!
//! Run:        cargo run --release --example lmfast_chat
//! Screenshot: cargo run --release --example lmfast_chat -- --screenshot chat.png [--streaming]

use std::time::{Duration, Instant};

use futures::StreamExt;
use rust_ui::prelude::*;
use rust_ui::TaskHandle;

// lmfast's owned palette: signal amber on flat near-black.
const BG: &str = "#0D0D0F";
const AMBER: &str = "#FFB000";

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
    open_folders: Vec<&'static str>,
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
}

#[derive(Clone, Debug)]
enum Msg {
    Switch(Screen),
    Model(usize),
    Select(usize),
    ToggleFolder(&'static str),
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
    CloseRequested,
    CancelQuit,
    Quit,
    Noop,
}

impl App for LmFast {
    type Msg = Msg;

    fn theme(&self) -> Theme {
        // Generate a full theme from lmfast's accent, then pin exact brand colors.
        let mut t = Theme::from_config(ThemeConfig {
            accent: hex(AMBER),
            gray: GrayTint::Custom { hue: 285.0, chroma: 0.006 },
            radius: 5.0,
            ..ThemeConfig::dark()
        });
        t.colors.background = hex(BG);
        t.colors.surface = hex(BG);
        t.colors.chrome = hex("#0A0A0C");
        t.colors.panel = hex("#131316");
        t.colors.text = hex("#E6E6EA");
        t.colors.success = hex("#4ADE80");
        t.colors.danger = hex("#F0506E");
        t.radius_lg = 8.0;
        t
    }

    fn subscriptions(&self) -> Subscriptions<Msg> {
        Subscriptions::none()
            .every_if(self.streaming.is_some(), Duration::from_millis(250), Msg::Tick)
            .on_close_request(Msg::CloseRequested)
    }

    fn update(&mut self, msg: Msg, cx: &mut Cx<Msg>) {
        match msg {
            Msg::Switch(s) => self.screen = s,
            Msg::Model(i) => self.model = Some(i),
            Msg::Select(i) => self.active = i,
            Msg::ToggleFolder(f) => {
                if let Some(p) = self.open_folders.iter().position(|x| *x == f) {
                    self.open_folders.remove(p);
                } else {
                    self.open_folders.push(f);
                }
            }
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
            let open = self.open_folders.contains(&f);
            list = list.child(tree_row(0, Some(open), Some(Icon::Folder), f, false).on_click(Msg::ToggleFolder(f)));
            if open {
                for (i, cv) in self.convos.iter().enumerate().filter(|(_, x)| x.folder == Some(f)) {
                    if cv.title.to_lowercase().contains(&q) {
                        list = list.child(tree_row(1, None, None, cv.title, i == self.active).on_click(Msg::Select(i)));
                    }
                }
            }
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
        let th = theme();
        let c = th.colors.clone();
        let mut t = col().id("transcript").grow(1.0).scroll_y().follow_end().px(24.0).py(20.0).gap(18.0);
        for (i, m) in self.messages.iter().enumerate() {
            let last = i + 1 == self.messages.len();
            if m.user {
                t = t.child(
                    row().justify(Justify::End).child(
                        rich_text([span(m.text.clone())])
                            .selectable()
                            .max_w(pct(75.0))
                            .px(14.0)
                            .py(10.0)
                            .rounded(th.radius_lg)
                            .bg(c.accent.with_alpha(0.12))
                            .border(1.0, c.accent.with_alpha(0.28)),
                    ),
                );
                continue;
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
            if m.text.is_empty() && self.streaming.is_some() && last {
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
            t = t.child(
                row()
                    .gap(12.0)
                    .items(Align::Start)
                    .child(avatar("AI", c.accent.darken(0.2)))
                    .child(body.child_if(!(last && self.streaming.is_some()), || actions).grow(1.0)),
            );
        }
        t
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
        open_folders: vec!["Research"],
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
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(pos) = args.iter().position(|a| a == "--screenshot") {
        let out = args.get(pos + 1).cloned().unwrap_or_else(|| "lmfast_chat.png".into());
        let mut h = Headless::new(initial(), 1360.0, 860.0, 1.0);
        h.settle();
        if args.iter().any(|a| a == "--streaming") {
            h.rt.send(Msg::Draft("Show me a table of model speeds".into()));
            h.rt.send(Msg::Send);
            h.wait_until(Duration::from_secs(5), |a| a.messages.last().is_some_and(|m| m.text.len() > 160));
            h.advance(0.3);
        }
        h.settle();
        h.save_png(&out).expect("save");
        println!("saved {out}");
        return;
    }
    let opts = WindowOptions::new("LM Fast").size(1360.0, 860.0).min_size(940.0, 600.0);
    rust_ui::run(initial(), opts).expect("run");
}
