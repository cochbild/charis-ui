//! Ready-made widgets styled from the active [`Theme`](crate::Theme).
//!
//! Every widget is an ordinary [`Element`], so everything can be restyled
//! after construction with the usual builder methods:
//!
//! ```no_run
//! # use rust_ui::prelude::*;
//! # #[derive(Clone)] enum Msg { Go }
//! let b: Element<Msg> = primary_button("Deploy").pill().px(20.0).on_click(Msg::Go);
//! ```

use std::rc::Rc;

use crate::color::Color;
use crate::element::*;
use crate::geometry::Point;
use crate::icons::Icon;
use crate::style::*;
use crate::theme::theme;

// ---------------------------------------------------------------- buttons

/// Visual variants for buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    /// Neutral bordered button.
    Secondary,
    /// Filled accent button.
    Primary,
    /// Transparent until hovered.
    Ghost,
    /// Destructive action.
    Danger,
}

/// A button with the given variant and label.
pub fn button_kind<M: 'static>(kind: ButtonKind, label: impl Into<String>) -> Element<M> {
    let th = theme();
    let c = &th.colors;
    let base = row()
        .items_center()
        .justify(Justify::Center)
        .gap(6.0)
        .h(th.control_height)
        .px(12.0)
        .rounded(th.radius)
        .medium()
        .shrink(0.0)
        .focusable()
        .transition(th.transition)
        .child(text(label).nowrap());
    match kind {
        ButtonKind::Primary => base
            .bg(c.accent)
            .color(c.accent_text)
            .shadows(th.shadow_sm.clone())
            .hover(|s| s.bg(c.accent_hover))
            .active(|s| s.bg(c.accent.darken(0.1)).translate(0.0, 0.5)),
        ButtonKind::Secondary => base
            .bg(c.elevated)
            .border(1.0, c.border_strong)
            .color(c.text)
            .shadows(th.shadow_sm.clone())
            .hover(|s| s.bg(c.elevated.blend(c.hover)).border_color(c.border_strong.lighten(0.08)))
            .active(|s| s.bg(c.elevated.blend(c.pressed)).translate(0.0, 0.5)),
        ButtonKind::Ghost => {
            base.color(c.text_muted).hover(|s| s.bg(c.hover).color(c.text)).active(|s| s.bg(c.pressed))
        }
        ButtonKind::Danger => base
            .bg(c.danger)
            .color(Color::WHITE)
            .hover(|s| s.bg(c.danger.lighten(0.1)))
            .active(|s| s.bg(c.danger.darken(0.1)).translate(0.0, 0.5)),
    }
}

/// A neutral (secondary) button.
pub fn button<M: 'static>(label: impl Into<String>) -> Element<M> {
    button_kind(ButtonKind::Secondary, label)
}

/// A filled accent button.
pub fn primary_button<M: 'static>(label: impl Into<String>) -> Element<M> {
    button_kind(ButtonKind::Primary, label)
}

/// A transparent button.
pub fn ghost_button<M: 'static>(label: impl Into<String>) -> Element<M> {
    button_kind(ButtonKind::Ghost, label)
}

/// A destructive button.
pub fn danger_button<M: 'static>(label: impl Into<String>) -> Element<M> {
    button_kind(ButtonKind::Danger, label)
}

/// A square, transparent icon-only button (toolbar style).
pub fn icon_button<M: 'static>(i: Icon) -> Element<M> {
    let th = theme();
    let c = &th.colors;
    div()
        .center()
        .square(28.0)
        .rounded(th.radius)
        .shrink(0.0)
        .color(c.text_muted)
        .focusable()
        .transition(th.transition)
        .hover(|s| s.bg(c.hover).color(c.text))
        .active(|s| s.bg(c.pressed))
        .child(icon(i).font_size(16.0))
}

impl<M: 'static> Element<M> {
    /// Prepend an icon to a button-like row.
    pub fn with_icon(mut self, i: Icon) -> Self {
        let size = self.style.font_size.unwrap_or(theme().font_size) + 2.0;
        self.children.insert(0, icon(i).font_size(size));
        self
    }
}

// ----------------------------------------------------------------- inputs

