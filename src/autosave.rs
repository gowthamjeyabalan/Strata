//! Saving without being asked: autosave every few minutes (to the board's file, or to a recovery copy
//! that is offered back after a crash), numbered increments (plan-001, plan-002, …), and backups of
//! the previous file kept on each save.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Where autosave writes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub enum Autosave {
    Off,
    /// The board's own file (a board never saved goes to a recovery copy).
    #[default]
    File,
    /// Only a recovery copy; the board's file changes when you save.
    Recovery,
}

/// What a recovery copy belongs to.
#[derive(Serialize, Deserialize)]
pub struct RecoveryInfo {
    /// The board's file, if it had one.
    pub original: Option<PathBuf>,
    /// When the copy was written, `YYYY-MM-DD HH:MM`.
    pub when: String,
}

/// A recovery copy found when Strata starts.
#[derive(Clone)]
pub struct Recovered {
    pub file: PathBuf,
    pub original: Option<PathBuf>,
    pub when: String,
}

/// The folder recovery copies go in.
pub fn recovery_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("Strata").join("recovery")
}

/// The recovery copy for a board: named after its file, or after when this run started when it has
/// none yet.
pub fn recovery_path(original: Option<&Path>, session: &str) -> PathBuf {
    let name = match original {
        Some(p) => {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            p.hash(&mut h);
            let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            format!("{stem}-{:08x}", h.finish() as u32)
        }
        None => format!("untitled-{session}"),
    };
    recovery_dir().join(format!("{name}.strata"))
}

/// Write `json` as the recovery copy at `file`, with a note of what it belongs to.
pub fn write_recovery(file: &Path, json: &str, original: Option<&Path>) -> Result<(), String> {
    std::fs::create_dir_all(recovery_dir()).map_err(|e| e.to_string())?;
    let tmp = file.with_extension("strata.tmp");
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, file).map_err(|e| e.to_string())?;
    let info = RecoveryInfo { original: original.map(Path::to_path_buf), when: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string() };
    std::fs::write(file.with_extension("json"), serde_json::to_string(&info).unwrap_or_default()).map_err(|e| e.to_string())
}

/// Remove a recovery copy and its note.
pub fn remove_recovery(file: &Path) {
    let _ = std::fs::remove_file(file);
    let _ = std::fs::remove_file(file.with_extension("json"));
}

/// Recovery copies left by a run that did not end cleanly, newest first. A copy older than its
/// board's file (saved since) is out of date and removed.
pub fn find_recovered() -> Vec<Recovered> {
    let Ok(dir) = std::fs::read_dir(recovery_dir()) else { return Vec::new() };
    let mut out = Vec::new();
    for e in dir.flatten() {
        let file = e.path();
        if !matches!(file.extension().and_then(|x| x.to_str()), Some("strata" | "overlay")) {
            continue;
        }
        let info: Option<RecoveryInfo> = std::fs::read_to_string(file.with_extension("json")).ok().and_then(|t| serde_json::from_str(&t).ok());
        let Some(info) = info else {
            remove_recovery(&file);
            continue;
        };
        let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        if let (Some(orig), Some(copy)) = (info.original.as_deref().and_then(modified), modified(&file)) {
            if orig >= copy {
                remove_recovery(&file);
                continue;
            }
        }
        out.push(Recovered { file, original: info.original, when: info.when });
    }
    out.sort_by(|a, b| b.when.cmp(&a.when));
    out
}

/// The next numbered file after `path`: plan.strata → plan-001.strata, plan-004.strata →
/// plan-005.strata, skipping numbers already taken in the folder.
pub fn next_increment(path: &Path) -> PathBuf {
    let dir = path.parent().unwrap_or(Path::new("."));
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Untitled".into());
    // A number already at the end (after -, _ or a space) is where counting continues.
    let (base, mut n) = match stem.rfind(['-', '_', ' ']) {
        Some(k) if k + 1 < stem.len() && stem[k + 1..].chars().all(|c| c.is_ascii_digit()) => (stem[..k].to_owned(), stem[k + 1..].parse::<u32>().unwrap_or(0)),
        _ => (stem.clone(), 0),
    };
    loop {
        n += 1;
        let p = dir.join(format!("{base}-{n:03}.strata"));
        if !p.exists() {
            return p;
        }
    }
}

/// Before `path` is written over, keep a copy of it in "<name> backups" beside it, and keep only the
/// newest `keep` copies there.
pub fn back_up(path: &Path, keep: u32) {
    if keep == 0 || !path.exists() {
        return;
    }
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let dir = path.parent().unwrap_or(Path::new(".")).join(format!("{stem} backups"));
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let stamp = chrono::Local::now().format("%Y-%m-%d %H%M%S");
    let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("strata");
    let _ = std::fs::copy(path, dir.join(format!("{stem} {stamp}.{ext}")));
    let mut copies: Vec<PathBuf> = std::fs::read_dir(&dir).map(|d| d.flatten().map(|e| e.path()).filter(|p| matches!(p.extension().and_then(|x| x.to_str()), Some("strata" | "overlay"))).collect()).unwrap_or_default();
    // The names sort by time.
    copies.sort();
    while copies.len() > keep as usize {
        let _ = std::fs::remove_file(copies.remove(0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn increments_count_on() {
        let dir = std::env::temp_dir();
        assert_eq!(next_increment(&dir.join("plan.strata")).file_name().unwrap(), "plan-001.strata");
        assert_eq!(next_increment(&dir.join("plan-004.strata")).file_name().unwrap(), "plan-005.strata");
        assert_eq!(next_increment(&dir.join("plan_v2_09.overlay")).file_name().unwrap(), "plan_v2-010.strata");
    }
}
