//! States and branches of the whole board.
//!
//! Every change becomes a state, recorded automatically. States form a tree:
//! - Edits that keep every item (adding, moving, formatting, grouping, collapsing, comments) continue
//!   the current branch.
//! - Deleting an item or a connection starts a new branch from the state before it, so the old graph
//!   stays intact on its own branch.
//! - Changing something after stepping back to an older state also starts a branch, so nothing that
//!   came after it is lost.
//!
//! Undo and redo move along the tree; the History panel jumps to any state.

use crate::board::{Board, Folder, Id, Kind, Layer, Note, Object, Wire};
use crate::board::ImageAsset;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

pub type StateId = u32;
pub type BranchId = u32;

#[derive(Clone)]
pub struct State {
    pub id: StateId,
    pub parent: Option<StateId>,
    pub branch: BranchId,
    /// Local time, `YYYY-MM-DD HH:MM:SS`.
    pub time: String,
    /// What changed, in a few words.
    pub label: String,
    pub board: Snapshot,
}

/// A board as stored in the history. Items that did not change from the state before are shared
/// with it rather than copied, so a long history costs little more than the items that changed.
#[derive(Clone, Default)]
pub struct Snapshot {
    pub objects: Vec<Arc<Object>>,
    pub wires: Vec<Wire>,
    pub images: BTreeMap<Id, ImageAsset>,
    pub next_id: Id,
    pub layers: Vec<Layer>,
    pub notes: Vec<Note>,
    pub folders: Vec<Folder>,
}

impl Snapshot {
    /// Store `b`, sharing every item that is unchanged from `prev`.
    pub fn capture(b: &Board, prev: Option<&Snapshot>) -> Snapshot {
        let old: HashMap<Id, &Arc<Object>> = prev.map(|p| p.objects.iter().map(|o| (o.id, o)).collect()).unwrap_or_default();
        let objects = b.objects.iter().map(|o| match old.get(&o.id) {
            Some(a) if ***a == *o => Arc::clone(a),
            _ => Arc::new(o.clone()),
        });
        Snapshot { objects: objects.collect(), wires: b.wires.clone(), images: b.images.clone(), next_id: b.next_id, layers: b.layers.clone(), notes: b.notes.clone(), folders: b.folders.clone() }
    }