/// A single-line text input. The value is controlled by the app: handle
/// `on_input` and store the new value.
pub fn text_input<M: 'static>(value: impl Into<String>, on_input: impl Fn(String) -> M + 'static) -> Element<M> {
    let th = theme();
    let c = &th.colors;
    let mut e =
        Element::new(Content::Input(InputSpec { value: value.into(), placeholder: String::new(), password: false }));
    e.handlers.input = Some(Rc::new(on_input));
    e.focusable = true;
    e.h(th.control_height)
        .px(10.0)
        .w_full()
        .min_w(40.0)
        .bg(c.input)
        .border(1.0, c.border_strong)
        .rounded(th.radius)
        .cursor(Cursor::Text)
        .transition(th.transition)
        .hover(|s| s.border_color(c.border_strong.lighten(0.1)))
        .focus_style(|s| s.border_color(c.accent).outline(3.0, 0.0, c.accent.with_alpha(0.22)))
}

impl<M: 'static> Element<M> {
    /// Placeholder text for a text input.
    pub fn placeholder(mut self, p: impl Into<String>) -> Self {
        if let Content::Input(spec) = &mut self.content {
            spec.placeholder = p.into();
        }
        self
    }
    /// Mask a text input's contents.
    pub fn password(mut self) -> Self {
        if let Content::Input(spec) = &mut self.content {
            spec.password = true;
        }
        self
    }
    /// Message sent when Enter is pressed in a text input.
    pub fn on_submit(mut self, m: M) -> Self {
        self.handlers.submit = Some(m);
        self
    }
    /// Message sent with the new value of a slider.
    pub fn on_change(mut self, f: impl Fn(f32) -> M + 'static) -> Self {
        self.handlers.value = Some(Rc::new(f));
        self
    }
}

/// A search field with a leading magnifier icon.
pub fn search_input<M: 'static>(value: impl Into<String>, on_input: impl Fn(String) -> M + 'static) -> Element<M> {
    let th = theme();
    div().w_full().items_center().child(text_input(value, on_input).pl(30.0).placeholder("Search")).child(
        icon(Icon::Search).font_size(14.0).color(th.colors.text_faint).absolute().left(10.0).pointer_events(false),
    )
}

/// A checkbox with a label. Attach `.on_click(...)` to toggle.
pub fn checkbox<M: 'static>(label: impl Into<String>, checked: bool) -> Element<M> {
    let th = theme();
    let c = &th.colors;
    let bx = div()
        .center()
        .square(16.0)
        .rounded(4.0)
        .shrink(0.0)
        .transition(th.transition)
        .when(checked, |b| {
            b.bg(c.accent).border(1.0, c.accent).color(c.accent_text).child(icon(Icon::Check).font_size(12.0).bold())
        })
        .when(!checked, |b| b.bg(c.input).border(1.0, c.border_strong));
    row().items_center().gap(8.0).focusable().cursor(Cursor::Pointer).rounded(4.0).child(bx).child(text(label).nowrap())
}

/// An iOS/macOS-style toggle switch. Attach `.on_click(...)` to toggle.
pub fn switch<M: 'static>(on: bool) -> Element<M> {
    let th = theme();
    let c = &th.colors;
    div()
        .w(34.0)
        .h(20.0)
        .pill()
        .shrink(0.0)
        .p(2.0)
        .focusable()
        .cursor(Cursor::Pointer)
        .transition(0.18)
        .bg(if on { c.accent } else { c.border_strong })
        .hover(|s| s.bg(if on { c.accent_hover } else { c.border_strong.lighten(0.08) }))
        .child(
            div()
                .square(16.0)
                .pill()
                .bg(Color::WHITE)
                .shadow(Shadow::new(0.0, 1.0, 3.0, 0.0, Color::BLACK.with_alpha(0.3)))
                .transition(0.18)
                .translate(if on { 14.0 } else { 0.0 }, 0.0),
        )
}

/// A labeled switch row.
pub fn switch_row<M: 'static>(label: impl Into<String>, on: bool) -> Element<M> {
    row().items_center().gap(10.0).cursor(Cursor::Pointer).child(switch(on)).child(text(label).nowrap())
}

