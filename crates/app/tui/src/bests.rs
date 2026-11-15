//! Merging shorter tours into `data/rust/best`: from experiment arms, the
//! old `data/rust-*` run folders, or Python's bests. Every merge first copies
//! the files it replaces into `data/rust/backups/<stamp>/`, with a manifest,
//! so it can be undone.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mow_report::bests::read_bests;
use mow_report::family::pessimistic_score;
use serde::{Deserialize, Serialize};

use crate::clock::{now, stamp};
use crate::data::{Lawn, Paths};

const MANIFEST: &str = "manifest.json";
const UNDONE: &str = "undone";
const SUFFIX: &str = ".solution.json";

/// A folder of tours to merge from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    /// Its path below the repository root.
    pub name: String,
    pub dir: PathBuf,
}

/// Every folder named `best` that could hold tours to merge: experiment
/// arms, the `data/rust-*` folders and their arms, and Python's bests.
pub fn sources(paths: &Paths) -> Vec<Source> {
    let data = paths.root.join("data");
    let mut found: Vec<PathBuf> = Vec::new();

    for top in subfolders(&data) {
        let name = top
            .file_name()
            .map_or(String::new(), |n| n.to_string_lossy().into_owned());
        if name.starts_with("rust-") {
            found.extend(best_dirs_within(&top));
        }
    }
    for experiment in subfolders(&paths.data.join("experiments")) {
        found.extend(best_dirs_within(&experiment));
    }
    found.push(data.join("solutions").join("best"));

    found
        .into_iter()
        .filter(|dir| dir.is_dir() && *dir != paths.best_dir())
        .map(|dir| Source {
            name: dir
                .strip_prefix(&paths.root)
                .unwrap_or(&dir)
                .to_string_lossy()
                .replace('\\', "/"),
            dir,
        })
        .collect()
}

/// `<dir>/best` and `<dir>/<arm>/best`.
fn best_dirs_within(dir: &Path) -> Vec<PathBuf> {
    let mut found = vec![dir.join("best")];
    found.extend(subfolders(dir).into_iter().map(|arm| arm.join("best")));

    found.into_iter().filter(|d| d.is_dir()).collect()
}

fn subfolders(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut found: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    found.sort();
    found
}

/// One tour a merge would take.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Change {
    pub uid: String,
    /// The current best's length, if there is one.
    pub before: Option<u64>,
    pub after: u64,
    #[serde(skip)]
    pub from: PathBuf,
}

/// What merging a source would change.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub source: Source,
    pub changes: Vec<Change>,
    /// Points the score gains.
    pub gain: f64,
}

/// The tours of `source` shorter than the stored bests, for instances the
/// lawn knows.
pub fn plan(source: &Source, lawn: &Lawn) -> Result<Plan, String> {
    let theirs = read_bests(&source.dir)?;
    let mut changes = Vec::new();
    let mut gain = 0.0;

    for inst in &lawn.instances {
        let Some(their) = theirs.get(&inst.uid) else {
            continue;
        };
        let before = inst.best.as_ref().map(|b| b.objective);
        if before.is_some_and(|b| b <= their.objective) {
            continue;
        }

        let points = |objective: Option<u64>| {
            objective
                .and_then(|o| pessimistic_score(inst.lower_bound, o))
                .unwrap_or(0.0)
        };
        gain += points(Some(their.objective)) - points(before);
        changes.push(Change {
            uid: inst.uid.clone(),
            before,
            after: their.objective,
            from: their.path.clone(),
        });
    }

    Ok(Plan {
        source: source.clone(),
        changes,
        gain,
    })
}

/// A merge as its backup folder records it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub source: String,
    pub made: u64,
    pub gain: f64,
    pub changes: Vec<Change>,
}

/// A backup folder and what it holds.
#[derive(Debug, Clone, PartialEq)]
pub struct Backup {
    pub dir: PathBuf,
    pub manifest: Manifest,
    pub undone: bool,
}

/// Merges the plan: backs up each best it replaces, then copies the shorter
/// tour in. Returns the backup folder.
pub fn merge(paths: &Paths, plan: &Plan) -> Result<PathBuf, String> {
    let backup = backups_dir(paths).join(format!("{}-merge", stamp(now())));
    std::fs::create_dir_all(&backup).map_err(|e| format!("cannot create {}: {e}", backup.display()))?;

    let manifest = Manifest {
        source: plan.source.name.clone(),
        made: now(),
        gain: plan.gain,
        changes: plan.changes.clone(),
    };
    write_manifest(&backup, &manifest)?;

    for change in &plan.changes {
        let target = best_file(paths, &change.uid);
        if target.is_file() {
            copy(&target, &backup.join(file_name(&change.uid)))?;
        }
        copy(&change.from, &target)?;
    }

    Ok(backup)
}