    /// An editable board from this state.
    pub fn to_board(&self) -> Board {
        Board { objects: self.objects.iter().map(|o| (**o).clone()).collect(), wires: self.wires.clone(), images: self.images.clone(), next_id: self.next_id, layers: self.layers.clone(), notes: self.notes.clone(), folders: self.folders.clone(), veil: Default::default() }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn get(&self, id: Id) -> Option<&Object> {
        self.objects.iter().find(|o| o.id == id).map(|o| &**o)
    }
}

/// Read access to the items and connections of a board or a stored state.
pub trait View {
    fn objs(&self) -> Vec<&Object>;
    fn wire_list(&self) -> &[Wire];
    fn layer_list(&self) -> &[Layer];
    fn note_list(&self) -> &[Note];
    fn folder_list(&self) -> &[Folder];
}

impl View for Board {
    fn objs(&self) -> Vec<&Object> {
        self.objects.iter().collect()
    }
    fn wire_list(&self) -> &[Wire] {
        &self.wires
    }
    fn layer_list(&self) -> &[Layer] {
        &self.layers
    }
    fn note_list(&self) -> &[Note] {
        &self.notes
    }
    fn folder_list(&self) -> &[Folder] {
        &self.folders
    }
}

impl View for Snapshot {
    fn objs(&self) -> Vec<&Object> {
        self.objects.iter().map(|o| &**o).collect()
    }
    fn wire_list(&self) -> &[Wire] {
        &self.wires
    }
    fn layer_list(&self) -> &[Layer] {
        &self.layers
    }
    fn note_list(&self) -> &[Note] {
        &self.notes
    }
    fn folder_list(&self) -> &[Folder] {
        &self.folders
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Branch {
    pub id: BranchId,
    pub name: String,
    /// The state this branch grew from (none for the first branch).
    pub from: Option<StateId>,
}

pub struct History {
    pub states: Vec<State>,
    pub branches: Vec<Branch>,
    pub current: StateId,
    /// For redo: the child last moved away from, per parent.
    redo_to: HashMap<StateId, StateId>,
}

pub fn now() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// "14:02" for today, "12 Oct 14:02" otherwise.
pub fn short_time(t: &str) -> String {
    match chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%d %H:%M:%S") {
        Ok(dt) if dt.date() == chrono::Local::now().date_naive() => dt.format("%H:%M").to_string(),
        Ok(dt) => dt.format("%d %b %H:%M").to_string(),
        Err(_) => t.to_owned(),
    }
}

/// What happened between two boards.
pub struct Change {
    pub label: String,
    /// An item or a connection was deleted.
    pub deleted: bool,
}

fn name_of(o: &Object) -> String {
    if o.label.trim().is_empty() { o.kind.name().to_lowercase() } else { format!("\"{}\"", o.label.lines().next().unwrap_or_default().chars().take(24).collect::<String>()) }
}

fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Describe the change from `a` to `b`, or `None` if nothing a person would notice changed.
pub fn describe(a: &impl View, b: &impl View) -> Option<Change> {
    let (ao, bo) = (a.objs(), b.objs());
    let (aw, bw) = (a.wire_list(), b.wire_list());
    let a_obj: BTreeMap<Id, &Object> = ao.iter().map(|o| (o.id, *o)).collect();
    let b_obj: BTreeMap<Id, &Object> = bo.iter().map(|o| (o.id, *o)).collect();
    let a_wire: BTreeMap<Id, &Wire> = aw.iter().map(|w| (w.id, w)).collect();
    let b_wire: BTreeMap<Id, &Wire> = bw.iter().map(|w| (w.id, w)).collect();

    // Removing a group alone is ungrouping; its members are still there.
    let deleted: Vec<&Object> = ao.iter().copied().filter(|o| !b_obj.contains_key(&o.id) && !o.is_group()).collect();
    let ungrouped = ao.iter().filter(|o| !b_obj.contains_key(&o.id) && o.is_group()).count();
    let wires_deleted = aw.iter().filter(|w| !b_wire.contains_key(&w.id)).count();
    if !deleted.is_empty() {
        let mut label = if deleted.len() == 1 { format!("Deleted {}", name_of(deleted[0])) } else { format!("Deleted {}", count(deleted.len(), "item", "items")) };
        let own_wires = aw.iter().filter(|w| !b_wire.contains_key(&w.id) && (b_obj.contains_key(&w.from) && b_obj.contains_key(&w.to))).count();
        if own_wires > 0 {
            label.push_str(&format!(" and {}", count(own_wires, "connection", "connections")));
        }
        return Some(Change { label, deleted: true });
    }
    if wires_deleted > 0 {
        return Some(Change { label: format!("Deleted {}", count(wires_deleted, "connection", "connections")), deleted: true });
    }
    if let Some(n) = a.note_list().iter().find(|n| !b.note_list().iter().any(|x| x.id == n.id)) {
        return Some(Change { label: format!("Deleted note \"{}\"", n.title().chars().take(24).collect::<String>()), deleted: true });
    }

    let added: Vec<&Object> = bo.iter().copied().filter(|o| !a_obj.contains_key(&o.id)).collect();
    let added_groups = added.iter().filter(|o| o.is_group()).count();
    let added_items: Vec<&&Object> = added.iter().filter(|o| !o.is_group()).collect();
    let wires_added: Vec<&Wire> = bw.iter().filter(|w| !a_wire.contains_key(&w.id)).collect();
    let label = if !added_items.is_empty() {
        if added_items.len() == 1 { format!("Added {}", added_items[0].kind.name().to_lowercase()) } else { format!("Added {}", count(added_items.len(), "item", "items")) }
    } else if added_groups > 0 {
        "Grouped".to_owned()
    } else if ungrouped > 0 {
        "Ungrouped".to_owned()
    } else if wires_added.len() == 1 {
        let w = wires_added[0];
        match (b_obj.get(&w.from), b_obj.get(&w.to)) {
            (Some(f), Some(t)) => format!("Connected {} to {}", name_of(f), name_of(t)),
            _ => "Added a connection".to_owned(),
        }
    } else if !wires_added.is_empty() {
        format!("Added {}", count(wires_added.len(), "connection", "connections"))
    } else if let Some(l) = describe_layers(a.layer_list(), b.layer_list()) {
        l
    } else if let Some(l) = describe_notes(a.note_list(), b.note_list()) {
        l
    } else if let Some(l) = describe_folders(a.folder_list(), b.folder_list()) {
        l
    } else if let Some(l) = describe_edit(&ao, &bo, aw, bw, &a_obj) {
        l
    } else {
        return None;
    };
    Some(Change { label, deleted: false })
}

/// Layers added, removed, renamed or reordered. Getting a first layer (older boards) is not a change.
fn describe_layers(a: &[Layer], b: &[Layer]) -> Option<String> {
    if a == b || a.is_empty() {
        return None;
    }
    let named = |l: &Layer| format!("layer \"{}\"", l.name);
    if let Some(l) = b.iter().find(|l| !a.iter().any(|x| x.id == l.id)) {
        return Some(format!("Added {}", named(l)));
    }
    if let Some(l) = a.iter().find(|l| !b.iter().any(|x| x.id == l.id)) {
        return Some(format!("Deleted {}", named(l)));
    }
    if let Some(l) = b.iter().find(|l| a.iter().any(|x| x.id == l.id && x.name != l.name)) {
        return Some(format!("Renamed {}", named(l)));
    }
    Some("Reordered layers".to_owned())
}

/// Notes written, edited or deleted in the Notes panel.
fn describe_notes(a: &[Note], b: &[Note]) -> Option<String> {
    if a == b {
        return None;
    }
    let named = |n: &Note| format!("note \"{}\"", n.title().chars().take(24).collect::<String>());
    if let Some(n) = b.iter().find(|n| !a.iter().any(|x| x.id == n.id)) {
        return Some(format!("Wrote {}", named(n)));
    }
    if let Some(n) = a.iter().find(|n| !b.iter().any(|x| x.id == n.id)) {
        return Some(format!("Deleted {}", named(n)));
    }
    if let Some(n) = b.iter().find(|n| a.iter().any(|x| x.id == n.id && x.text != n.text)) {
        return Some(format!("Edited {}", named(n)));
    }
    b.iter().find(|n| a.iter().any(|x| x.id == n.id && x.folder != n.folder)).map(|n| format!("Moved {}", named(n)))
}

/// Note folders made, renamed or deleted.
fn describe_folders(a: &[Folder], b: &[Folder]) -> Option<String> {
    if a == b {
        return None;
    }
    if let Some(f) = b.iter().find(|f| !a.iter().any(|x| x.id == f.id)) {
        return Some(format!("Added folder \"{}\"", f.name));
    }
    if let Some(f) = a.iter().find(|f| !b.iter().any(|x| x.id == f.id)) {
        return Some(format!("Deleted folder \"{}\"", f.name));
    }
    b.iter().find(|f| a.iter().any(|x| x.id == f.id && x.name != f.name)).map(|f| format!("Renamed folder \"{}\"", f.name))
}

/// Same items and connections, something about them changed.
fn describe_edit(ao: &[&Object], bo: &[&Object], aw: &[Wire], bw: &[Wire], a_obj: &BTreeMap<Id, &Object>) -> Option<String> {
    let changed: Vec<(&Object, &Object)> = bo.iter().filter_map(|o| a_obj.get(&o.id).map(|p| (*p, *o))).filter(|(p, o)| !std::ptr::eq(*p, *o) && p != o).collect();
    let first = |f: &dyn Fn(&Object, &Object) -> bool| changed.iter().find(|(p, o)| f(p, o)).copied();

    if let Some((_, o)) = first(&|p, o| p.comments != o.comments) {
        return Some(format!("Comment on {}", name_of(o)));
    }
    if let Some((_, o)) = first(&|p, o| matches!((&p.kind, &o.kind), (Kind::Group { collapsed: x, .. }, Kind::Group { collapsed: y, .. }) if x != y)) {
        let collapsed = matches!(o.kind, Kind::Group { collapsed: true, .. });
        return Some(format!("{} {}", if collapsed { "Collapsed" } else { "Expanded" }, name_of(o)));
    }
    if let Some((_, o)) = first(&|p, o| p.locked != o.locked) {
        return Some(format!("{} {}", if o.locked { "Locked" } else { "Unlocked" }, name_of(o)));
    }
    if let Some((p, o)) = first(&|p, o| p.label != o.label) {
        return Some(if p.label.is_empty() { format!("Named {}", name_of(o)) } else { format!("Edited {}", name_of(o)) });
    }
    if let Some((_, o)) = first(&|p, o| p.layer != o.layer && p.layer != 0) {
        return Some(format!("Moved {} to another layer", name_of(o)));
    }
    if first(&|p, o| (p.opacity - o.opacity).abs() > 1e-4).is_some() {
        return Some("Changed opacity".to_owned());
    }
    if first(&|p, o| p.style != o.style).is_some() {
        return Some("Changed formatting".to_owned());
    }
    if first(&|p, o| p.kind != o.kind && !p.is_group()).is_some() {
        return Some("Changed shape".to_owned());
    }
    // Group frames and text heights follow their contents, so only other items count as moved.
    let moved: Vec<&(&Object, &Object)> = changed.iter().filter(|(p, o)| p.rect != o.rect && !o.is_group() && !matches!(o.kind, Kind::Text) || (matches!(o.kind, Kind::Text) && (p.rect[0], p.rect[1], p.rect[2]) != (o.rect[0], o.rect[1], o.rect[2]))).collect();
    if !moved.is_empty() {
        let resized = moved.iter().any(|(p, o)| (p.rect[2], p.rect[3]) != (o.rect[2], o.rect[3]) && !matches!(o.kind, Kind::Text));
        return Some(if resized && moved.len() == 1 { format!("Resized {}", name_of(moved[0].1)) } else { format!("Moved {}", count(moved.len(), "item", "items")) });
    }
    let a_order: Vec<Id> = ao.iter().map(|o| o.id).collect();
    let b_order: Vec<Id> = bo.iter().map(|o| o.id).collect();
    if a_order != b_order {
        return Some("Changed order".to_owned());
    }
    if aw != bw {
        return Some("Edited a connection".to_owned());
    }
    None
}

impl History {
    pub fn new(board: &Board, label: &str) -> Self {
        Self {
            states: vec![State { id: 0, parent: None, branch: 0, time: now(), label: label.to_owned(), board: Snapshot::capture(board, None) }],
            branches: vec![Branch { id: 0, name: "Main".to_owned(), from: None }],
            current: 0,
            redo_to: HashMap::new(),
        }
    }

