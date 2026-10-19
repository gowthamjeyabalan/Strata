//! Dates on items: when each was made and every day it was changed, for the Calendar.
//!
//! Stamps are written when a change is recorded as a state, by comparing the board with the state
//! before it. Boards made before items carried dates get theirs from the History.

use crate::board::{Board, Id, Kind, Object};
use crate::history::{History, Snapshot};
use std::collections::{HashMap, HashSet};

/// The day part of a `YYYY-MM-DD HH:MM:SS` time.
pub fn day_of(time: &str) -> &str {
    time.get(..10).unwrap_or(time)
}

/// Did anything about the item change that a person would call working on it? Group frames that
/// only follow their members, and text boxes that only grow with their text, do not count.
fn touched(p: &Object, o: &Object) -> bool {
    if p == o {
        return false;
    }
    let norm = |x: &Object| {
        let mut x = x.clone();
        x.created.clear();
        x.days.clear();
        if x.is_group() {
            x.rect = [0.0; 4];
        } else if matches!(x.kind, Kind::Text) {
            x.rect[3] = 0.0;
        }
        x
    };
    norm(p) != norm(o)
}

fn add_day(o: &mut Object, day: &str) {
    if !o.days.iter().any(|d| d == day) {
        o.days.push(day.to_owned());
        o.days.sort();
    }
}

/// Stamp items that are new or changed since `prev` with `now`.
pub fn stamp(prev: &Snapshot, b: &mut Board, now: &str) {
    let old: HashMap<Id, &Object> = prev.objects.iter().map(|o| (o.id, &**o)).collect();
    let day = day_of(now);
    for o in &mut b.objects {
        match old.get(&o.id) {
            None => {
                if o.created.is_empty() {
                    o.created = now.to_owned();
                }
                add_day(o, day);
            }
            Some(p) if touched(p, o) => add_day(o, day),
            _ => {}
        }
    }
}

/// Give items without dates the ones the History knows: the state they first appear in, and every
/// state that changed them.
pub fn backfill(h: &History, b: &mut Board) {
    if b.objects.iter().all(|o| !o.created.is_empty()) {
        return;
    }
    let mut states: Vec<_> = h.states.iter().collect();
    states.sort_by_key(|s| s.id);
    let maps: HashMap<_, HashMap<Id, &Object>> = states.iter().map(|s| (s.id, s.board.objects.iter().map(|o| (o.id, &**o)).collect())).collect();
    for o in b.objects.iter_mut().filter(|o| o.created.is_empty()) {
        for s in &states {
            let Some(now) = maps[&s.id].get(&o.id) else { continue };
            let before = s.parent.and_then(|p| maps.get(&p)).and_then(|m| m.get(&o.id));
            match before {
                None => {
                    if o.created.is_empty() {
                        o.created = s.time.clone();
                    }
                    add_day(o, day_of(&s.time));
                }
                Some(p) if touched(p, now) => add_day(o, day_of(&s.time)),
                _ => {}
            }
        }
    }
}

/// Was the item made or changed on any of `days`?
pub fn on_days(o: &Object, days: &[String]) -> bool {
    o.days.iter().any(|d| days.contains(d))
}

/// What the Calendar's chosen days do to the board.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum DayMode {
    /// Nothing: the board shows everything as usual.
    #[default]
    All,
    /// Items from the chosen days are marked; the rest are faded.
    Highlight,
    /// Only items from the chosen days are on the board.
    Only,
}

/// Items from `days`, with the groups around them and everything inside groups among them (a group
/// from that day shows with its contents; an item shows inside its group). Also returns the items
/// that were themselves made or changed on those days.
pub fn matching(b: &Board, days: &[String]) -> (HashSet<Id>, HashSet<Id>) {
    let direct: HashSet<Id> = b.objects.iter().filter(|o| on_days(o, days)).map(|o| o.id).collect();
    let mut parent: HashMap<Id, Id> = HashMap::new();
    for o in &b.objects {
        if let Kind::Group { members, .. } = &o.kind {
            for m in members {
                parent.insert(*m, o.id);
            }
        }
    }
    let ids: Vec<Id> = direct.iter().copied().collect();
    let mut all: HashSet<Id> = b.with_descendants(&ids).into_iter().collect();
    for id in ids {
        let mut cur = id;
        while let Some(p) = parent.get(&cur) {
            all.insert(*p);
            cur = *p;
        }
    }
    (all, direct)
}
