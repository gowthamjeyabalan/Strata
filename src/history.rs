//! Undo and redo: the last changes to the board, kept in memory while the app is open (not saved).
//!
//! Each step is a snapshot of the board. Items that did not change are shared with the step before,
//! so a step costs little more than what changed in it.

use crate::board::{Board, Folder, Id, ImageAsset, Layer, Note, Object, Page, Wire};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Arc;

/// How many changes can be undone, unless Settings says otherwise.
pub const STEPS: usize = 100;

/// The board at one step.
pub struct Snapshot {
    pub objects: Vec<Arc<Object>>,
    pub wires: Vec<Wire>,
    pub images: BTreeMap<Id, ImageAsset>,
    pub next_id: Id,
    pub layers: Vec<Layer>,
    pub notes: Vec<Note>,
    pub folders: Vec<Folder>,
    pub pages: Vec<Page>,
}

impl Snapshot {
    /// Store `b`, sharing every item that is unchanged from `prev`.
    pub fn capture(b: &Board, prev: Option<&Snapshot>) -> Snapshot {
        let old: HashMap<Id, &Arc<Object>> = prev.map(|p| p.objects.iter().map(|o| (o.id, o)).collect()).unwrap_or_default();
        let objects = b.objects.iter().map(|o| match old.get(&o.id) {
            Some(a) if ***a == *o => Arc::clone(a),
            _ => Arc::new(o.clone()),
        });
        Snapshot {
            objects: objects.collect(),
            wires: b.wires.clone(),
            images: b.images.clone(),
            next_id: b.next_id,
            layers: b.layers.clone(),
            notes: b.notes.clone(),
            folders: b.folders.clone(),
            pages: b.pages.clone(),
        }
    }

    /// An editable board from this step.
    pub fn to_board(&self) -> Board {
        Board {
            objects: self.objects.iter().map(|o| (**o).clone()).collect(),
            wires: self.wires.clone(),
            images: self.images.clone(),
            next_id: self.next_id,
            layers: self.layers.clone(),
            notes: self.notes.clone(),
            folders: self.folders.clone(),
            pages: self.pages.clone(),
            veil: Default::default(),
        }
    }

    /// Is `b` the same as this step (nothing to record)?
    fn same(&self, b: &Board) -> bool {
        self.objects.len() == b.objects.len()
            && self.objects.iter().zip(&b.objects).all(|(a, o)| **a == *o)
            && self.wires == b.wires
            && self.layers == b.layers
            && self.notes == b.notes
            && self.folders == b.folders
            && self.pages == b.pages
            && self.images.len() == b.images.len()
    }
}

pub struct Undo {
    current: Snapshot,
    past: VecDeque<Snapshot>,
    future: Vec<Snapshot>,
    /// How many steps back are kept.
    limit: usize,
}

impl Undo {
    pub fn new(board: &Board, limit: usize) -> Self {
        Undo { current: Snapshot::capture(board, None), past: VecDeque::new(), future: Vec::new(), limit: limit.max(1) }
    }

    pub fn set_limit(&mut self, limit: usize) {
        self.limit = limit.max(1);
        while self.past.len() > self.limit {
            self.past.pop_front();
        }
    }

    /// The board as last recorded.
    pub fn current(&self) -> &Snapshot {
        &self.current
    }

    /// Record `board` as a new step if it changed. A new change can no longer be redone over.
    pub fn record(&mut self, board: &Board) -> bool {
        if self.current.same(board) {
            return false;
        }
        let next = Snapshot::capture(board, Some(&self.current));
        self.past.push_back(std::mem::replace(&mut self.current, next));
        if self.past.len() > self.limit {
            self.past.pop_front();
        }
        self.future.clear();
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    pub fn undo(&mut self) -> Option<Board> {
        let prev = self.past.pop_back()?;
        self.future.push(std::mem::replace(&mut self.current, prev));
        Some(self.current.to_board())
    }

    pub fn redo(&mut self) -> Option<Board> {
        let next = self.future.pop()?;
        self.past.push_back(std::mem::replace(&mut self.current, next));
        Some(self.current.to_board())
    }
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
