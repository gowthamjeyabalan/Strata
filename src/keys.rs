//! Keyboard shortcuts that can be changed in Settings: every tool and most commands. A few keys
//! (Esc, Delete, Enter, the arrows, copy and paste, Space to pan) stay as they are.

use crate::tools::Tool;
use egui::{Key, Modifiers};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Action {
    Tool(Tool),
    NewBoard,
    Open,
    Save,
    SaveAs,
    SaveIncrement,
    Export,
    Search,
    Settings,
    Undo,
    Redo,
    SelectAll,
    Duplicate,
    Lock,
    Group,
    Ungroup,
    InsertImage,
    AddItem,
    BringFront,
    SendBack,
    ZoomFit,
    ZoomSelection,
    ActualSize,
    Overview,
    Present,
}

impl Action {
    pub fn name(self) -> String {
        match self {
            Action::Tool(t) => t.name().to_owned(),
            Action::NewBoard => "New board".into(),
            Action::Open => "Open".into(),
            Action::Save => "Save".into(),
            Action::SaveAs => "Save as".into(),
            Action::SaveIncrement => "Save increment (the next numbered copy)".into(),
            Action::Export => "Export image".into(),
            Action::Search => "Command menu (search everything)".into(),
            Action::Settings => "Settings".into(),
            Action::Undo => "Undo".into(),
            Action::Redo => "Redo".into(),
            Action::SelectAll => "Select all".into(),
            Action::Duplicate => "Duplicate".into(),
            Action::Lock => "Lock or unlock".into(),
            Action::Group => "Group".into(),
            Action::Ungroup => "Ungroup".into(),
            Action::InsertImage => "Insert image".into(),
            Action::AddItem => "Add an item (the / list)".into(),
            Action::BringFront => "Bring to front".into(),
            Action::SendBack => "Send to back".into(),
            Action::ZoomFit => "Zoom to fit".into(),
            Action::ZoomSelection => "Zoom to the selection".into(),
            Action::ActualSize => "Actual size".into(),
            Action::Overview => "Show or hide the Overview".into(),
            Action::Present => "Present frames".into(),
        }
    }

    /// Works while typing in a text box (file commands with Ctrl, and search).
    pub fn while_typing(self) -> bool {
        matches!(self, Action::NewBoard | Action::Open | Action::Save | Action::SaveAs | Action::SaveIncrement | Action::Export | Action::Search | Action::Settings)
    }
}

/// A key with the modifiers held for it.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Combo {
    /// egui's name for the key, such as "S" or "F5".
    pub key: String,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub alt: bool,
}

impl Combo {
    pub fn new(key: Key, ctrl: bool, shift: bool) -> Self {
        Combo { key: key.name().to_owned(), ctrl, shift, alt: false }
    }
    /// The key pressed with `m`, as a combo.
    pub fn from_press(key: Key, m: Modifiers) -> Self {
        Combo { key: key.name().to_owned(), ctrl: m.command || m.ctrl, shift: m.shift, alt: m.alt }
    }
    pub fn label(&self) -> String {
        let key = Key::from_name(&self.key).map(|k| k.symbol_or_name().to_owned()).unwrap_or_else(|| self.key.clone());
        let mut s = String::new();
        if self.ctrl {
            s.push_str("Ctrl+");
        }
        if self.alt {
            s.push_str("Alt+");
        }
        if self.shift {
            s.push_str("Shift+");
        }
        s.push_str(&key);
        s
    }
}

/// Every action that has a shortcut, in the order Settings lists them, by section.
pub fn sections() -> Vec<(&'static str, Vec<Action>)> {
    vec![
        ("Command menu", vec![Action::Search]),
        ("Tools", Tool::ALL.into_iter().map(Action::Tool).collect()),
        ("File", vec![Action::NewBoard, Action::Open, Action::Save, Action::SaveAs, Action::SaveIncrement, Action::Export, Action::Settings]),
        ("Edit", vec![Action::Undo, Action::Redo, Action::SelectAll, Action::Duplicate, Action::Lock, Action::Group, Action::Ungroup, Action::BringFront, Action::SendBack]),
        ("Add", vec![Action::AddItem, Action::InsertImage]),
        ("View", vec![Action::ZoomFit, Action::ZoomSelection, Action::ActualSize, Action::Overview, Action::Present]),
    ]
}

/// The shortcut each action starts with.
pub fn default_combo(a: Action) -> Option<Combo> {
    use Key::*;
    let c = |k| Some(Combo::new(k, true, false));
    let cs = |k| Some(Combo::new(k, true, true));
    let plain = |k| Some(Combo::new(k, false, false));
    match a {
        Action::Tool(t) => plain(t.key()),
        Action::NewBoard => c(N),
        Action::Open => c(O),
        Action::Save => c(S),
        Action::SaveAs => cs(S),
        Action::SaveIncrement => Some(Combo { alt: true, ..Combo::new(S, true, false) }),
        Action::Export => c(E),
        Action::Search => c(K),
        Action::Settings => c(Comma),
        Action::Undo => c(Z),
        Action::Redo => c(Y),
        Action::SelectAll => c(A),
        Action::Duplicate => c(D),
        Action::Lock => c(L),
        Action::Group => c(G),
        Action::Ungroup => cs(G),
        Action::InsertImage => c(I),
        Action::AddItem => plain(Slash),
        Action::BringFront => c(CloseBracket),
        Action::SendBack => c(OpenBracket),
        Action::ZoomFit => Some(Combo::new(Num1, false, true)),
        Action::ZoomSelection => Some(Combo::new(Num2, false, true)),
        Action::ActualSize => c(Num0),
        Action::Overview => plain(M),
        Action::Present => plain(F5),
    }
}

/// The shortcuts in use: the defaults, with whatever was changed in Settings.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Keymap {
    /// Only the changed ones; `None` means the action has no shortcut.
    changed: Vec<(Action, Option<Combo>)>,
}

impl Keymap {
    pub fn get(&self, a: Action) -> Option<Combo> {
        match self.changed.iter().find(|(x, _)| *x == a) {
            Some((_, c)) => c.clone(),
            None => default_combo(a),
        }
    }

    pub fn set(&mut self, a: Action, c: Option<Combo>) {
        self.changed.retain(|(x, _)| *x != a);
        if c != default_combo(a) {
            self.changed.push((a, c));
        }
    }

    pub fn is_default(&self, a: Action) -> bool {
        !self.changed.iter().any(|(x, _)| *x == a)
    }

    pub fn reset_all(&mut self) {
        self.changed.clear();
    }

    /// The other action that already uses `c`, if any.
    pub fn user_of(&self, c: &Combo, except: Action) -> Option<Action> {
        sections().into_iter().flat_map(|(_, v)| v).find(|a| *a != except && self.get(*a).as_ref() == Some(c))
    }

    /// The action for a key pressed with `m`.
    pub fn action_for(&self, key: Key, m: Modifiers) -> Option<Action> {
        let c = Combo::from_press(key, m);
        sections().into_iter().flat_map(|(_, v)| v).find(|a| self.get(*a).as_ref() == Some(&c))
    }

    /// "Pen (P)" style label for tooltips and menus.
    pub fn label(&self, a: Action) -> String {
        match self.get(a) {
            Some(c) => format!("{} ({})", a.name(), c.label()),
            None => a.name(),
        }
    }
}
