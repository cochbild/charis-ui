//! Accessibility tree (AccessKit) and assistive-technology actions.
#![cfg(feature = "accessibility")]

use accesskit::{Action, ActionData, ActionRequest, Node, NodeId, Role as Ak, Toggled, TreeId, TreeUpdate};
use rust_ui::prelude::*;

#[derive(Default)]
struct Form {
    name: String,
    remember: bool,
    volume: f32,
    saved: u32,
    dialog: bool,
}

#[derive(Clone, Debug)]
enum Msg {
    Name(String),
    Remember,
    Volume(f32),
    Save,
    Close,
}

impl App for Form {
    type Msg = Msg;
    fn update(&mut self, msg: Msg, _: &mut Cx<Msg>) {
        match msg {
            Msg::Name(s) => self.name = s,
            Msg::Remember => self.remember = !self.remember,
            Msg::Volume(v) => self.volume = v,
            Msg::Save => self.saved += 1,
            Msg::Close => self.dialog = false,
        }
    }
    fn view(&self) -> Element<Msg> {
        let mut root = col()
            .p(20.0)
            .gap(10.0)
            .child(text("Account settings").heading(1))
            .child(text("Changes apply after restart."))
            .child(text_input(self.name.clone(), Msg::Name).id("name").placeholder("Your name"))
            .child(checkbox("Remember me", self.remember).on_click(Msg::Remember))
            .child(slider(self.volume, 0.0, 10.0).step(1.0).on_change(Msg::Volume))
            .child(primary_button("Save").with_icon(Icon::Check).on_click(Msg::Save))
            .child(tooltip_icon_button(Icon::Settings, "Preferences").on_click(Msg::Save))
            .child(progress(0.4))
            .child(segmented(vec![("Dark".into(), true, Msg::Save), ("Light".into(), false, Msg::Save)]));
        if self.dialog {
            root = root.child(modal("Discard changes?", text("This can't be undone."), vec![], Msg::Close));
        }
        root
    }
}

fn tree(h: &mut Headless<Form>) -> TreeUpdate {
    h.rt.accessibility_tree()
}

fn find<'a>(t: &'a TreeUpdate, role: Ak, name: &str) -> (NodeId, &'a Node) {
    t.nodes
        .iter()
        .find(|(_, n)| n.role() == role && (n.label() == Some(name) || n.value() == Some(name)))
        .map(|(id, n)| (*id, n))
        .unwrap_or_else(|| {
            let all: Vec<String> =
                t.nodes.iter().map(|(_, n)| format!("{:?} {:?} {:?}", n.role(), n.label(), n.value())).collect();
            panic!("no {role:?} named {name:?} in:\n{}", all.join("\n"))
        })
}

fn act(h: &mut Headless<Form>, target: NodeId, action: Action, data: Option<ActionData>) {
    h.rt.accessibility_action(ActionRequest { action, target_tree: TreeId::ROOT, target_node: target, data });
    h.settle();
}

#[test]
fn widgets_expose_roles_names_and_states() {
    let mut h = Headless::new(Form { volume: 3.0, ..Default::default() }, 500.0, 500.0, 1.0);
    h.settle();
    let t = tree(&mut h);
    let root = t.tree.as_ref().unwrap().root;
    assert_eq!(t.nodes.iter().find(|(id, _)| *id == root).unwrap().1.role(), Ak::Window);

    let (_, heading) = find(&t, Ak::Heading, "Account settings");
    assert_eq!(heading.level(), Some(1));
    find(&t, Ak::Label, "Changes apply after restart.");

    // A button is named by its text, which isn't repeated as a separate node.
    let (_, save) = find(&t, Ak::Button, "Save");
    assert!(save.supports_action(Action::Click) && save.supports_action(Action::Focus));
    assert!(save.children().is_empty(), "text folded into the name, icon is decoration");
    assert!(!t.nodes.iter().any(|(_, n)| n.role() == Ak::Label && n.value() == Some("Save")));
    // Icon-only buttons are named by their tooltip.
    find(&t, Ak::Button, "Preferences");

    let (_, cb) = find(&t, Ak::CheckBox, "Remember me");
    assert_eq!(cb.toggled(), Some(Toggled::False));

    let input = &t.nodes.iter().find(|(_, n)| n.role() == Ak::TextInput).unwrap().1;
    assert_eq!(input.placeholder(), Some("Your name"));

    let slider = &t.nodes.iter().find(|(_, n)| n.role() == Ak::Slider).unwrap().1;
    assert_eq!(
        (slider.numeric_value(), slider.min_numeric_value(), slider.max_numeric_value()),
        (Some(3.0), Some(0.0), Some(10.0))
    );

    let bar = &t.nodes.iter().find(|(_, n)| n.role() == Ak::ProgressIndicator).unwrap().1;
    assert_eq!(bar.numeric_value(), Some(40.0));

    let (_, dark) = find(&t, Ak::RadioButton, "Dark");
    assert_eq!(dark.toggled(), Some(Toggled::True));
    assert_eq!(find(&t, Ak::RadioButton, "Light").1.toggled(), Some(Toggled::False));
    assert!(t.nodes.iter().any(|(_, n)| n.role() == Ak::RadioGroup));

    // Bounds are in physical pixels.
    let b = save.bounds().unwrap();
    assert!(b.x1 > b.x0 && b.y1 > b.y0);
}