/// A horizontal slider. Handle `.on_change(|v| ...)`.
pub fn slider<M: 'static>(value: f32, min: f32, max: f32) -> Element<M> {
    let th = theme();
    let c = &th.colors;
    let t = if max > min { ((value - min) / (max - min)).clamp(0.0, 1.0) } else { 0.0 };
    let knob = 16.0;
    let mut e = div()
        .h(knob)
        .w_full()
        .min_w(60.0)
        .items_center()
        .focusable()
        .cursor(Cursor::Pointer)
        .rounded(knob / 2.0)
        .child(
            div()
                .absolute()
                .left(knob / 2.0)
                .right(knob / 2.0)
                .h(4.0)
                .pill()
                .bg(c.border_strong)
                .child(div().h_full().w(pct(t * 100.0)).pill().bg(c.accent)),
        )
        .child(
            // Knob positioned along the track.
            div().absolute().left(0.0).right(0.0).h(knob).child(
                div().flex_row().w_full().h(knob).child(div().w(pct(t * 100.0)).shrink(1.0)).child(
                    div()
                        .square(knob)
                        .shrink(0.0)
                        .ml(-(t * knob))
                        .pill()
                        .bg(Color::WHITE)
                        .border(1.0, c.border_strong.with_alpha(0.3))
                        .shadow(Shadow::new(0.0, 1.0, 3.0, 0.0, Color::BLACK.with_alpha(0.35))),
                ),
            ),
        );
    e.behavior = Behavior::Slider { value, min, max, step: 0.0 };
    e
}

impl<M: 'static> Element<M> {
    /// Snap a slider's values to multiples of `step`.
    pub fn step(mut self, step: f32) -> Self {
        if let Behavior::Slider { step: s, .. } = &mut self.behavior {
            *s = step;
        }
        self
    }
}

/// A progress bar, `value` in 0..=1.
pub fn progress<M: 'static>(value: f32) -> Element<M> {
    let th = theme();
    div().h(6.0).w_full().pill().bg(th.colors.border).clip().child(
        div()
            .h_full()
            .w(pct(value.clamp(0.0, 1.0) * 100.0))
            .pill()
            .gradient(90.0, [(0.0, th.colors.accent), (1.0, th.colors.accent_hover)]),
    )
}

// -------------------------------------------------------------- decoration

/// A small pill label.
pub fn badge<M: 'static>(label: impl Into<String>) -> Element<M> {
    let th = theme();
    row()
        .items_center()
        .h(18.0)
        .px(7.0)
        .pill()
        .shrink(0.0)
        .bg(th.colors.accent)
        .color(th.colors.accent_text)
        .font_size(11.0)
        .semibold()
        .child(text(label).nowrap())
}

/// A soft, tinted tag.
pub fn tag<M: 'static>(label: impl Into<String>, color: Color) -> Element<M> {
    row()
        .items_center()
        .h(20.0)
        .px(8.0)
        .rounded(5.0)
        .shrink(0.0)
        .bg(color.with_alpha(0.15))
        .color(color)
        .font_size(11.5)
        .medium()
        .child(text(label).nowrap())
}

/// A keyboard shortcut hint, e.g. `kbd("Ctrl+P")`.
pub fn kbd<M: 'static>(keys: impl Into<String>) -> Element<M> {
    let th = theme();
    row()
        .items_center()
        .h(18.0)
        .px(5.0)
        .rounded(4.0)
        .shrink(0.0)
        .bg(th.colors.hover)
        .border(1.0, th.colors.border)
        .color(th.colors.text_muted)
        .font_size(11.0)
        .child(text(keys).nowrap())
}

/// A horizontal divider line.
pub fn separator<M: 'static>() -> Element<M> {
    div().h(1.0).w_full().shrink(0.0).bg(theme().colors.border)
}

/// A vertical divider line.
pub fn vseparator<M: 'static>() -> Element<M> {
    div().w(1.0).self_align(Align::Stretch).shrink(0.0).bg(theme().colors.border)
}

/// A raised card container.
pub fn card<M: 'static>() -> Element<M> {
    let th = theme();
    col()
        .bg(th.colors.elevated)
        .border(1.0, th.colors.border)
        .rounded(th.radius_lg)
        .shadows(th.shadow_sm.clone())
        .p(16.0)
        .gap(10.0)
}

