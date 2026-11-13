//! `mow mine`: the job grid of `jobs.py` on threads (port of `mine.py`).
//!
//! - One queue in run order; `--threads` workers take the next job each.
//! - One `Arc<Problem>` per instance, loaded by the first job that needs it
//!   and dropped once the last job of the round on that instance has
//!   started and every running job is done with it.
//! - Every result passes the Rust coverage pre-check in the pipeline. A
//!   result that would beat the stored best is checked by the official
//!   verifier and only then offered to the store; the store therefore holds
//!   verified solutions only (its `candidates/` are the verified bests in
//!   turn). Other results are logged with their pre-checked objective.
//! - Every job is appended to `<out>/jobs.jsonl`; a restart skips the job
//!   ids already there and starts from the bests in `<out>/best`.

pub(crate) mod job;
mod progress;
mod queue;

use std::path::{Path, PathBuf};
use std::time::Instant;

use mow_store::{DirStore, JobLog};

use crate::args::{list, parse};
use crate::grid::{build_jobs, version_tag, Instance, ROUND_ORDER};
use crate::layout::{instance_files, read_uid_list, uid_of, DEFAULT_INSTANCES, DEFAULT_OUT, PYTHON_DATA};
use crate::official::Official;

pub use job::Bests;
pub use progress::Progress;
pub use queue::Queue;

pub const USAGE: &str = "usage: mow mine [--instances DIR] [--only uid,uid | --bench] [--rounds 0,1,...] \
                         [--threads N] [--hours H] [--kick-scale F] [--time SECONDS] [--verifiers N] [--out DIR]";

/// Everything a mining run needs to know.
#[derive(Debug, Clone)]
pub struct MineOptions {
    pub instances: PathBuf,
    /// Only these instances; all in the folder when `None`.
    pub only: Option<Vec<String>>,
    pub rounds: Vec<u32>,
    pub threads: usize,
    /// Stop starting jobs after this long.
    pub hours: Option<f64>,
    pub kick_scale: f64,
    /// Overrides every job's time limit, seconds (for quick runs).
    pub time: Option<f64>,
    /// Official verifier processes.
    pub verifiers: usize,
    pub out: PathBuf,
}

pub fn parse_args(raw: impl Iterator<Item = String>) -> Result<MineOptions, String> {
    let valued = [
        "--instances",
        "--only",
        "--rounds",
        "--threads",
        "--hours",
        "--kick-scale",
        "--time",
        "--verifiers",
        "--out",
    ];
    let parsed = parse(raw, &valued, &["--bench"])?;
    if !parsed.positional.is_empty() {
        return Err(format!("unexpected {:?}\n{USAGE}", parsed.positional));
    }

    let only = match (parsed.value("--only"), parsed.switch("--bench")) {
        (Some(_), true) => return Err("--only and --bench exclude each other".to_owned()),
        (Some(text), false) => Some(list::<String>("--only", text)?),
        (None, true) => Some(read_uid_list(&Path::new(PYTHON_DATA).join("bench.txt"))?),
        (None, false) => None,
    };
    let rounds = match parsed.value("--rounds") {
        Some(text) => list("--rounds", text)?,
        None => ROUND_ORDER.to_vec(),
    };
    let threads = parsed
        .number("--threads")?
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()));

    Ok(MineOptions {
        instances: PathBuf::from(parsed.value("--instances").unwrap_or(DEFAULT_INSTANCES)),
        only,
        rounds,
        threads: threads.max(1),
        hours: parsed.number("--hours")?,
        kick_scale: parsed.number("--kick-scale")?.unwrap_or(1.0),
        time: parsed.number("--time")?,
        verifiers: parsed.number("--verifiers")?.unwrap_or(2),
        out: PathBuf::from(parsed.value("--out").unwrap_or(DEFAULT_OUT)),
    })
}

/// What a mining run did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MineSummary {
    pub total: usize,
    pub skipped: usize,
    pub done: usize,
    pub failed: usize,
    pub bests: usize,
    pub unstarted: usize,
    pub seconds: f64,
}

/// Runs the grid. Errors only for setup problems; a failing job is logged.
pub fn mine(options: &MineOptions, official: &dyn Official) -> Result<MineSummary, String> {
    let started = Instant::now();
    let instances = read_instances(options)?;
    let version = version_tag(options.kick_scale);
    let jobs = build_jobs(&instances, &options.rounds);

    let store = DirStore::open(&options.out).map_err(|e| e.to_string())?;
    let log_path = options.out.join("jobs.jsonl");
    let done = JobLog::read(&log_path).map_err(|e| e.to_string())?;
    let log = JobLog::open(&log_path).map_err(|e| e.to_string())?;

    let done_ids: std::collections::HashSet<&str> = done.iter().map(|r| r.job_id.as_str()).collect();
    let total = jobs.len();
    let pending: Vec<_> = jobs
        .into_iter()
        .filter(|job| !done_ids.contains(job.job_id(&version).as_str()))
        .collect();
    let skipped = total - pending.len();
    eprintln!(
        "{} instances, {total} jobs, {skipped} already done, {} to run on {} threads (kick scale {}, version {version})",
        instances.len(),
        pending.len(),
        options.threads,
        options.kick_scale
    );

    let deadline = options
        .hours
        .and_then(|h| started.checked_add(job::duration(h * 3600.0)));
    let queue = Queue::new(pending, &options.instances, deadline);
    let progress = Progress::new(queue.len());
    let context = job::Context {
        store: &store,
        log: &log,
        official,
        bests: Bests::default(),
        version,
        kick_scale: options.kick_scale,
        time: options.time,
        progress: &progress,
    };

    std::thread::scope(|scope| {
        for _ in 0..options.threads {
            scope.spawn(|| job::work(&queue, &context));
        }
    });

    let unstarted = queue.len();
    if unstarted > 0 {
        eprintln!("time is up, {unstarted} jobs left unstarted");
    }

    Ok(progress.summary(total, skipped, unstarted, started.elapsed().as_secs_f64()))
}

/// The instances to mine, with their cell counts and cutters, in folder order.
fn read_instances(options: &MineOptions) -> Result<Vec<Instance>, String> {
    let files = instance_files(&options.instances)?;
    let mut instances = Vec::new();

    for file in files {
        let uid = uid_of(&file).unwrap_or_default();
        let wanted = options.only.as_ref().map_or(true, |only| only.contains(&uid));
        if !wanted {
            continue;
        }

        // loaded only to learn the size; dropped right away
        let problem = mow_io::load_problem(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        instances.push(Instance {
            uid,
            cells: problem.num_cells(),
            k: problem.k(),
        });
    }

    if let Some(only) = &options.only {
        let missing: Vec<&String> = only
            .iter()
            .filter(|u| !instances.iter().any(|i| &i.uid == *u))
            .collect();
        if !missing.is_empty() {
            return Err(format!(
                "no instance file for {missing:?} in {}",
                options.instances.display()
            ));
        }
    }

    Ok(instances)
}