    pub fn state(&self, id: StateId) -> Option<&State> {
        self.states.iter().find(|s| s.id == id)
    }

    pub fn current(&self) -> &State {
        self.state(self.current).expect("current state exists")
    }

    pub fn branch(&self, id: BranchId) -> Option<&Branch> {
        self.branches.iter().find(|b| b.id == id)
    }

    pub fn branch_mut(&mut self, id: BranchId) -> Option<&mut Branch> {
        self.branches.iter_mut().find(|b| b.id == id)
    }

    pub fn children(&self, id: StateId) -> impl Iterator<Item = &State> {
        self.states.iter().filter(move |s| s.parent == Some(id))
    }

    /// The newest state on a branch.
    pub fn head(&self, branch: BranchId) -> Option<StateId> {
        self.states.iter().filter(|s| s.branch == branch).map(|s| s.id).max()
    }

    /// Record `board` as a new state if it differs from the current one. Returns the change and
    /// whether it started a new branch.
    pub fn record(&mut self, board: &Board) -> Option<(Change, bool)> {
        let change = describe(&self.current().board, board)?;
        let cur = self.current().clone_meta();
        let has_children = self.children(cur.0).next().is_some();
        let new_branch = change.deleted || has_children;
        let branch = if new_branch {
            let id = self.branches.iter().map(|b| b.id).max().unwrap_or(0) + 1;
            let name = format!("Branch {}", id + 1);
            self.branches.push(Branch { id, name, from: Some(cur.0) });
            id
        } else {
            cur.1
        };
        let id = self.states.iter().map(|s| s.id).max().unwrap_or(0) + 1;
        let snap = Snapshot::capture(board, Some(&self.current().board));
        self.states.push(State { id, parent: Some(cur.0), branch, time: now(), label: change.label.clone(), board: snap });
        self.current = id;
        Some((change, new_branch))
    }