/// A circular avatar with initials.
pub fn avatar<M: 'static>(initials: &str, color: Color) -> Element<M> {
    div()
        .center()
        .square(28.0)
        .pill()
        .shrink(0.0)
        .gradient(135.0, [(0.0, color.lighten(0.15)), (1.0, color.darken(0.15))])
        .color(Color::WHITE)
        .font_size(11.0)
        .semibold()
        .child(text(initials.to_string()).nowrap())
}

/// An uppercase section heading, as used in sidebars.
pub fn section_header<M: 'static>(title: impl Into<String>) -> Element<M> {
    let th = theme();
    row()
        .items_center()
        .h(30.0)
        .px(12.0)
        .shrink(0.0)
        .color(th.colors.text_faint)
        .font_size(11.0)
        .semibold()
        .letter_spacing(0.6)
        .child(text(title.into().to_uppercase()).nowrap())
}

// ------------------------------------------------------------- list / tree

/// A selectable list row with an optional icon.
pub fn list_item<M: 'static>(i: Option<Icon>, label: impl Into<String>, selected: bool) -> Element<M> {
    tree_row(0, None, i, label, selected)
}

/// A tree-view row. `expanded`: `Some(true/false)` shows a disclosure chevron.
pub fn tree_row<M: 'static>(
    depth: usize,
    expanded: Option<bool>,
    i: Option<Icon>,
    label: impl Into<String>,
    selected: bool,
) -> Element<M> {
    let th = theme();
    let c = &th.colors;
    let mut r = row()
        .items_center()
        .h(th.row_height)
        .pl(8.0 + depth as f32 * 14.0)
        .pr(8.0)
        .gap(6.0)
        .mx(6.0)
        .rounded(th.radius_sm)
        .shrink(0.0)
        .color(if selected { c.text } else { c.text_muted })
        .transition(0.08)
        .hover(|s| s.bg(if selected { c.accent_soft } else { c.hover }).color(c.text))
        .cursor(Cursor::Default);
    if selected {
        r = r.bg(c.accent_soft);
    }
    r = match expanded {
        Some(e) => {
            r.child(icon(if e { Icon::ChevronDown } else { Icon::ChevronRight }).font_size(14.0).color(c.text_faint))
        }
        None if depth > 0 => r.child(div().w(14.0).shrink(0.0)),
        None => r,
    };
    if let Some(i) = i {
        r = r.child(icon(i).font_size(15.0));
    }
    r.child(text(label).ellipsis().grow(1.0))
}

// -------------------------------------------------------------------- tabs

/// One tab in a [`tab_bar`].
pub struct Tab<M> {
    pub label: String,
    pub icon: Option<Icon>,
    pub active: bool,
    pub modified: bool,
    pub on_select: M,
    pub on_close: Option<M>,
}

impl<M> Tab<M> {
    pub fn new(label: impl Into<String>, active: bool, on_select: M) -> Self {
        Self { label: label.into(), icon: None, active, modified: false, on_select, on_close: None }
    }
    pub fn icon(mut self, i: Icon) -> Self {
        self.icon = Some(i);
        self
    }
    pub fn closable(mut self, m: M) -> Self {
        self.on_close = Some(m);
        self
    }
    pub fn modified(mut self, m: bool) -> Self {
        self.modified = m;
        self
    }
}