/// Puts back what a merge replaced, for each instance whose best is still
/// the merged tour. Returns how many were restored and how many skipped
/// because a later tour replaced them.
pub fn undo(paths: &Paths, backup: &Backup) -> Result<(usize, usize), String> {
    if backup.undone {
        return Err("this merge is undone already".to_owned());
    }
    let current = read_bests(&paths.best_dir())?;
    let (mut restored, mut skipped) = (0, 0);

    for change in &backup.manifest.changes {
        if current.get(&change.uid).map(|b| b.objective) != Some(change.after) {
            skipped += 1;
            continue;
        }

        let target = best_file(paths, &change.uid);
        match change.before {
            Some(_) => copy(&backup.dir.join(file_name(&change.uid)), &target)?,
            None => std::fs::remove_file(&target)
                .map_err(|e| format!("cannot remove {}: {e}", target.display()))?,
        }
        restored += 1;
    }

    std::fs::write(backup.dir.join(UNDONE), format!("undone at {}\n", now())).map_err(|e| e.to_string())?;
    Ok((restored, skipped))
}

/// Every merge backup, newest first.
pub fn backups(paths: &Paths) -> Vec<Backup> {
    let mut found: Vec<Backup> = subfolders(&backups_dir(paths))
        .into_iter()
        .filter_map(|dir| {
            let text = std::fs::read_to_string(dir.join(MANIFEST)).ok()?;
            let manifest = serde_json::from_str(&text).ok()?;
            Some(Backup {
                undone: dir.join(UNDONE).is_file(),
                dir,
                manifest,
            })
        })
        .collect();
    found.sort_by(|a, b| b.manifest.made.cmp(&a.manifest.made).then(b.dir.cmp(&a.dir)));

    found
}

/// The merged changes by uid, for showing a backup.
pub fn by_uid(backup: &Backup) -> BTreeMap<&str, &Change> {
    backup
        .manifest
        .changes
        .iter()
        .map(|c| (c.uid.as_str(), c))
        .collect()
}

fn backups_dir(paths: &Paths) -> PathBuf {
    paths.data.join("backups")
}

fn best_file(paths: &Paths, uid: &str) -> PathBuf {
    paths.best_dir().join(file_name(uid))
}

fn file_name(uid: &str) -> String {
    format!("{uid}{SUFFIX}")
}

fn write_manifest(dir: &Path, manifest: &Manifest) -> Result<(), String> {
    let json = serde_json::to_string_pretty(manifest).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(MANIFEST), json).map_err(|e| format!("cannot write the manifest: {e}"))
}

fn copy(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    std::fs::copy(from, to)
        .map(drop)
        .map_err(|e| format!("cannot copy {} to {}: {e}", from.display(), to.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::paths_under;
    use crate::runs::tests::scratch;

    /// A one-cutter solution file of a square tour `side` long a side.
    fn solution(uid: &str, side: i32) -> String {
        format!(
            "{{\"instance_uid\":\"{uid}\",\"tours\":[{{\"x\":[0,{side},{side},0],\"y\":[0,0,{side},{side}]}}],\"meta\":{{}}}}"
        )
    }

    fn put(dir: &Path, uid: &str, side: i32) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(file_name(uid)), solution(uid, side)).unwrap();
    }

    fn catalog(paths: &Paths) {
        let header = "source,instance,cells,width,height,aspect,fill,holes,k,cutter_cells,cutter_width,cutter_height,cutter_kind,step_gain,lower_bound\n";
        let row = |uid: &str| format!("instances,{uid},100,10,10,1,1,0,1,9,3,3,square,3,20\n");
        std::fs::write(
            paths.data.join("catalog.csv"),
            format!("{header}{}{}{}", row("a_1s3"), row("b_1s3"), row("c_1s3")),
        )
        .unwrap();
    }

    #[test]
    fn a_merge_takes_only_shorter_tours_and_undoes() {
        let root = scratch("bests");
        let paths = paths_under(&root, "instances");
        put(&paths.best_dir(), "a_1s3", 10); // length 40
        put(&paths.best_dir(), "b_1s3", 10);
        catalog(&paths);

        let arm = paths
            .data
            .join("experiments")
            .join("e1")
            .join("fast")
            .join("best");
        put(&arm, "a_1s3", 8); // shorter
        put(&arm, "b_1s3", 12); // longer
        put(&arm, "c_1s3", 9); // no best yet

        let found = sources(&paths);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].name, "data/rust/experiments/e1/fast/best");

        let lawn = Lawn::load(&paths).unwrap();
        let plan = plan(&found[0], &lawn).unwrap();
        let uids: Vec<&str> = plan.changes.iter().map(|c| c.uid.as_str()).collect();
        assert_eq!(uids, ["a_1s3", "c_1s3"]);
        assert!(plan.gain > 0.0);

        merge(&paths, &plan).unwrap();
        let after = Lawn::load(&paths).unwrap();
        assert_eq!(after.find("a_1s3").unwrap().best.as_ref().unwrap().objective, 32);
        assert!(after.find("c_1s3").unwrap().best.is_some());

        let backup = backups(&paths).remove(0);
        assert_eq!(undo(&paths, &backup).unwrap(), (2, 0));
        let undone = Lawn::load(&paths).unwrap();
        assert_eq!(undone.find("a_1s3").unwrap().best.as_ref().unwrap().objective, 40);
        assert!(undone.find("c_1s3").unwrap().best.is_none());
        assert!(undo(&paths, &backups(&paths)[0]).is_err());

        std::fs::remove_dir_all(root).unwrap();
    }
}