    pub fn can_undo(&self) -> bool {
        self.current().parent.is_some()
    }

    pub fn can_redo(&self) -> bool {
        self.redo_target().is_some()
    }

    fn redo_target(&self) -> Option<StateId> {
        let cur = self.current;
        self.redo_to.get(&cur).copied().filter(|c| self.state(*c).is_some()).or_else(|| {
            // Prefer a child on the same branch, then the newest.
            let b = self.current().branch;
            self.children(cur).find(|s| s.branch == b).or_else(|| self.children(cur).max_by_key(|s| s.id)).map(|s| s.id)
        })
    }

    pub fn undo(&mut self) -> Option<Board> {
        let parent = self.current().parent?;
        self.redo_to.insert(parent, self.current);
        self.current = parent;
        Some(self.current().board.to_board())
    }

    pub fn redo(&mut self) -> Option<Board> {
        self.current = self.redo_target()?;
        Some(self.current().board.to_board())
    }

    pub fn jump(&mut self, id: StateId) -> Option<Board> {
        self.state(id)?;
        self.current = id;
        Some(self.current().board.to_board())
    }

    /// After states were removed, re-describe a reconnected state against its new parent.
    fn relabel(&mut self, id: StateId) {
        let Some(s) = self.state(id) else { return };
        let Some(parent) = s.parent.and_then(|p| self.state(p)) else { return };
        let label = describe(&parent.board, &s.board).map(|c| c.label).unwrap_or_else(|| "No change".to_owned());
        if let Some(s) = self.states.iter_mut().find(|x| x.id == id) {
            s.label = label;
        }
    }