/// Editor-style tabs (VS Code / browser look).
pub fn tab_bar<M: Clone + 'static>(tabs: Vec<Tab<M>>) -> Element<M> {
    let th = theme();
    let c = th.colors.clone();
    let mut bar =
        row().h(th.tab_height).shrink(0.0).bg(c.panel).border_b(1.0, c.border).scroll_x().items(Align::Stretch);
    for (i, t) in tabs.into_iter().enumerate() {
        let mut tab = row()
            .key(("tab", i, t.label.clone()))
            .items_center()
            .gap(7.0)
            .pl(12.0)
            .pr(if t.on_close.is_some() { 6.0 } else { 12.0 })
            .min_w(80.0)
            .shrink(0.0)
            .border_r(1.0, c.border)
            .color(if t.active { c.text } else { c.text_muted })
            .transition(0.08)
            .on_click(t.on_select.clone())
            .cursor(Cursor::Default);
        if t.active {
            tab = tab.bg(c.surface).child(div().absolute().top(0.0).left(0.0).right(0.0).h(2.0).bg(c.accent));
            // Cover the bar's bottom border so the active tab merges into content.
            tab = tab.child(div().absolute().bottom(-1.0).left(0.0).right(0.0).h(1.0).bg(c.surface));
        } else {
            tab = tab.hover(|s| s.bg(c.hover).color(c.text));
        }
        if let Some(i) = &t.icon {
            tab = tab.child(icon(i.clone()).font_size(14.0).color(if t.active { c.accent } else { c.text_faint }));
        }
        tab = tab.child(text(t.label).nowrap());
        if let Some(close) = t.on_close {
            let btn = div()
                .center()
                .square(20.0)
                .rounded(4.0)
                .color(c.text_faint)
                .transition(0.08)
                .hover(|s| s.bg(c.pressed).color(c.text))
                .on_click(close);
            let btn = if t.modified && !t.active {
                btn.child(icon(Icon::Dot).font_size(10.0))
            } else {
                btn.child(icon(Icon::Close).font_size(13.0))
            };
            tab = tab.child(btn);
        }
        bar = bar.child(tab);
    }
    bar
}

/// Segmented control / pill tabs.
pub fn segmented<M: Clone + 'static>(items: Vec<(String, bool, M)>) -> Element<M> {
    let th = theme();
    let c = th.colors.clone();
    let mut r = row()
        .p(3.0)
        .gap(2.0)
        .rounded(th.radius + 2.0)
        .bg(c.input)
        .border(1.0, c.border)
        .shrink(0.0)
        .self_align(Align::Start);
    for (label, active, msg) in items {
        let mut b = row()
            .items_center()
            .h(24.0)
            .px(12.0)
            .rounded(th.radius)
            .font_size(12.0)
            .medium()
            .transition(0.12)
            .on_click(msg)
            .child(text(label).nowrap());
        b = if active {
            b.bg(c.elevated).color(c.text).shadows(th.shadow_sm.clone())
        } else {
            b.color(c.text_muted).hover(|s| s.color(c.text))
        };
        r = r.child(b);
    }
    r
}

// --------------------------------------------------------- window chrome

/// Minimize / maximize / close buttons for frameless windows (Windows style).
pub fn window_controls<M: 'static>(maximized: bool) -> Element<M> {
    let th = theme();
    let c = th.colors.clone();
    let btn = |i: Icon, ctl: WindowControl, danger: bool| {
        div()
            .center()
            .w(46.0)
            .h_full()
            .color(c.text_muted)
            .transition(0.08)
            .hover(
                |s| if danger { s.bg(Color::hex("#e81123")).color(Color::WHITE) } else { s.bg(c.hover).color(c.text) },
            )
            .window_control(ctl)
            .child(icon(i).font_size(14.0).weight(Weight(300)))
    };
    row()
        .h_full()
        .shrink(0.0)
        .child(btn(Icon::Minus, WindowControl::Minimize, false))
        .child(btn(if maximized { Icon::Restore } else { Icon::Maximize }, WindowControl::ToggleMaximize, false))
        .child(btn(Icon::Close, WindowControl::Close, true))
}

/// A custom title bar: `left` content, a centered title, `right` content and
/// window controls. The empty areas drag the window; double-click maximizes.
pub fn titlebar<M: 'static>(
    title: impl Into<String>,
    left: Element<M>,
    right: Element<M>,
    maximized: bool,
) -> Element<M> {
    let th = theme();
    let c = th.colors.clone();
    row()
        .h(th.titlebar_height)
        .shrink(0.0)
        .items_center()
        .bg(c.chrome)
        .border_b(1.0, c.border)
        .child(left.shrink(0.0))
        .child(
            row()
                .grow(1.0)
                .h_full()
                .center()
                .min_w(0.0)
                .window_drag_area()
                .color(c.text_muted)
                .font_size(12.0)
                .child(text(title).ellipsis().pointer_events(false)),
        )
        .child(right.shrink(0.0))
        .child(window_controls(maximized))
}

// ------------------------------------------------------------------- menus

