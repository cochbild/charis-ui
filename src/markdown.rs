//! Markdown rendering (CommonMark + tables, task lists, strikethrough).
//!
//! ```
//! # use charis_ui::prelude::*;
//! # #[derive(Clone)] enum Msg { Open(String) }
//! let md: Element<Msg> = markdown("# Title\nSome **bold** text and `code`.\n\n```rust\nfn main() {}\n```")
//!     .on_link(Msg::Open);
//! ```
//!
//! Paragraphs are selectable rich text; code blocks get a language label and a
//! copy button. Rendering is cheap enough to re-run on every streamed token:
//! parsing is fast and unchanged paragraphs hit the text-layout cache.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::element::*;
use crate::icons::Icon;
use crate::style::*;
use crate::text::{span, Span};
use crate::theme::theme;
use crate::widgets::tooltip_icon_button;

#[derive(Debug, Clone, PartialEq)]
enum Block {
    Para(Vec<Span>),
    Heading(u8, Vec<Span>),
    Code(Option<String>, String),
    List(Option<u64>, Vec<(Option<bool>, Vec<Block>)>),
    Quote(Vec<Block>),
    Rule,
    Table(Vec<Vec<Vec<Span>>>),
}

#[derive(Default, Clone)]
struct InlineStyle {
    bold: u32,
    italic: u32,
    link: Option<String>,
}

struct P<'a> {
    events: std::iter::Peekable<Parser<'a>>,
}

impl<'a> P<'a> {
    /// Parse blocks until the matching end tag (or end of input).
    fn blocks(&mut self, until: Option<TagEnd>) -> Vec<Block> {
        let mut out = Vec::new();
        let mut loose: Vec<Span> = Vec::new(); // inline content outside a paragraph (tight lists)
        let mut task: Option<bool> = None;
        // Bold, italic and links of the loose inline content, across events.
        let mut loose_style = InlineStyle::default();
        while let Some(ev) = self.events.next() {
            match ev {
                Event::End(e) if Some(e) == until => break,
                Event::Start(Tag::Paragraph) => {
                    flush(&mut loose, &mut out);
                    let spans = self.inlines(TagEnd::Paragraph);
                    out.push(Block::Para(spans));
                }
                Event::Start(Tag::Heading { level, .. }) => {
                    flush(&mut loose, &mut out);
                    let spans = self.inlines(TagEnd::Heading(level));
                    let n = match level {
                        HeadingLevel::H1 => 1,
                        HeadingLevel::H2 => 2,
                        HeadingLevel::H3 => 3,
                        HeadingLevel::H4 => 4,
                        HeadingLevel::H5 => 5,
                        HeadingLevel::H6 => 6,
                    };
                    out.push(Block::Heading(n, spans));
                }
                Event::Start(Tag::CodeBlock(kind)) => {
                    flush(&mut loose, &mut out);
                    let lang = match kind {
                        CodeBlockKind::Fenced(l) if !l.is_empty() => {
                            Some(l.split_whitespace().next().unwrap_or("").to_string())
                        }
                        _ => None,
                    };
                    let mut code = String::new();
                    for ev in self.events.by_ref() {
                        match ev {
                            Event::Text(t) => code.push_str(&t),
                            Event::End(TagEnd::CodeBlock) => break,
                            _ => {}
                        }
                    }
                    if code.ends_with('\n') {
                        code.pop();
                    }
                    out.push(Block::Code(lang, code));
                }
                Event::Start(Tag::List(start)) => {
                    flush(&mut loose, &mut out);
                    let mut items = Vec::new();
                    loop {
                        match self.events.next() {
                            Some(Event::Start(Tag::Item)) => {
                                let mut body = self.blocks(Some(TagEnd::Item));
                                let mut checked = None;
                                if let Some(Block::Para(sp)) = body.first_mut() {
                                    if let Some(first) = sp.first() {
                                        if first.text.starts_with('\u{1}') {
                                            checked = Some(first.text.starts_with("\u{1}x"));
                                            sp.remove(0);
                                        }
                                    }
                                }
                                items.push((checked, body));
                            }
                            Some(Event::End(TagEnd::List(_))) | None => break,
                            _ => {}
                        }
                    }
                    out.push(Block::List(start, items));
                }
                Event::Start(Tag::BlockQuote(_)) => {
                    flush(&mut loose, &mut out);
                    let inner = self.blocks(Some(TagEnd::BlockQuote(None)));
                    out.push(Block::Quote(inner));
                }
                Event::End(TagEnd::BlockQuote(_)) if matches!(until, Some(TagEnd::BlockQuote(_))) => break,
                Event::Start(Tag::Table(_)) => {
                    flush(&mut loose, &mut out);
                    out.push(Block::Table(self.table()));
                }
                Event::Rule => {
                    flush(&mut loose, &mut out);
                    out.push(Block::Rule);
                }
                Event::TaskListMarker(done) => task = Some(done),
                // Inline content directly in a block container (tight list items).
                other => {
                    if let Some(done) = task.take() {
                        loose.push(span(if done { "\u{1}x" } else { "\u{1} " }));
                    }
                    self.inline_event(other, &mut loose_style, &mut loose);
                }
            }
        }
        if let Some(done) = task {
            loose.insert(0, span(if done { "\u{1}x" } else { "\u{1} " }));
        }
        flush(&mut loose, &mut out);
        out
    }