    /// Delete one state. What came after it is joined to what came before it, and branches that grew
    /// from it grow from the state before it instead. Later states keep their full content: this removes
    /// a step from the history, it does not undo that step's change in later states.
    /// If the current state is deleted, the state before it becomes current.
    pub fn delete_state(&mut self, id: StateId) -> Result<(), &'static str> {
        let s = self.state(id).ok_or("That state no longer exists")?;
        let parent = s.parent.ok_or("The first state cannot be deleted")?;
        let branch = s.branch;
        let children: Vec<StateId> = self.children(id).map(|c| c.id).collect();
        for st in &mut self.states {
            if st.parent == Some(id) {
                st.parent = Some(parent);
            }
        }
        for b in &mut self.branches {
            if b.from == Some(id) {
                b.from = Some(parent);
            }
        }
        self.states.retain(|x| x.id != id);
        // A branch with nothing left on it goes too (never the first branch).
        if branch != 0 && !self.states.iter().any(|x| x.branch == branch) {
            self.branches.retain(|b| b.id != branch);
        }
        if self.current == id {
            self.current = parent;
        }
        self.redo_to.retain(|k, v| *k != id && *v != id);
        for c in children {
            self.relabel(c);
        }
        Ok(())
    }

    /// Delete a branch and every state on it. Branches that grew out of it are kept and reattached to
    /// the state it grew from. If you are on the deleted branch, you go to that state.
    /// The first branch cannot be deleted.
    pub fn delete_branch(&mut self, branch: BranchId) -> Result<(), &'static str> {
        if branch == 0 {
            return Err("The first branch cannot be deleted");
        }
        let from = self.branch(branch).ok_or("That branch no longer exists")?.from.ok_or("The first branch cannot be deleted")?;
        let doomed: Vec<StateId> = self.states.iter().filter(|s| s.branch == branch).map(|s| s.id).collect();
        let mut reattached = Vec::new();
        for st in &mut self.states {
            if st.branch != branch && st.parent.is_some_and(|p| doomed.contains(&p)) {
                st.parent = Some(from);
                reattached.push(st.id);
            }
        }
        for b in &mut self.branches {
            if b.id != branch && b.from.is_some_and(|f| doomed.contains(&f)) {
                b.from = Some(from);
            }
        }
        self.states.retain(|s| s.branch != branch);
        self.branches.retain(|b| b.id != branch);
        if doomed.contains(&self.current) {
            self.current = from;
        }
        self.redo_to.retain(|k, v| !doomed.contains(k) && !doomed.contains(v));
        for id in reattached {
            self.relabel(id);
        }
        Ok(())
    }