/// An entry in a dropdown or context menu.
pub enum MenuItem<M> {
    Action { label: String, shortcut: Option<String>, icon: Option<Icon>, msg: M, disabled: bool },
    Check { label: String, checked: bool, msg: M },
    Separator,
    Header(String),
}

impl<M> MenuItem<M> {
    pub fn action(label: impl Into<String>, msg: M) -> Self {
        MenuItem::Action { label: label.into(), shortcut: None, icon: None, msg, disabled: false }
    }
    pub fn shortcut(mut self, s: impl Into<String>) -> Self {
        if let MenuItem::Action { shortcut, .. } = &mut self {
            *shortcut = Some(s.into());
        }
        self
    }
    pub fn icon(mut self, i: Icon) -> Self {
        if let MenuItem::Action { icon, .. } = &mut self {
            *icon = Some(i);
        }
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        if let MenuItem::Action { disabled, .. } = &mut self {
            *disabled = d;
        }
        self
    }
    pub fn check(label: impl Into<String>, checked: bool, msg: M) -> Self {
        MenuItem::Check { label: label.into(), checked, msg }
    }
}

/// The popup panel of a menu (without positioning).
pub fn menu_panel<M: Clone + 'static>(items: Vec<MenuItem<M>>) -> Element<M> {
    let th = theme();
    let c = th.colors.clone();
    let mut panel = col()
        .min_w(220.0)
        .p(5.0)
        .bg(c.elevated)
        .border(1.0, c.border_strong)
        .rounded(th.radius + 2.0)
        .shadows(th.shadow_popover.clone());
    let item_row = |label: String, lead: Element<M>, trail: Option<Element<M>>, msg: M, disabled: bool| {
        let mut r = row()
            .items_center()
            .h(th.row_height + 2.0)
            .px(8.0)
            .gap(8.0)
            .rounded(th.radius_sm)
            .color(c.text)
            .transition(0.06)
            .child(lead)
            .child(text(label).nowrap().grow(1.0));
        if let Some(t) = trail {
            r = r.child(t);
        }
        if disabled {
            r.disabled(true).opacity(0.45)
        } else {
            r.hover(|s| s.bg(c.accent).color(c.accent_text)).on_click(msg).cursor(Cursor::Default)
        }
    };
    for it in items {
        panel = match it {
            MenuItem::Action { label, shortcut, icon: ic, msg, disabled } => {
                let lead = match ic {
                    Some(i) => icon(i).font_size(15.0),
                    None => div().w(15.0).shrink(0.0),
                };
                let trail = shortcut.map(|s| text(s).nowrap().font_size(11.5).opacity(0.6).ml(24.0));
                panel.child(item_row(label, lead, trail, msg, disabled))
            }
            MenuItem::Check { label, checked, msg } => {
                let lead = if checked { icon(Icon::Check).font_size(15.0) } else { div().w(15.0).shrink(0.0) };
                panel.child(item_row(label, lead, None, msg, false))
            }
            MenuItem::Separator => panel.child(div().h(1.0).my(4.0).mx(4.0).bg(c.border)),
            MenuItem::Header(h) => panel.child(
                row()
                    .h(24.0)
                    .px(8.0)
                    .items_center()
                    .color(c.text_faint)
                    .font_size(11.0)
                    .semibold()
                    .child(text(h).nowrap()),
            ),
        };
    }
    panel
}

/// A full-window invisible layer that closes popups when clicked.
pub fn backdrop<M: 'static>(on_dismiss: M, dim: bool) -> Element<M> {
    let mut b = div()
        .fixed()
        .top(0.0)
        .left(0.0)
        .right(0.0)
        .bottom(0.0)
        .z_index(90)
        .on_click(on_dismiss)
        .cursor(Cursor::Default);
    if dim {
        b = b.bg(Color::BLACK.with_alpha(0.45));
    }
    b
}

/// A context menu at a window position, with a dismiss backdrop.
pub fn context_menu<M: Clone + 'static>(at: Point, items: Vec<MenuItem<M>>, on_dismiss: M) -> Element<M> {
    div().child(backdrop(on_dismiss, false)).child(menu_panel(items).fixed().left(at.x).top(at.y).z_index(100))
}

