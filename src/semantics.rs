//! Accessibility semantics: what an element *is* to assistive technology
//! (screen readers, voice control). Most semantics are inferred — text,
//! inputs, sliders, scroll areas, dropdowns — and the built-in widgets set
//! the rest. Custom widgets use [`Element::role`], [`Element::aria_label`]
//! and the `aria_*` state methods, mirroring ARIA on the web.
//!
//! [`Element::role`]: crate::Element::role
//! [`Element::aria_label`]: crate::Element::aria_label

/// The kind of control or structure an element represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    Button,
    CheckBox,
    Switch,
    RadioButton,
    RadioGroup,
    Tab,
    TabList,
    TabPanel,
    Menu,
    MenuBar,
    MenuItem,
    Link,
    Heading,
    List,
    ListItem,
    Tree,
    TreeItem,
    Table,
    Row,
    Cell,
    ColumnHeader,
    Dialog,
    ProgressBar,
    Image,
    Toolbar,
    Status,
    Navigation,
    Group,
    Separator,
    Label,
    ComboBox,
    Slider,
    TextInput,
    ScrollView,
}

/// Accessibility information attached to an element.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Semantics {
    pub role: Option<Role>,
    /// Accessible name. Defaults to the text inside the element (or its tooltip).
    pub label: Option<String>,
    pub description: Option<String>,
    /// Checked / on state (checkboxes, switches, radio buttons).
    pub checked: Option<bool>,
    /// Selected state (tabs, list and tree items, rows).
    pub selected: Option<bool>,
    /// Expanded state (tree items, dropdowns, disclosure buttons).
    pub expanded: Option<bool>,
    /// Heading level 1–6.
    pub level: Option<u8>,
    /// Numeric value, e.g. progress (with `min`/`max`).
    pub value: Option<(f64, f64, f64)>,
    pub url: Option<String>,
    pub modal: bool,
    /// Hide this element and its subtree from assistive technology.
    pub hidden: bool,
}