    /// Keep only the given states and branches, and the states needed to reach them from the start;
    /// delete everything else. Returns how many states were removed.
    /// If you are on something that goes, you move to the newest kept state you selected.
    pub fn keep_only(&mut self, states: &[StateId], branches: &[BranchId]) -> Result<usize, &'static str> {
        let mut keep: std::collections::HashSet<StateId> = std::collections::HashSet::new();
        let path_to = |id: StateId, keep: &mut std::collections::HashSet<StateId>| {
            let mut cur = Some(id);
            while let Some(c) = cur {
                if !keep.insert(c) {
                    break;
                }
                cur = self.state(c).and_then(|s| s.parent);
            }
        };
        let mut chosen: Vec<StateId> = Vec::new();
        for b in branches {
            for s in self.states.iter().filter(|s| s.branch == *b) {
                chosen.push(s.id);
            }
        }
        chosen.extend(states.iter().copied().filter(|id| self.state(*id).is_some()));
        if chosen.is_empty() {
            return Err("Select at least one state or branch");
        }
        for id in &chosen {
            path_to(*id, &mut keep);
        }
        let before = self.states.len();
        self.states.retain(|s| keep.contains(&s.id));
        let kept_branches: Vec<BranchId> = self.states.iter().map(|s| s.branch).collect();
        self.branches.retain(|b| kept_branches.contains(&b.id));
        if !keep.contains(&self.current) {
            self.current = chosen.iter().copied().max().unwrap_or(0);
        }
        self.redo_to.retain(|k, v| keep.contains(k) && keep.contains(v));
        Ok(before - self.states.len())
    }

    // ── files ──

    /// History for saving: each distinct version of an object is stored once and states refer to it,
    /// and images are left out (the file keeps every image once, next to the board).
    pub fn to_file(&self) -> HistoryFile {
        let mut objects: Vec<Object> = Vec::new();
        // Shared items are found by pointer (cheap); different copies with the same content by value.
        let mut by_ptr: HashMap<*const Object, u32> = HashMap::new();
        let mut by_value: HashMap<String, u32> = HashMap::new();
        let mut states = Vec::new();
        for s in &self.states {
            let refs = s
                .board
                .objects
                .iter()
                .map(|o| {
                    let ptr = Arc::as_ptr(o);
                    if let Some(i) = by_ptr.get(&ptr) {
                        return *i;
                    }
                    let key = serde_json::to_string(&**o).unwrap_or_default();
                    let i = *by_value.entry(key).or_insert_with(|| {
                        objects.push((**o).clone());
                        (objects.len() - 1) as u32
                    });
                    by_ptr.insert(ptr, i);
                    i
                })
                .collect();
            states.push(StateFile {
                id: s.id,
                parent: s.parent,
                branch: s.branch,
                time: s.time.clone(),
                label: s.label.clone(),
                objects: refs,
                wires: s.board.wires.clone(),
                next_id: s.board.next_id,
                layers: s.board.layers.clone(),
                notes: s.board.notes.clone(),
                folders: s.board.folders.clone(),
            });
        }
        HistoryFile { objects, states, branches: self.branches.clone(), current: self.current }
    }

    /// Every image any state uses.
    pub fn all_images(&self) -> BTreeMap<Id, crate::board::ImageAsset> {
        let mut out = BTreeMap::new();
        for s in &self.states {
            for (k, v) in &s.board.images {
                out.entry(*k).or_insert_with(|| v.clone());
            }
        }
        out
    }

    pub fn from_file(f: HistoryFile, images: &BTreeMap<Id, crate::board::ImageAsset>) -> Option<Self> {
        // Each stored item becomes one shared copy that every state using it points to.
        let shared: Vec<Arc<Object>> = f.objects.into_iter().map(Arc::new).collect();
        let mut states = Vec::new();
        for s in f.states {
            let objects: Vec<Arc<Object>> = s.objects.iter().filter_map(|i| shared.get(*i as usize).cloned()).collect();
            let imgs = objects
                .iter()
                .filter_map(|o| if let Kind::Image { image } = o.kind { images.get(&image).map(|a| (image, a.clone())) } else { None })
                .collect();
            let board = Snapshot { objects, wires: s.wires, images: imgs, next_id: s.next_id, layers: s.layers, notes: s.notes, folders: s.folders };
            states.push(State { id: s.id, parent: s.parent, branch: s.branch, time: s.time, label: s.label, board });
        }
        let h = History { states, branches: f.branches, current: f.current, redo_to: HashMap::new() };
        (h.state(h.current).is_some() && !h.branches.is_empty()).then_some(h)
    }
}

