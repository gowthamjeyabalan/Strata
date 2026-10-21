//! Dates on items: when each was made and every day it was changed, for the Calendar.
//!
//! Stamps are written when a change is recorded as a state, by comparing the board with the state
//! before it.

use crate::board::{Board, Id, Kind, Object};
use crate::history::Snapshot;
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
    // Notes whose text changed: their cards count as worked on.
    let edited: HashSet<Id> = b.notes.iter().filter(|n| prev.notes.iter().find(|p| p.id == n.id).is_none_or(|p| p.text != n.text)).map(|n| n.id).collect();
    for n in b.notes.iter_mut().filter(|n| n.created.is_empty()) {
        n.created = now.to_owned();
    }
    for o in &mut b.objects {
        if let Kind::Note { note } = o.kind {
            if edited.contains(&note) {
                add_day(o, day);
            }
        }
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

/// Was the item made or changed on any of `days`?
pub fn on_days(o: &Object, days: &[String]) -> bool {
    o.days.iter().any(|d| days.contains(d))
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
