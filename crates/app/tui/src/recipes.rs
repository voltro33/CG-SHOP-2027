//! What the menus can start, as run specs: a pass of a search over some
//! instances, packing a submission zip, building a commit's binary, and an
//! A/B experiment between two binaries.

use std::path::{Path, PathBuf};

use crate::clock::{now, stamp};
use crate::data::{Lawn, Paths};
use crate::runs::{Spec, Step};

/// A search step over the stored bests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Search {
    Lns,
    Turns,
    Polish,
}

impl Search {
    pub const ALL: [Self; 3] = [Self::Lns, Self::Turns, Self::Polish];

    pub fn name(self) -> &'static str {
        match self {
            Self::Lns => "lns",
            Self::Turns => "turns",
            Self::Polish => "polish",
        }
    }

    /// The prefix of its job labels in the job log.
    fn label(self) -> &'static str {
        match self {
            Self::Lns => "lns/",
            Self::Turns => "turns/",
            Self::Polish => "polish/",
        }
    }
}

/// Which instances a run covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Set {
    All,
    /// The fixed bench list, `data/bench.txt`.
    Bench,
    Cutters(usize),
    Family(&'static str),
}

impl Set {
    pub fn name(&self) -> String {
        match self {
            Self::All => "all instances".to_owned(),
            Self::Bench => "the bench set".to_owned(),
            Self::Cutters(k) => format!("k = {k}"),
            Self::Family(f) => format!("{f} cutters"),
        }
    }

    /// The uids it holds.
    pub fn uids(&self, paths: &Paths, lawn: &Lawn) -> Result<Vec<String>, String> {
        let every = lawn.instances.iter();

        Ok(match self {
            Self::All => every.map(|i| i.uid.clone()).collect(),
            Self::Bench => read_list(&paths.root.join("data").join("bench.txt"))?,
            Self::Cutters(k) => every.filter(|i| i.k == *k).map(|i| i.uid.clone()).collect(),
            Self::Family(f) => every.filter(|i| i.family == *f).map(|i| i.uid.clone()).collect(),
        })
    }
}

/// A pass of `search` over `set`.
#[derive(Debug, Clone, PartialEq)]
pub struct Pass {
    pub search: Search,
    pub set: Set,
    pub seconds: u32,
    pub threads: usize,
    /// For `lns` over all instances: the pass number, each a new set of jobs.
    pub pass: u32,
}

/// The run of a pass. Over all instances it goes through the resumable job
/// log; over a subset, through a uid list written into the run's folder.
pub fn pass_spec(paths: &Paths, lawn: &Lawn, pass: &Pass) -> Result<Spec, String> {
    let id = run_id(pass.search.name());
    let dir = runs_dir(paths).join(&id);
    let mow = dir.join(exe("mow"));

    let mut args = vec![pass.search.name().to_owned()];
    let mut files = Vec::new();
    match &pass.set {
        Set::All => {
            args.push("--all".to_owned());
            if pass.search == Search::Lns {
                args.extend(["--pass".to_owned(), pass.pass.to_string()]);
            }
        }
        set => {
            let uids = set.uids(paths, lawn)?;
            if uids.is_empty() {
                return Err(format!("{} holds no instances", set.name()));
            }
            files.push(("instances.txt".to_owned(), uids.join("\n") + "\n"));
            args.extend([
                "--bench".to_owned(),
                dir.join("instances.txt").to_string_lossy().into_owned(),
            ]);
        }
    }
    args.extend(numbers(&[
        ("--seconds", pass.seconds as usize),
        ("--threads", pass.threads),
        ("--verifiers", 1),
    ]));
    let path = |p: PathBuf| p.to_string_lossy().into_owned();
    args.extend([
        "--out".to_owned(),
        path(paths.data.clone()),
        "--instances".to_owned(),
        path(paths.instances.clone()),
        "--catalog".to_owned(),
        path(paths.data.join("catalog.csv")),
    ]);

    let title = match (pass.search, &pass.set) {
        (Search::Lns, Set::All) => {
            format!("lns pass {} on all instances, {} s each", pass.pass, pass.seconds)
        }
        (search, set) => format!("{} on {}, {} s each", search.name(), set.name(), pass.seconds),
    };

    Ok(Spec {
        id,
        kind: pass.search.name().to_owned(),
        title,
        started: now(),
        score_before: lawn.score(),
        data: paths.data.clone(),
        instances: paths.instances.clone(),
        files,
        phases: vec![
            vec![copy_mow(&paths.mow, &mow)],
            vec![exec(pass.search.name(), &mow, args, &paths.root, paths)],
        ],
    })
}

/// The next unused `lns` pass number: one past the highest in the job log.
pub fn next_lns_pass(paths: &Paths) -> u32 {
    let Ok(text) = std::fs::read_to_string(paths.job_log()) else {
        return 1;
    };

    text.lines()
        .filter_map(|line| line.split("/pass=").nth(1))
        .filter_map(|rest| {
            rest.split(|c: char| !c.is_ascii_digit())
                .next()?
                .parse::<u32>()
                .ok()
        })
        .max()
        .map_or(1, |p| p + 1)
}

/// How many jobs of `search` the log holds, for passes that skip them.
pub fn logged_jobs(paths: &Paths, search: Search) -> usize {
    let Ok(text) = std::fs::read_to_string(paths.job_log()) else {
        return 0;
    };
    let needle = format!("\"label\":\"{}", search.label());

    text.lines().filter(|line| line.contains(&needle)).count()
}