impl State {
    fn clone_meta(&self) -> (StateId, BranchId) {
        (self.id, self.branch)
    }
}

#[derive(Serialize, Deserialize)]
pub struct StateFile {
    pub id: StateId,
    pub parent: Option<StateId>,
    pub branch: BranchId,
    pub time: String,
    pub label: String,
    /// Indexes into `HistoryFile::objects`.
    pub objects: Vec<u32>,
    pub wires: Vec<Wire>,
    pub next_id: Id,
    #[serde(default)]
    pub layers: Vec<Layer>,
    #[serde(default)]
    pub notes: Vec<Note>,
    #[serde(default)]
    pub folders: Vec<Folder>,
}

#[derive(Serialize, Deserialize)]
pub struct HistoryFile {
    pub objects: Vec<Object>,
    pub states: Vec<StateFile>,
    pub branches: Vec<Branch>,
    pub current: StateId,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{Side, Style, WireStyle};
    use egui::{Rect, pos2, vec2};

    fn add(b: &mut Board) -> Id {
        b.add(Rect::from_min_size(pos2(0.0, 0.0), vec2(10.0, 10.0)), Kind::Text, Style::default())
    }

    #[test]
    fn formatting_continues_and_deleting_branches() {
        let mut board = Board::default();
        let mut h = History::new(&board, "New board");
        let a = add(&mut board);
        let (c, branched) = h.record(&board).unwrap();
        assert!(!branched && c.label.starts_with("Added"));
        board.get_mut(a).unwrap().style.ink = crate::board::Ink::Rgb([1, 2, 3]);
        let (c, branched) = h.record(&board).unwrap();
        assert!(!branched && c.label == "Changed formatting");
        assert!(h.record(&board).is_none(), "no change, no state");
        let before_delete = h.current;
        board.remove(&[a]);
        let (c, branched) = h.record(&board).unwrap();
        assert!(branched && c.deleted);
        assert_eq!(h.branches.len(), 2);
        assert_eq!(h.branch(h.current().branch).unwrap().from, Some(before_delete));
        // The old branch still has the item.
        assert!(h.state(before_delete).unwrap().board.get(a).is_some());
    }

    #[test]
    fn undo_redo_and_change_after_undo_branches() {
        let mut board = Board::default();
        let mut h = History::new(&board, "New board");
        add(&mut board);
        h.record(&board);
        let b = add(&mut board);
        h.record(&board);
        let back = h.undo().unwrap().clone();
        assert!(back.get(b).is_none());
        assert!(h.redo().unwrap().get(b).is_some());
        h.undo();
        let mut board = h.current().board.to_board();
        board.objects[0].label = "x".into();
        let (_, branched) = h.record(&board).unwrap();
        assert!(branched, "changing an older state keeps what came after it");
    }

    #[test]
    fn ungroup_and_connections() {
        let mut board = Board::default();
        let a = add(&mut board);
        let c = add(&mut board);
        let mut h = History::new(&board, "start");
        let g = board.group(&[a, c], Style::default()).unwrap();
        assert_eq!(h.record(&board).unwrap().0.label, "Grouped");
        board.ungroup(g);
        let (ch, branched) = h.record(&board).unwrap();
        assert!(ch.label == "Ungrouped" && !branched);
        let w = board.connect(a, Side::Right, c, Side::Left, WireStyle::Curve).unwrap();
        assert!(!h.record(&board).unwrap().1);
        board.wires.retain(|x| x.id != w);
        assert!(h.record(&board).unwrap().1, "deleting a connection branches");
    }

