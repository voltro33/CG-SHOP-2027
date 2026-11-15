//! What the menus show: every challenge instance with its lower bound and
//! the best tour stored for it.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use mow_report::bests::read_bests;
use mow_report::catalog;
use mow_report::family::{family, pessimistic_score};

/// Where the data lives, all under the repository root.
#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
    /// `data/rust`: `best/`, `candidates/`, `catalog.csv`, `jobs.jsonl`.
    pub data: PathBuf,
    /// The challenge instance files.
    pub instances: PathBuf,
    /// The `mow` binary runs start: the one running, unless a test says.
    pub mow: PathBuf,
}

impl Paths {
    pub fn best_dir(&self) -> PathBuf {
        self.data.join("best")
    }

    pub fn candidates_dir(&self, uid: &str) -> PathBuf {
        self.data.join("candidates").join(uid)
    }

    pub fn instance_file(&self, uid: &str) -> PathBuf {
        self.instances.join(format!("{uid}.instance.json"))
    }

    pub fn job_log(&self) -> PathBuf {
        self.data.join("jobs.jsonl")
    }

    /// Where the viewer pages go: inside the git-ignored data folder.
    pub fn view_dir(&self) -> PathBuf {
        self.data.join("view")
    }
}

/// One instance and its stored best.
#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    pub uid: String,
    pub k: usize,
    pub family: &'static str,
    pub cells: usize,
    pub cutter_cells: usize,
    pub lower_bound: u64,
    pub best: Option<Best>,
}

/// The stored best of an instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Best {
    pub objective: u64,
    /// The job that found it, `?` if unknown.
    pub label: String,
    pub path: PathBuf,
}

impl Instance {
    /// `(best - bound) / bound` in percent.
    pub fn gap(&self) -> Option<f64> {
        let best = self.best.as_ref()?.objective as f64;
        let bound = self.lower_bound as f64;

        (bound > 0.0).then(|| 100.0 * (best - bound) / bound)
    }

    /// `(bound / best)^2`, 0 without a best.
    pub fn points(&self) -> f64 {
        self.best
            .as_ref()
            .and_then(|b| pessimistic_score(self.lower_bound, b.objective))
            .unwrap_or(0.0)
    }
}

/// Every challenge instance, by uid.
#[derive(Debug, Clone, Default)]
pub struct Lawn {
    pub instances: Vec<Instance>,
    /// The most recently written best, and when.
    pub latest: Option<(String, SystemTime)>,
}

impl Lawn {
    /// Reads `catalog.csv` and the bests. Slow on the largest tours (it
    /// parses every best file), so the menus load it once and on refresh.
    pub fn load(paths: &Paths) -> Result<Self, String> {
        let catalog = catalog::read(&paths.data.join("catalog.csv"))?;
        let bests = read_bests(&paths.best_dir())?;

        let mut instances: Vec<Instance> = catalog
            .into_iter()
            .filter(catalog::Features::is_challenge)
            .map(|row| Instance {
                family: family(&row.instance),
                best: bests.get(&row.instance).map(|b| Best {
                    objective: b.objective,
                    label: job_label(&row.instance, &b.label),
                    path: b.path.clone(),
                }),
                uid: row.instance,
                k: row.k,
                cells: row.cells,
                cutter_cells: row.cutter_cells,
                lower_bound: row.lower_bound,
            })
            .collect();
        instances.sort_by(|a, b| a.uid.cmp(&b.uid));
        let latest = latest(&instances);

        Ok(Self { instances, latest })
    }

    pub fn score(&self) -> f64 {
        self.instances.iter().map(Instance::points).sum()
    }

    pub fn find(&self, uid: &str) -> Option<&Instance> {
        self.instances.iter().find(|i| i.uid == uid)
    }
}

/// The config part of a job id `uid|config|seed|version`, without the uid
/// every row already shows.
fn job_label(uid: &str, job: &str) -> String {
    let rest = job.strip_prefix(uid).map_or(job, |r| r.trim_start_matches('|'));
    rest.split('|').next().unwrap_or(rest).to_owned()
}

/// The best file written last, by its modification time.
fn latest(instances: &[Instance]) -> Option<(String, SystemTime)> {
    instances
        .iter()
        .filter_map(|i| {
            let path = &i.best.as_ref()?.path;
            let modified = std::fs::metadata(path).ok()?.modified().ok()?;
            Some((i.uid.clone(), modified))
        })
        .max_by_key(|(_, time)| *time)
}

/// The repository's usual layout under `root`.
pub fn paths_under(root: &Path, instances: &str) -> Paths {
    Paths {
        root: root.to_path_buf(),
        data: root.join("data").join("rust"),
        instances: root.join(instances),
        mow: std::env::current_exe().unwrap_or_else(|_| PathBuf::from("mow")),
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// An instance with bound 100 and a best of `best`, if any.
    pub fn instance(uid: &str, k: usize, family: &'static str, best: Option<u64>) -> Instance {
        Instance {
            uid: uid.to_owned(),
            k,
            family,
            cells: 1000,
            cutter_cells: 9,
            lower_bound: 100,
            best: best.map(|objective| Best {
                objective,
                label: "lns".to_owned(),
                path: PathBuf::from(format!("{uid}.solution.json")),
            }),
        }
    }

    #[test]
    fn gap_and_points_follow_the_bound() {
        let at_bound = instance("a", 1, "square", Some(100));
        let doubled = instance("b", 1, "square", Some(200));
        let missing = instance("c", 1, "square", None);

        assert_eq!(at_bound.gap(), Some(0.0));
        assert_eq!(doubled.gap(), Some(100.0));
        assert_eq!(doubled.points(), 0.25);
        assert_eq!(missing.gap(), None);
        assert_eq!(missing.points(), 0.0);

        let lawn = Lawn {
            instances: vec![at_bound, doubled, missing],
            latest: None,
        };
        assert_eq!(lawn.score(), 1.25);
        assert!(lawn.find("b").is_some());
    }

    #[test]
    fn job_labels_drop_the_uid_and_the_seed() {
        assert_eq!(job_label("a_1x3", "a_1x3|lns/pass=3|s0|vr1"), "lns/pass=3");
        assert_eq!(job_label("a_1x3", "?"), "?");
    }
}