    fn table(&mut self) -> Vec<Vec<Vec<Span>>> {
        let mut rows: Vec<Vec<Vec<Span>>> = Vec::new();
        let mut row: Vec<Vec<Span>> = Vec::new();
        while let Some(ev) = self.events.next() {
            match ev {
                Event::Start(Tag::TableCell) => row.push(self.inlines(TagEnd::TableCell)),
                Event::End(TagEnd::TableRow) | Event::End(TagEnd::TableHead) => rows.push(std::mem::take(&mut row)),
                Event::End(TagEnd::Table) => break,
                _ => {}
            }
        }
        rows
    }

    fn inlines(&mut self, until: TagEnd) -> Vec<Span> {
        let mut out = Vec::new();
        let mut st = InlineStyle::default();
        while let Some(ev) = self.events.next() {
            if let Event::End(e) = &ev {
                if *e == until {
                    break;
                }
            }
            self.inline_event(ev, &mut st, &mut out);
        }
        out
    }

    fn inline_event(&mut self, ev: Event<'a>, st: &mut InlineStyle, out: &mut Vec<Span>) {
        let styled = |t: &str, st: &InlineStyle| {
            let mut s = span(t);
            if st.bold > 0 {
                s = s.bold();
            }
            if st.italic > 0 {
                s = s.italic();
            }
            if let Some(l) = &st.link {
                s = s.link(l.clone());
            }
            s
        };
        match ev {
            Event::Text(t) => out.push(styled(&t, st)),
            Event::Code(t) => {
                let mut s = styled(&t, st).mono();
                if st.link.is_none() {
                    s = s.color(theme().colors.code_text);
                }
                out.push(s);
            }
            Event::InlineMath(t) | Event::DisplayMath(t) => out.push(styled(&t, st).italic()),
            Event::SoftBreak => out.push(styled(" ", st)),
            Event::HardBreak => out.push(styled("\n", st)),
            Event::Html(t) | Event::InlineHtml(t) => out.push(styled(&t, st)),
            Event::Start(Tag::Strong) => st.bold += 1,
            Event::End(TagEnd::Strong) => st.bold = st.bold.saturating_sub(1),
            Event::Start(Tag::Emphasis) => st.italic += 1,
            Event::End(TagEnd::Emphasis) => st.italic = st.italic.saturating_sub(1),
            Event::Start(Tag::Link { dest_url, .. }) => st.link = Some(dest_url.to_string()),
            Event::End(TagEnd::Link) => st.link = None,
            Event::Start(Tag::Image { dest_url, .. }) => {
                // Images render as their alt text, linked to the source.
                st.link = Some(dest_url.to_string());
            }
            Event::End(TagEnd::Image) => st.link = None,
            _ => {}
        }
    }
}

