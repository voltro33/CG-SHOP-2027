//! [`DirStore`]: the best solution per instance and the candidates close to
//! it, in the folders Python's miner uses.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use mow_core::{Offer, PortError, Solution, SolutionStore};
use mow_io::{load_solution, load_solution_file, SolutionFile};

use crate::atomic::{safe_name, write_atomic};
use crate::external;

/// A candidate is kept when it is at most this much longer than the best.
pub const CANDIDATE_SLACK: f64 = 1.05;

const SUFFIX: &str = ".solution.json";

/// `<root>/best/<uid>.solution.json` and
/// `<root>/candidates/<uid>/<label>.solution.json`.
///
/// One lock guards every offer, so threads never race on a file. Another
/// process may write `best/` too: before replacing a best, the file on disk
/// has the last word.
#[derive(Debug)]
pub struct DirStore {
    best: PathBuf,
    candidates: PathBuf,
    /// The best objective per instance as last seen, so most offers need no read.
    known: Mutex<BTreeMap<String, u64>>,
}

impl DirStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, PortError> {
        let root = root.as_ref();
        let best = root.join("best");
        let candidates = root.join("candidates");

        for dir in [&best, &candidates] {
            std::fs::create_dir_all(dir).map_err(|e| external(dir, e))?;
        }

        Ok(Self {
            best,
            candidates,
            known: Mutex::new(BTreeMap::new()),
        })
    }

    /// Where the best of `uid` is kept.
    pub fn best_path(&self, uid: &str) -> Result<PathBuf, PortError> {
        Ok(self.best.join(format!("{}{SUFFIX}", plain(uid)?)))
    }

    /// The stored best as it is on disk, `meta` included.
    pub fn best_file(&self, uid: &str) -> Result<Option<SolutionFile>, PortError> {
        let path = self.best_path(uid)?;
        if !path.is_file() {
            return Ok(None);
        }

        load_solution_file(&path)
            .map(Some)
            .map_err(|e| external(&path, e))
    }

    pub fn best_objective(&self, uid: &str) -> Result<Option<u64>, PortError> {
        read_objective(&self.best_path(uid)?)
    }

    /// Offers a verified solution with its `meta`. The candidate file is
    /// named after `meta.job` when there is one, else after the objective.
    pub fn offer_file(&self, file: &SolutionFile) -> Result<Offer, PortError> {
        let solution = file
            .to_solution()
            .map_err(|e| external(Path::new(&file.instance_uid), e))?;
        let uid = solution.uid();
        let objective = solution.objective();
        let target = self.best_path(uid)?;
        let json = file.to_json().map_err(|e| external(&target, e))?;

        let mut known = self.known.lock().unwrap_or_else(|p| p.into_inner());
        let current = match known.get(uid) {
            Some(&current) => Some(current),
            None => read_objective(&target)?,
        };

        // the best is written first, so a failing candidate write never costs a new best
        let better = current.map_or(true, |c| objective < c);
        let stored = better && self.store_best(&mut known, uid, &target, objective, &json)?;

        let close = current.map_or(true, |c| objective as f64 <= c as f64 * CANDIDATE_SLACK);
        if close {
            let name = format!("{}{SUFFIX}", safe_name(&candidate_label(file, objective)));
            let path = self.candidates.join(uid).join(name);

            let written = write_atomic(&path, &json).map_err(|e| external(&path, e));
            match written {
                Err(error) if stored => eprintln!("store: best kept, candidate lost: {error}"),
                other => other?,
            }
        }

        if !stored {
            if let Some(current) = current {
                known.entry(uid.to_owned()).or_insert(current);
            }
        }

        Ok(match (stored, close) {
            (true, _) => Offer::Best,
            (false, true) => Offer::Candidate,
            (false, false) => Offer::Rejected,
        })
    }

    /// Writes a new best unless another process stored a better one meanwhile.
    fn store_best(
        &self,
        known: &mut BTreeMap<String, u64>,
        uid: &str,
        target: &Path,
        objective: u64,
        json: &str,
    ) -> Result<bool, PortError> {
        if let Some(on_disk) = read_objective(target)? {
            if on_disk <= objective {
                known.insert(uid.to_owned(), on_disk);
                return Ok(false);
            }
        }

        write_atomic(target, json).map_err(|e| external(target, e))?;
        known.insert(uid.to_owned(), objective);

        Ok(true)
    }
}

impl SolutionStore for DirStore {
    fn best(&self, uid: &str) -> Result<Option<Solution>, PortError> {
        let path = self.best_path(uid)?;
        if !path.is_file() {
            return Ok(None);
        }

        load_solution(&path).map(Some).map_err(|e| external(&path, e))
    }

    fn offer(&self, solution: &Solution) -> Result<Offer, PortError> {
        self.offer_file(&SolutionFile::from_solution(solution))
    }
}

fn read_objective(path: &Path) -> Result<Option<u64>, PortError> {
    if !path.is_file() {
        return Ok(None);
    }

    // a broken best file must not block every later offer: it gets replaced
    match load_solution(path) {
        Ok(solution) => Ok(Some(solution.objective())),
        Err(error) => {
            eprintln!(
                "store: unreadable best {}, treated as missing: {error}",
                path.display()
            );
            Ok(None)
        }
    }
}

/// `meta.job` if it is a string, as the Python miner records it.
fn candidate_label(file: &SolutionFile, objective: u64) -> String {
    let meta: serde_json::Value = serde_json::from_str(file.meta.get()).unwrap_or_default();

    match meta.get("job").and_then(|job| job.as_str()) {
        Some(job) if !job.is_empty() => job.to_owned(),
        _ => format!("objective-{objective}"),
    }
}

/// A uid that is a plain file name, so it can never reach outside the store.
fn plain(uid: &str) -> Result<&str, PortError> {
    let plain = !uid.is_empty() && !uid.contains(['/', '\\', ':']) && uid != "." && uid != "..";
    match plain {
        true => Ok(uid),
        false => Err(PortError::NotFound(format!("instance {uid:?}"))),
    }
}
