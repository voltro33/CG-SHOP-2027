//! The jobs not started yet, in run order, and the problems they share.
//!
//! A problem is loaded once per instance and round, by the first job that
//! needs it, and shared through an `Arc` by every job of that instance. The
//! queue lets go of it when it hands out the last job of the round on that
//! instance, so it is freed as soon as the running jobs are done with it.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use mow_core::Problem;

use crate::grid::Job;
use crate::layout::instance_path;

/// One instance's problem, loaded at most once, by whoever needs it first.
#[derive(Default)]
pub struct Slot {
    cell: OnceLock<Result<Arc<Problem>, String>>,
}

impl Slot {
    /// The problem, loading it from `path` if nobody did yet. Other threads
    /// asking meanwhile wait for that one load.
    pub fn problem(&self, path: &Path) -> Result<Arc<Problem>, String> {
        self.cell
            .get_or_init(|| {
                mow_io::load_problem(path)
                    .map(Arc::new)
                    .map_err(|e| format!("load: {e}"))
            })
            .clone()
    }
}

/// A job handed to a worker, with the shared slot of its problem.
pub struct Task {
    pub job: Job,
    pub path: PathBuf,
    pub slot: Arc<Slot>,
}

struct Queued {
    job: Job,
    /// No later job of the same round needs this instance.
    last: bool,
}

struct State {
    jobs: VecDeque<Queued>,
    slots: HashMap<String, Arc<Slot>>,
}

pub struct Queue {
    state: Mutex<State>,
    instances: PathBuf,
    deadline: Option<Instant>,
}

impl Queue {
    /// `jobs` in run order; nothing starts after `deadline`.
    pub fn new(jobs: Vec<Job>, instances: &Path, deadline: Option<Instant>) -> Self {
        let jobs = mark_last(jobs);

        Self {
            state: Mutex::new(State {
                jobs,
                slots: HashMap::new(),
            }),
            instances: instances.to_path_buf(),
            deadline,
        }
    }

    /// The next job, or `None` when the queue is empty or time is up.
    pub fn next(&self) -> Option<Task> {
        if self.deadline.is_some_and(|d| Instant::now() >= d) {
            return None;
        }

        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        let Queued { job, last } = state.jobs.pop_front()?;

        let uid = job.instance.clone();
        let slot = Arc::clone(state.slots.entry(uid.clone()).or_default());
        if last {
            state.slots.remove(&uid);
        }

        Some(Task {
            path: instance_path(&self.instances, &uid),
            job,
            slot,
        })
    }

    /// Jobs not handed out.
    pub fn len(&self) -> usize {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).jobs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Instances whose problem the queue still holds on to.
    pub fn held(&self) -> usize {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).slots.len()
    }
}

/// Marks each job that is the last of its round on its instance.
fn mark_last(jobs: Vec<Job>) -> VecDeque<Queued> {
    let mut seen = HashSet::new();
    let mut marked: Vec<Queued> = jobs
        .into_iter()
        .rev()
        .map(|job| {
            let last = seen.insert((job.round, job.instance.clone()));
            Queued { job, last }
        })
        .collect();
    marked.reverse();

    marked.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{build_jobs, Instance};

    fn jobs() -> Vec<Job> {
        let instances = ["a", "b"].map(|uid| Instance {
            uid: uid.into(),
            cells: if uid == "a" { 20 } else { 10 },
            k: 1,
        });

        build_jobs(&instances, &[0, 1])
    }

    #[test]
    fn lets_go_of_a_problem_after_its_last_job_of_the_round() {
        let queue = Queue::new(jobs(), Path::new("none"), None);
        assert_eq!(queue.len(), 8);

        let first = queue.next().unwrap();
        assert_eq!(first.job.instance, "a");
        assert_eq!(queue.held(), 1);

        let second = queue.next().unwrap();
        assert!(Arc::ptr_eq(&first.slot, &second.slot));
        assert_eq!(queue.held(), 0);

        let third = queue.next().unwrap();
        assert_eq!((third.job.instance.as_str(), third.job.round), ("b", 0));
        assert!(!Arc::ptr_eq(&first.slot, &third.slot));
    }

    #[test]
    fn a_missing_instance_is_an_error_once_for_everyone() {
        let queue = Queue::new(jobs(), Path::new("no-such-folder"), None);
        let task = queue.next().unwrap();

        assert!(task.slot.problem(&task.path).is_err());
        assert!(task.slot.problem(&task.path).is_err());
    }

    #[test]
    fn nothing_starts_after_the_deadline() {
        let queue = Queue::new(jobs(), Path::new("none"), Some(Instant::now()));

        assert!(queue.next().is_none());
        assert_eq!(queue.len(), 8);
    }
}
