//! Drag-and-drop docking: drag any tab onto another panel's center to join it,
//! or onto an edge to split it. Every splitter is resizable.
//!
//! Run:        cargo run --release --example dock
//! Screenshot: cargo run --release --example dock -- --screenshot dock.png

use rust_ui::prelude::*;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Panel {
    Explorer,
    Outline,
    Editor(&'static str),
    Terminal,
    Problems,
    Preview,
    Chat,
}

impl Panel {
    fn title(&self) -> String {
        match self {
            Panel::Explorer => "Explorer".into(),
            Panel::Outline => "Outline".into(),
            Panel::Editor(f) => f.to_string(),
            Panel::Terminal => "Terminal".into(),
            Panel::Problems => "Problems".into(),
            Panel::Preview => "Preview".into(),
            Panel::Chat => "Assistant".into(),
        }
    }
    fn icon(&self) -> Icon {
        match self {
            Panel::Explorer => Icon::Files,
            Panel::Outline => Icon::Layers,
            Panel::Editor(_) => Icon::Code,
            Panel::Terminal => Icon::Terminal,
            Panel::Problems => Icon::Warning,
            Panel::Preview => Icon::Play,
            Panel::Chat => Icon::User,
        }
    }
}

struct DockDemo {
    dock: Dock<Panel>,
    dark: bool,
}

#[derive(Clone)]
enum Msg {
    Dock(DockMsg),
    Theme,
}

impl App for DockDemo {
    type Msg = Msg;
    fn theme(&self) -> Theme {
        if self.dark {
            Theme::dark()
        } else {
            Theme::light()
        }
    }
    fn update(&mut self, msg: Msg, _cx: &mut Cx) {
        match msg {
            Msg::Dock(m) => self.dock.update(m),
            Msg::Theme => self.dark = !self.dark,
        }
    }
    fn view(&self) -> Element<Msg> {
        let th = theme();
        let bar = titlebar(
            "Docking demo — drag tabs onto panel edges",
            row()
                .pl(12.0)
                .gap(8.0)
                .items_center()
                .child(icon(Icon::Grid).font_size(16.0).color(th.colors.accent))
                .child(text("Dock").semibold()),
            row().pr(6.0).child(
                icon_button(if self.dark { Icon::Sun } else { Icon::Moon })
                    .on_click(Msg::Theme)
                    .tooltip("Toggle theme"),
            ),
            window_info().maximized,
        );
        col().size_full().child(bar).child(self.dock.view_with_icons(
            "dock",
            Panel::title,
            |p| Some(p.icon()),
            panel_content,
            Msg::Dock,
        ))
    }
}

fn panel_content(p: &Panel) -> Element<Msg> {
    let th = theme();
    let c = &th.colors;
    match p {
        Panel::Explorer => col().py(6.0).scroll_y().children(
            ["src", "main.rs", "app.rs", "dock.rs", "theme.rs", "Cargo.toml", "README.md"].iter().enumerate().map(
                |(i, f)| {
                    tree_row(
                        if i == 0 { 0 } else { 1 },
                        if i == 0 { Some(true) } else { None },
                        Some(if i == 0 { Icon::Folder } else { Icon::File }),
                        *f,
                        i == 1,
                    )
                },
            ),
        ),
        Panel::Outline => col().p(12.0).gap(6.0).color(c.text_muted).children(
            ["struct DockDemo", "enum Msg", "impl App", "fn panel_content", "fn main"]
                .map(|s| text(s).mono().font_size(12.0)),
        ),
        Panel::Editor(f) => col()
            .p(18.0)
            .gap(4.0)
            .mono()
            .font_size(13.0)
            .child(text(format!("// {f}")).color(c.text_faint))
            .child(text("fn main() {").color(c.text))
            .child(text("    println!(\"hello, docking\");").color(c.success))
            .child(text("}").color(c.text)),
        Panel::Terminal => col()
            .p(12.0)
            .mono()
            .font_size(12.5)
            .gap(2.0)
            .child(text("$ cargo run --example dock").color(c.text))
            .child(text("   Running `target/release/examples/dock`").color(c.text_muted)),
        Panel::Problems => col().p(12.0).gap(6.0).child(
            row()
                .gap(8.0)
                .items_center()
                .child(icon(Icon::Warning).color(c.warning))
                .child(text("unused import `Rect`").color(c.text_muted)),
        ),
        Panel::Preview => col()
            .center()
            .gap(10.0)
            .child(
                div()
                    .w(160.0)
                    .h(100.0)
                    .rounded(12.0)
                    .gradient(135.0, [(0.0, c.accent), (1.0, hex("#a371f7"))])
                    .shadows(th.shadow_popover.clone()),
            )
            .child(text("Live preview").color(c.text_muted)),
        Panel::Chat => {
            col()
                .p(12.0)
                .gap(10.0)
                .child(card().p(10.0).child(
                    text("Try dragging the “Terminal” tab onto the right edge of the editor.").color(c.text_muted),
                ))
                .child(spacer())
                .child(text_input("", |_| Msg::Theme).placeholder("Ask something…"))
        }
    }
}

fn initial() -> Dock<Panel> {
    Dock::new(DockNode::hsplit(vec![
        (
            1.0,
            DockNode::vsplit(vec![
                (2.0, DockNode::tabs(vec![Panel::Explorer])),
                (1.0, DockNode::tabs(vec![Panel::Outline])),
            ]),
        ),
        (
            3.2,
            DockNode::vsplit(vec![
                (
                    2.4,
                    DockNode::tabs(vec![Panel::Editor("main.rs"), Panel::Editor("app.rs"), Panel::Editor("dock.rs")]),
                ),
                (1.0, DockNode::tabs(vec![Panel::Terminal, Panel::Problems])),
            ]),
        ),
        (
            1.3,
            DockNode::vsplit(vec![
                (1.0, DockNode::tabs(vec![Panel::Preview])),
                (1.0, DockNode::tabs(vec![Panel::Chat])),
            ]),
        ),
    ]))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(pos) = args.iter().position(|a| a == "--screenshot") {
        let out = args.get(pos + 1).cloned().unwrap_or_else(|| "dock.png".into());
        let mut h = Headless::new(DockDemo { dock: initial(), dark: true }, 1280.0, 800.0, 1.0);
        h.settle();
        // Drag "Terminal" (bottom-center group) toward the editor's right edge, and
        // capture the frame mid-drag to show the drop preview.
        let from =
            (h.rt.rect_of_text("Terminal").unwrap().center().x, h.rt.rect_of_text("Terminal").unwrap().center().y);
        h.move_to(from.0, from.1);
        h.event(rust_ui::Event::PointerDown(Point::new(from.0, from.1), rust_ui::MouseButton::Left));
        for i in 1..=10 {
            let t = i as f32 / 10.0;
            h.move_to(from.0 + (900.0 - from.0) * t, from.1 + (260.0 - from.1) * t);
        }
        h.settle();
        h.save_png(&out).expect("save");
        h.event(rust_ui::Event::PointerUp(Point::new(900.0, 260.0), rust_ui::MouseButton::Left));
        h.settle();
        let after = out.replace(".png", "-after.png");
        h.save_png(&after).expect("save");
        println!("saved {out} and {after}");
        return;
    }
    rust_ui::run(
        DockDemo { dock: initial(), dark: true },
        WindowOptions::new("Dock demo").size(1280.0, 800.0).frameless(true),
    )
    .expect("run");
}