    /// Main: 0 - 1 - 2 - 3, and a branch from 2 holding 4 - 5, and a branch from 4 holding 6.
    fn tree() -> History {
        let mut board = Board::default();
        let mut h = History::new(&board, "start");
        let a = add(&mut board);
        h.record(&board); // 1
        let b = add(&mut board);
        h.record(&board); // 2
        add(&mut board);
        h.record(&board); // 3 (main)
        h.jump(2);
        let mut fork = h.current().board.to_board();
        fork.get_mut(a).unwrap().label = "x".into();
        h.record(&fork); // 4, new branch (2 had a child)
        fork.get_mut(b).unwrap().label = "y".into();
        h.record(&fork); // 5
        h.jump(4);
        let mut sub = h.current().board.to_board();
        sub.get_mut(a).unwrap().label = "z".into();
        h.record(&sub); // 6, another branch
        h
    }

    #[test]
    fn deleting_a_state_joins_before_and_after() {
        let mut h = tree();
        assert_eq!(h.branches.len(), 3);
        let b1 = h.state(4).unwrap().branch;
        // Delete 4: 5 now follows 2, and the branch that grew from 4 grows from 2.
        h.delete_state(4).unwrap();
        assert!(h.state(4).is_none());
        assert_eq!(h.state(5).unwrap().parent, Some(2));
        assert_eq!(h.state(6).unwrap().parent, Some(2));
        assert!(h.branches.iter().filter(|b| b.id != b1).any(|b| b.from == Some(2) && b.id != 0));
        // The current state (6) is untouched; deleting it moves you back.
        assert_eq!(h.current, 6);
        h.delete_state(6).unwrap();
        assert_eq!(h.current, 2);
        // The emptied branch is gone; the first state can't go.
        assert_eq!(h.branches.len(), 2);
        assert!(h.delete_state(0).is_err());
    }

    #[test]
    fn deleting_a_branch_removes_its_states_and_keeps_offshoots() {
        let mut h = tree();
        let b1 = h.state(4).unwrap().branch;
        let b2 = h.state(6).unwrap().branch;
        h.delete_branch(b1).unwrap();
        assert!(h.state(4).is_none() && h.state(5).is_none());
        // The branch that grew out of it is kept, now growing from where b1 grew from (state 2).
        assert_eq!(h.state(6).unwrap().parent, Some(2));
        assert_eq!(h.branch(b2).unwrap().from, Some(2));
        // You were on b2, so you stay put; deleting b2 takes you to state 2.
        assert_eq!(h.current, 6);
        h.delete_branch(b2).unwrap();
        assert_eq!(h.current, 2);
        assert!(h.delete_branch(0).is_err());
    }

    #[test]
    fn keeping_only_a_branch_keeps_its_path_from_the_start() {
        let mut h = tree();
        let b1 = h.state(4).unwrap().branch;
        // Keep only b1 (4, 5): its path back is 2, 1, 0. Main's 3 and the other branch (6) go.
        let removed = h.keep_only(&[], &[b1]).unwrap();
        assert_eq!(removed, 2);
        let mut ids: Vec<u32> = h.states.iter().map(|s| s.id).collect();
        ids.sort();
        assert_eq!(ids, vec![0, 1, 2, 4, 5]);
        assert_eq!(h.branches.len(), 2, "Main (for the path) and b1");
        // You were on 6, which went: you are now on the newest kept state you chose.
        assert_eq!(h.current, 5);
    }

    #[test]
    fn keeping_only_a_state_keeps_the_path_to_it() {
        let mut h = tree();
        h.keep_only(&[3], &[]).unwrap();
        let mut ids: Vec<u32> = h.states.iter().map(|s| s.id).collect();
        ids.sort();
        assert_eq!(ids, vec![0, 1, 2, 3]);
        assert_eq!(h.branches.len(), 1);
        assert!(h.keep_only(&[], &[]).is_err());
    }

    #[test]
    fn file_round_trip_shares_objects() {
        let mut board = Board::default();
        let mut h = History::new(&board, "start");
        let a = add(&mut board);
        h.record(&board);
        board.get_mut(a).unwrap().label = "hello".into();
        h.record(&board);
        board.wires.clear();
        add(&mut board);
        h.record(&board);
        let f = h.to_file();
        // The unchanged first item is stored once even though three states contain it.
        assert_eq!(f.objects.len(), 3);
        let json = serde_json::to_string(&f).unwrap();
        let back = History::from_file(serde_json::from_str(&json).unwrap(), &BTreeMap::new()).unwrap();
        assert_eq!(back.states.len(), 4);
        assert_eq!(back.current().board.objects.len(), 2);
    }
}
