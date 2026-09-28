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
#[non_exhaustive]
pub enum Role {
    /// ARIA `button`.
    Button,
    /// ARIA `checkbox`.
    CheckBox,
    /// ARIA `switch`.
    Switch,
    /// ARIA `radio`.
    RadioButton,
    /// ARIA `radiogroup`.
    RadioGroup,
    /// ARIA `tab`.
    Tab,
    /// ARIA `tablist`.
    TabList,
    /// ARIA `tabpanel`.
    TabPanel,
    /// ARIA `menu`.
    Menu,
    /// ARIA `menubar`.
    MenuBar,
    /// ARIA `menuitem`.
    MenuItem,
    /// ARIA `link`.
    Link,
    /// ARIA `heading` (see [`Semantics::level`]).
    Heading,
    /// ARIA `list`.
    List,
    /// ARIA `listitem`.
    ListItem,
    /// ARIA `tree`.
    Tree,
    /// ARIA `treeitem`.
    TreeItem,
    /// ARIA `table`.
    Table,
    /// ARIA `row`.
    Row,
    /// ARIA `cell`.
    Cell,
    /// ARIA `columnheader`.
    ColumnHeader,
    /// ARIA `dialog`.
    Dialog,
    /// ARIA `progressbar`.
    ProgressBar,
    /// ARIA `img`.
    Image,
    /// ARIA `toolbar`.
    Toolbar,
    /// ARIA `status`.
    Status,
    /// ARIA `navigation`.
    Navigation,
    /// ARIA `group`.
    Group,
    /// ARIA `separator`.
    Separator,
    /// Static text; no direct ARIA role (like a `<label>`).
    Label,
    /// ARIA `combobox`.
    ComboBox,
    /// ARIA `slider`.
    Slider,
    /// ARIA `textbox`.
    TextInput,
    /// A scrollable region; no direct ARIA role (like an overflowing `<div>`).
    ScrollView,
}

/// Accessibility information attached to an element.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Semantics {
    /// What the element is. `None` lets the runtime infer it.
    pub role: Option<Role>,
    /// Accessible name. Defaults to the text inside the element (or its tooltip).
    pub label: Option<String>,
    /// Longer accessible description (ARIA `aria-description`).
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
    /// Link target, for [`Role::Link`].
    pub url: Option<String>,
    /// Whether this is a modal dialog (ARIA `aria-modal`).
    pub modal: bool,
    /// Hide this element and its subtree from assistive technology.
    pub hidden: bool,
}