/// Packing the stored bests into a verified zip under `data/rust/`, with
/// `tools/pack/pack.py`.
pub fn pack_spec(paths: &Paths, lawn: &Lawn) -> Result<Spec, String> {
    let zip = zip_name(&paths.data);
    let python = mow_verify::python_in(&paths.root);
    let script = paths.root.join("tools").join("pack").join("pack.py");
    let args = vec![
        script.to_string_lossy().into_owned(),
        paths.root.to_string_lossy().into_owned(),
        zip.to_string_lossy().into_owned(),
    ];

    Ok(Spec {
        id: run_id("pack"),
        kind: "pack".to_owned(),
        title: format!(
            "pack {}",
            zip.file_name()
                .map_or(String::new(), |n| n.to_string_lossy().into_owned())
        ),
        started: now(),
        score_before: lawn.score(),
        data: paths.data.clone(),
        instances: paths.instances.clone(),
        files: Vec::new(),
        phases: vec![vec![exec("pack", &python, args, &paths.root, paths)]],
    })
}

/// `submission-YYYY-MM-DD.zip`, with `-2`, `-3`... if taken.
fn zip_name(data: &Path) -> PathBuf {
    let base = format!("submission-{}", crate::clock::date(now()));

    (1..)
        .map(|n| match n {
            1 => data.join(format!("{base}.zip")),
            n => data.join(format!("{base}-{n}.zip")),
        })
        .find(|p| !p.exists())
        .unwrap_or_else(|| data.join(format!("{base}.zip")))
}

/// Where runs live.
pub fn runs_dir(paths: &Paths) -> PathBuf {
    paths.data.join("runs")
}

/// `<stamp>-<kind>`: unique per second, sorting by time.
pub fn run_id(kind: &str) -> String {
    format!("{}-{kind}", stamp(now()))
}

/// The binary name on this platform.
pub fn exe(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}

/// The `mow` binary copied to `to`, so a rebuild never touches a run.
fn copy_mow(from: &Path, to: &Path) -> Step {
    Step::Copy {
        name: "binary".to_owned(),
        from: from.to_path_buf(),
        to: to.to_path_buf(),
    }
}

/// A command run from `cwd` with the verifier's Python set.
pub fn exec(name: &str, program: &Path, args: Vec<String>, cwd: &Path, paths: &Paths) -> Step {
    let python = mow_verify::python_in(&paths.root);

    Step::Exec {
        name: name.to_owned(),
        program: program.to_path_buf(),
        args,
        cwd: cwd.to_path_buf(),
        env: vec![(
            mow_verify::PYTHON_ENV.to_owned(),
            python.to_string_lossy().into_owned(),
        )],
    }
}

/// `--flag value` pairs.
pub fn numbers(pairs: &[(&str, usize)]) -> Vec<String> {
    pairs
        .iter()
        .flat_map(|(flag, value)| [(*flag).to_owned(), value.to_string()])
        .collect()
}

/// Uids of a list file: one per line, `#` starts a comment.
pub fn read_list(path: &Path) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;

    Ok(text
        .lines()
        .map(|line| line.split('#').next().unwrap_or("").trim())
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::tests::instance;
    use crate::runs::tests::scratch;

    fn lawn() -> Lawn {
        Lawn {
            instances: vec![
                instance("a_1s3", 1, "square", Some(120)),
                instance("b_2x5", 2, "X", Some(150)),
            ],
            latest: None,
        }
    }

    #[test]
    fn a_pass_on_all_goes_through_the_log_and_a_subset_through_a_list() {
        let root = scratch("recipes");
        let paths = crate::data::paths_under(&root, "instances");
        let pass = Pass {
            search: Search::Lns,
            set: Set::All,
            seconds: 300,
            threads: 12,
            pass: 5,
        };

        let all = pass_spec(&paths, &lawn(), &pass).unwrap();
        let Step::Exec { args, .. } = &all.phases[1][0] else {
            panic!()
        };
        assert_eq!(args[..4], ["lns", "--all", "--pass", "5"]);
        assert!(args.windows(2).any(|w| w == ["--threads", "12"]));
        assert!(all.files.is_empty());
        assert_eq!(all.title, "lns pass 5 on all instances, 300 s each");

        let subset = pass_spec(
            &paths,
            &lawn(),
            &Pass {
                set: Set::Family("X"),
                search: Search::Turns,
                ..pass
            },
        )
        .unwrap();
        let Step::Exec { args, .. } = &subset.phases[1][0] else {
            panic!()
        };
        assert_eq!(args[..2], ["turns", "--bench"]);
        assert_eq!(subset.files, [("instances.txt".to_owned(), "b_2x5\n".to_owned())]);

        let none = Pass {
            set: Set::Cutters(3),
            search: Search::Lns,
            seconds: 1,
            threads: 1,
            pass: 1,
        };
        assert!(pass_spec(&paths, &lawn(), &none).is_err());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_next_pass_follows_the_highest_logged() {
        let root = scratch("next-pass");
        let paths = crate::data::paths_under(&root, "instances");
        std::fs::create_dir_all(&paths.data).unwrap();
        assert_eq!(next_lns_pass(&paths), 1);

        std::fs::write(
            paths.job_log(),
            "{\"label\":\"lns/temperature=0.0003/pass=3\"}\n{\"label\":\"lns/temperature=0.0003/pass=12\"}\n{\"label\":\"turns/window=6\"}\n",
        )
        .unwrap();
        assert_eq!(next_lns_pass(&paths), 13);
        assert_eq!(logged_jobs(&paths, Search::Turns), 1);
        assert_eq!(logged_jobs(&paths, Search::Lns), 2);

        std::fs::remove_dir_all(root).unwrap();
    }
}
