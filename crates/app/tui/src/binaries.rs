//! The `mow` binaries experiments compare, kept in `data/rust/bin/` as
//! `<label>.exe` with a `<label>.json` saying what they were built from:
//! the running build saved as it is, or a commit built in a git worktree.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::clock::now;
use crate::data::{Lawn, Paths};
use crate::recipes::{exe, exec, run_id};
use crate::runs::{Spec, Step};

/// What a kept binary was built from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meta {
    pub commit: String,
    pub subject: String,
    /// Built with changes not committed.
    pub dirty: bool,
    pub made: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binary {
    pub label: String,
    pub path: PathBuf,
    pub meta: Option<Meta>,
}

/// A commit, newest first in [`commits`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub short: String,
    pub subject: String,
    pub time: u64,
}

pub fn bin_dir(paths: &Paths) -> PathBuf {
    paths.data.join("bin")
}

/// Every kept binary, newest first.
pub fn list(paths: &Paths) -> Vec<Binary> {
    let Ok(entries) = std::fs::read_dir(bin_dir(paths)) else {
        return Vec::new();
    };
    let suffix = std::env::consts::EXE_SUFFIX;

    let mut found: Vec<Binary> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension().map_or(suffix.is_empty(), |e| {
                format!(".{}", e.to_string_lossy()) == suffix
            })
        })
        .filter_map(|path| {
            let label = path.file_stem()?.to_string_lossy().into_owned();
            let meta = std::fs::read_to_string(path.with_extension("json"))
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok());
            Some(Binary { label, path, meta })
        })
        .collect();
    found.sort_by_key(|b| std::cmp::Reverse(b.meta.as_ref().map_or(0, |m| m.made)));

    found
}

/// Copies the running `mow` into the kept binaries, labelled by the
/// checked-out commit (`-dirty` with uncommitted changes).
pub fn save_current(paths: &Paths) -> Result<String, String> {
    let head = git(&paths.root, &["log", "-1", "--format=%h%x09%s"])?;
    let (commit, subject) = head.trim().split_once('\t').unwrap_or((head.trim(), ""));
    let dirty = !git(&paths.root, &["status", "--porcelain", "--untracked-files=no"])?
        .trim()
        .is_empty();
    let label = if dirty {
        format!("{commit}-dirty")
    } else {
        commit.to_owned()
    };

    let meta = Meta {
        commit: commit.to_owned(),
        subject: subject.to_owned(),
        dirty,
        made: now(),
    };
    let dir = bin_dir(paths);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let to = dir.join(exe(&label));
    std::fs::copy(&paths.mow, &to).map_err(|e| format!("cannot copy to {}: {e}", to.display()))?;
    let json = serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(format!("{label}.json")), json).map_err(|e| e.to_string())?;

    Ok(label)
}

/// The last `count` commits of the checked-out branch.
pub fn commits(paths: &Paths, count: usize) -> Result<Vec<Commit>, String> {
    let n = format!("-{count}");
    let text = git(&paths.root, &["log", &n, "--format=%h%x09%ct%x09%s"])?;

    Ok(text
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            Some(Commit {
                short: parts.next()?.to_owned(),
                time: parts.next()?.parse().ok()?,
                subject: parts.next().unwrap_or("").to_owned(),
            })
        })
        .collect())
}

/// The run that builds `commit` in a worktree under `data/rust/worktrees/`
/// (one target folder shared by all, so later builds are incremental) and
/// keeps its `mow` as `bin/<commit>`.
pub fn build_spec(paths: &Paths, lawn: &Lawn, commit: &Commit) -> Result<Spec, String> {
    let trees = paths.data.join("worktrees");
    let tree = trees.join(&commit.short);
    let target = trees.join("target");
    let id = run_id("build");
    let run_dir = crate::recipes::runs_dir(paths).join(&id);

    let meta = Meta {
        commit: commit.short.clone(),
        subject: commit.subject.clone(),
        dirty: false,
        made: now(),
    };
    let meta_json = serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?;
    let path_arg = |p: &Path| p.to_string_lossy().into_owned();

    let checkout = exec(
        "checkout",
        Path::new("git"),
        vec![
            "worktree".into(),
            "add".into(),
            "--force".into(),
            "--detach".into(),
            path_arg(&tree),
            commit.short.clone(),
        ],
        &paths.root,
        paths,
    );
    let Step::Exec {
        name,
        program,
        args,
        cwd,
        mut env,
    } = exec(
        "cargo",
        Path::new("cargo"),
        vec!["build".into(), "--release".into(), "-p".into(), "mow-cli".into()],
        &tree,
        paths,
    )
    else {
        return Err("not a command".to_owned());
    };
    env.push(("CARGO_TARGET_DIR".to_owned(), path_arg(&target)));
    // half the cores, so a build never starves a pass running beside it
    let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    env.push(("CARGO_BUILD_JOBS".to_owned(), (cores / 2).max(1).to_string()));
    let build = Step::Exec {
        name,
        program,
        args,
        cwd,
        env,
    };
    let remove = exec(
        "cleanup",
        Path::new("git"),
        vec![
            "worktree".into(),
            "remove".into(),
            "--force".into(),
            path_arg(&tree),
        ],
        &paths.root,
        paths,
    );

    let bin = bin_dir(paths);
    let mut phases = Vec::new();
    if !tree.exists() {
        phases.push(vec![checkout]);
    }
    phases.extend([
        vec![build],
        vec![
            Step::Copy {
                name: "binary".to_owned(),
                from: target.join("release").join(exe("mow")),
                to: bin.join(exe(&commit.short)),
            },
            Step::Copy {
                name: "meta".to_owned(),
                from: run_dir.join("meta.json"),
                to: bin.join(format!("{}.json", commit.short)),
            },
        ],
        vec![remove],
    ]);

    Ok(Spec {
        id,
        kind: "build".to_owned(),
        title: format!("build {} {}", commit.short, commit.subject),
        started: now(),
        score_before: lawn.score(),
        data: paths.data.clone(),
        instances: paths.instances.clone(),
        files: vec![("meta.json".to_owned(), meta_json)],
        phases,
    })
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;

    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runs::tests::scratch;

    #[test]
    fn a_build_checks_out_builds_keeps_and_cleans_up() {
        let root = scratch("binaries");
        let paths = crate::data::paths_under(&root, "instances");
        let commit = Commit {
            short: "abc1234".to_owned(),
            subject: "feat: x".to_owned(),
            time: 0,
        };

        let spec = build_spec(&paths, &Lawn::default(), &commit).unwrap();
        let names: Vec<Vec<&str>> = spec
            .phases
            .iter()
            .map(|p| p.iter().map(Step::name).collect())
            .collect();
        assert_eq!(
            names,
            [
                vec!["checkout"],
                vec!["cargo"],
                vec!["binary", "meta"],
                vec!["cleanup"]
            ]
        );

        let Step::Exec { env, cwd, .. } = &spec.phases[1][0] else {
            panic!()
        };
        assert!(env.iter().any(|(k, _)| k == "CARGO_TARGET_DIR"));
        assert!(env.iter().any(|(k, _)| k == "CARGO_BUILD_JOBS"));
        assert!(cwd.ends_with("abc1234"));
        assert!(list(&paths).is_empty());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn this_repository_has_commits() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let paths = crate::data::paths_under(&root, "instances");

        let found = commits(&paths, 3).unwrap();
        assert_eq!(found.len(), 3);
        assert!(found[0].time >= found[2].time);
    }
}