fn flush(loose: &mut Vec<Span>, out: &mut Vec<Block>) {
    if !loose.is_empty() {
        out.push(Block::Para(std::mem::take(loose)));
    }
}

fn parse(src: &str) -> Vec<Block> {
    let opts = Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH;
    P { events: Parser::new_ext(src, opts).peekable() }.blocks(None)
}

/// Render Markdown as an element tree. Attach `.on_link(...)` to receive link clicks.
pub fn markdown<M: Clone + 'static>(src: &str) -> Element<M> {
    let blocks = parse(src);
    col().gap(10.0).min_w(0.0).children(blocks.iter().map(render_block::<M>))
}

fn render_block<M: Clone + 'static>(b: &Block) -> Element<M> {
    let th = theme();
    let c = &th.colors;
    let base = th.font_size;
    match b {
        Block::Para(spans) => rich_text(spans.iter().cloned()).selectable().line_height(1.6),
        Block::Heading(level, spans) => {
            let (k, w) = match level {
                1 => (1.7, 700),
                2 => (1.4, 700),
                3 => (1.2, 650),
                _ => (1.05, 600),
            };
            rich_text(spans.iter().cloned().map(|s| s.weight(w)))
                .selectable()
                .font_size((base * k).round())
                .line_height(1.3)
                .mt(if *level <= 2 { 6.0 } else { 2.0 })
                .heading(*level)
        }
        Block::Code(lang, code) => {
            let header = row()
                .items_center()
                .h(28.0)
                .pl(12.0)
                .pr(4.0)
                .border_b(1.0, c.border)
                .child(
                    text(lang.clone().unwrap_or_else(|| "text".into())).font_size(11.5).color(c.text_faint).grow(1.0),
                )
                .child(tooltip_icon_button(Icon::Files, "Copy").aria_label("Copy code").copy_on_click(code.clone()));
            col().bg(c.input).border(1.0, c.border).rounded(th.radius).clip().min_w(0.0).child(header).child(
                div()
                    .scroll_x()
                    .px(12.0)
                    .py(10.0)
                    .child(text(code.clone()).mono().nowrap().selectable().font_size(base - 0.5).line_height(1.55)),
            )
        }
        Block::List(start, items) => col().gap(4.0).children(items.iter().enumerate().map(|(i, (task, body))| {
            let marker: Element<M> = match (task, start) {
                (Some(done), _) => div()
                    .center()
                    .square(14.0)
                    .mt(3.0)
                    .rounded(3.0)
                    .border(1.0, if *done { c.accent } else { c.border_strong })
                    .when(*done, |d| {
                        d.bg(c.accent).color(c.accent_text).child(icon(Icon::Check).font_size(10.0).bold())
                    }),
                (None, Some(n)) => {
                    text(format!("{}.", n + i as u64)).color(c.text_muted).w(22.0).text_align(TextAlign::Right)
                }
                (None, None) => div().w(22.0).child(div().square(5.0).pill().bg(c.text_muted).mt(8.0).ml(9.0)),
            };
            row()
                .gap(8.0)
                .items(Align::Start)
                .child(marker.shrink(0.0))
                .child(col().gap(6.0).grow(1.0).min_w(0.0).children(body.iter().map(render_block::<M>)))
        })),
        Block::Quote(inner) => row().child(div().w(3.0).rounded(2.0).bg(c.border_strong).shrink(0.0)).child(
            col()
                .gap(8.0)
                .pl(12.0)
                .grow(1.0)
                .min_w(0.0)
                .color(c.text_muted)
                .children(inner.iter().map(render_block::<M>)),
        ),
        Block::Rule => div().h(1.0).w_full().bg(c.border).my(4.0),
        Block::Table(rows) => {
            let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0).max(1);
            let mut grid = div()
                .grid(vec![Track::Auto; cols])
                .border(1.0, c.border)
                .rounded(th.radius)
                .clip()
                .self_align(Align::Start);
            for (ri, r) in rows.iter().enumerate() {
                for ci in 0..cols {
                    let spans = r.get(ci).cloned().unwrap_or_default();
                    let mut cell = rich_text(spans).selectable().px(10.0).py(6.0).border_b(1.0, c.border);
                    if ri == 0 {
                        cell = cell.semibold().bg(c.hover);
                    }
                    grid = grid.child(cell);
                }
            }
            grid
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(spans: &[Span]) -> String {
        spans.iter().map(|s| s.text.as_str()).collect()
    }

    #[test]
    fn parses_common_blocks() {
        let md = "# Title\n\nHello **bold** and *it* with `code` and [link](http://x).\n\n- a\n- b\n  1. nested\n\n> quote\n\n```rust\nfn main() {}\n```\n\n---\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n- [x] done\n- [ ] todo\n";
        let b = parse(md);
        assert!(matches!(&b[0], Block::Heading(1, s) if texts(s) == "Title"));
        match &b[1] {
            Block::Para(s) => {
                assert_eq!(texts(s), "Hello bold and it with code and link.");
                assert!(s.iter().any(|x| x.text == "bold" && x.weight == Some(700)));
                assert!(s.iter().any(|x| x.text == "it" && x.italic));
                assert!(s.iter().any(|x| x.text == "code" && x.mono));
                assert!(s.iter().any(|x| x.text == "link" && x.link.as_deref() == Some("http://x")));
            }
            other => panic!("{other:?}"),
        }
        match &b[2] {
            Block::List(None, items) => {
                assert_eq!(items.len(), 2);
                assert!(matches!(&items[1].1[1], Block::List(Some(1), _)), "nested ordered list: {:?}", items[1]);
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(&b[3], Block::Quote(q) if matches!(&q[0], Block::Para(_))));
        assert!(matches!(&b[4], Block::Code(Some(l), c) if l == "rust" && c == "fn main() {}"));
        assert!(matches!(&b[5], Block::Rule));
        assert!(matches!(&b[6], Block::Table(rows) if rows.len() == 2 && rows[1].len() == 2));
        match &b[7] {
            Block::List(None, items) => {
                assert_eq!(items[0].0, Some(true));
                assert_eq!(items[1].0, Some(false));
                assert!(matches!(&items[0].1[0], Block::Para(s) if texts(s) == "done"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn tight_list_items_keep_emphasis() {
        let b = parse("- **Faster** layout with *incremental* [updates](u)\n- plain\n");
        let Block::List(None, items) = &b[0] else { panic!("{b:?}") };
        let Block::Para(s) = &items[0].1[0] else { panic!("{:?}", items[0]) };
        assert!(s.iter().any(|x| x.text == "Faster" && x.weight == Some(700)), "{s:?}");
        assert!(s.iter().any(|x| x.text == " layout with " && x.weight.is_none() && !x.italic), "{s:?}");
        assert!(s.iter().any(|x| x.text == "incremental" && x.italic), "{s:?}");
        assert!(s.iter().any(|x| x.text == "updates" && x.link.as_deref() == Some("u")), "{s:?}");
        let Block::Para(s) = &items[1].1[0] else { panic!("{:?}", items[1]) };
        assert!(s.iter().all(|x| x.weight.is_none() && !x.italic), "style leaks into the next item: {s:?}");
    }

    #[test]
    fn partial_streaming_input_is_fine() {
        // Unterminated fences and emphasis (mid-stream) must not panic.
        for md in ["```py\nprint(1", "**bol", "| a |\n|--", "- [", "> "] {
            let _ = parse(md);
        }
    }
}