/// A top-level menu for a [`menu_bar`].
pub struct Menu<M> {
    pub title: String,
    pub items: Vec<MenuItem<M>>,
}

impl<M> Menu<M> {
    pub fn new(title: impl Into<String>, items: Vec<MenuItem<M>>) -> Self {
        Self { title: title.into(), items }
    }
}

/// An application menu bar (File, Edit, View…). `open` is the index of the
/// open menu, and `on_open` is called with the menu to open (or `None` to close).
pub fn menu_bar<M: Clone + 'static>(
    menus: Vec<Menu<M>>,
    open: Option<usize>,
    on_open: impl Fn(Option<usize>) -> M + 'static,
) -> Element<M> {
    let th = theme();
    let c = th.colors.clone();
    let on_open = Rc::new(on_open);
    let mut bar = row().h_full().items_center().px(4.0).gap(1.0);
    for (i, m) in menus.into_iter().enumerate() {
        let is_open = open == Some(i);
        let o = on_open.clone();
        let mut btn = row()
            .items_center()
            .h(th.row_height)
            .px(9.0)
            .rounded(th.radius_sm)
            .font_size(12.5)
            .color(if is_open { c.text } else { c.text_muted })
            .transition(0.06)
            .hover(|s| s.bg(c.hover).color(c.text))
            .on_click(on_open(if is_open { None } else { Some(i) }))
            .child(text(m.title).nowrap());
        if open.is_some() && !is_open {
            // While a menu is open, hovering another title switches to it.
            btn.handlers.hover = Some(Rc::new(move |_entered: bool| o(Some(i))));
        }
        if is_open {
            btn = btn.bg(c.pressed).child(menu_panel(m.items).absolute().top(28.0).left(0.0).z_index(100));
        }
        bar = bar.child(btn);
    }
    if open.is_some() {
        bar = bar.child(backdrop(on_open(None), false));
    }
    bar
}

/// A centered modal dialog with a dimmed backdrop.
pub fn modal<M: Clone + 'static>(
    title: impl Into<String>,
    body: Element<M>,
    actions: Vec<Element<M>>,
    on_dismiss: M,
) -> Element<M> {
    let th = theme();
    let c = th.colors.clone();
    div().child(backdrop(on_dismiss.clone(), true)).child(
        div().fixed().top(0.0).left(0.0).right(0.0).bottom(0.0).z_index(95).center().pointer_events(false).child(
            col()
                .pointer_events(true)
                .w(440.0)
                .max_w(pct(90.0))
                .bg(c.elevated)
                .border(1.0, c.border_strong)
                .rounded(th.radius_lg + 2.0)
                .shadows(th.shadow_popover.clone())
                .child(
                    row()
                        .items_center()
                        .px(18.0)
                        .pt(16.0)
                        .pb(6.0)
                        .child(text(title).font_size(th.font_size_lg).semibold().grow(1.0))
                        .child(icon_button(Icon::Close).on_click(on_dismiss)),
                )
                .child(col().px(18.0).py(8.0).gap(10.0).color(c.text_muted).child(body))
                .child(row().justify(Justify::End).gap(8.0).px(18.0).pt(10.0).pb(16.0).children(actions)),
        ),
    )
}

// ------------------------------------------------------------- status bar

/// A thin status bar at the bottom of the window.
pub fn status_bar<M: 'static>() -> Element<M> {
    let th = theme();
    row()
        .h(24.0)
        .shrink(0.0)
        .items_center()
        .px(8.0)
        .gap(2.0)
        .bg(th.colors.status_bar)
        .border_t(1.0, th.colors.border)
        .color(th.colors.status_text)
        .font_size(11.5)
}

/// An item in the status bar.
pub fn status_item<M: 'static>(i: Option<Icon>, label: impl Into<String>) -> Element<M> {
    let th = theme();
    let mut r = row()
        .items_center()
        .gap(5.0)
        .h(20.0)
        .px(6.0)
        .rounded(3.0)
        .shrink(0.0)
        .transition(0.08)
        .hover(|s| s.bg(th.colors.hover).color(th.colors.text));
    if let Some(i) = i {
        r = r.child(icon(i).font_size(13.0));
    }
    r.child(text(label).nowrap())
}