#[test]
fn screen_reader_actions_drive_the_app() {
    let mut h = Headless::new(Form::default(), 500.0, 500.0, 1.0);
    h.settle();
    let t = tree(&mut h);
    let (save, _) = find(&t, Ak::Button, "Save");
    act(&mut h, save, Action::Click, None);
    assert_eq!(h.rt.app.saved, 1);

    let (cb, _) = find(&t, Ak::CheckBox, "Remember me");
    act(&mut h, cb, Action::Click, None);
    assert!(h.rt.app.remember);
    let t = tree(&mut h);
    assert_eq!(find(&t, Ak::CheckBox, "Remember me").1.toggled(), Some(Toggled::True));

    let input = t.nodes.iter().find(|(_, n)| n.role() == Ak::TextInput).unwrap().0;
    act(&mut h, input, Action::Focus, None);
    act(&mut h, input, Action::SetValue, Some(ActionData::Value("Ada".into())));
    assert_eq!(h.rt.app.name, "Ada");
    let t = tree(&mut h);
    assert_eq!(t.focus, input, "focus is reported to the screen reader");
    assert_eq!(t.nodes.iter().find(|(id, _)| *id == input).unwrap().1.value(), Some("Ada"));

    let slider = t.nodes.iter().find(|(_, n)| n.role() == Ak::Slider).unwrap().0;
    act(&mut h, slider, Action::Increment, None);
    act(&mut h, slider, Action::Increment, None);
    assert_eq!(h.rt.app.volume, 2.0);
    act(&mut h, slider, Action::SetValue, Some(ActionData::Value("7".into())));
    assert_eq!(h.rt.app.volume, 7.0);
}

#[test]
fn modal_dialogs_are_marked_modal_and_named() {
    let mut h = Headless::new(Form { dialog: true, ..Default::default() }, 500.0, 500.0, 1.0);
    h.settle();
    let t = tree(&mut h);
    let (_, d) = find(&t, Ak::Dialog, "Discard changes?");
    assert!(d.is_modal());
    let (close, _) = find(&t, Ak::Button, "Close");
    act(&mut h, close, Action::Click, None);
    assert!(!h.rt.app.dialog);
}

#[test]
fn keyboard_reaches_and_activates_built_in_controls() {
    use rust_ui::Event;
    let mut h = Headless::new(Form::default(), 500.0, 500.0, 1.0);
    h.settle();
    let tab = |h: &mut Headless<Form>| {
        h.event(Event::Key(KeyEvent { key: Key::Tab, mods: Modifiers::default(), repeat: false }));
    };
    // Tab until the "Light" segment is focused, then press Space.
    let mut reached = false;
    for _ in 0..20 {
        tab(&mut h);
        let t = h.rt.accessibility_tree();
        let focused = t.nodes.iter().find(|(id, _)| *id == t.focus).map(|(_, n)| n.clone());
        if focused.is_some_and(|n| n.role() == Ak::RadioButton && n.label() == Some("Light")) {
            reached = true;
            break;
        }
    }
    assert!(reached, "Tab reaches the segmented control's options");
    let saved = h.rt.app.saved;
    h.event(Event::Key(KeyEvent { key: Key::Space, mods: Modifiers::default(), repeat: false }));
    assert_eq!(h.rt.app.saved, saved + 1, "Space activates the focused option");
}
