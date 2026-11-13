//! Every pipeline event as a numbered picture in one folder:
//! `000-started.svg`, `001-segments.svg`, `002-ordered.svg`,
//! `003-routes-repair.svg`, ..., so a run can be replayed step by step.
//!
//! Pictures are a side channel: a picture that cannot be written must not
//! stop or change the run. So [`Observer::observe`] never panics and never
//! returns an error; a failed write is reported on stderr and counted (see
//! [`SvgObserver::failures`]), and the run goes on. The failed picture still
//! uses up its number, so the numbers keep matching the event order.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use mow_core::{Event, Observer};

use crate::scene::Scene;

#[derive(Debug)]
pub struct SvgObserver {
    dir: PathBuf,
    next: AtomicUsize,
    written: AtomicUsize,
    failures: AtomicUsize,
}

impl SvgObserver {
    /// Pictures go into `dir`, which is created if missing. Files already
    /// there with the same names are replaced.
    pub fn new(dir: impl AsRef<Path>) -> io::Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir)?;

        Ok(Self {
            dir,
            next: AtomicUsize::new(0),
            written: AtomicUsize::new(0),
            failures: AtomicUsize::new(0),
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Pictures written so far.
    pub fn written(&self) -> usize {
        self.written.load(Ordering::Relaxed)
    }

    /// Pictures that could not be written.
    pub fn failures(&self) -> usize {
        self.failures.load(Ordering::Relaxed)
    }

    fn save(&self, name: &str, scene: Scene<'_>) {
        let index = self.next.fetch_add(1, Ordering::Relaxed);
        let path = self.dir.join(format!("{index:03}-{name}.svg"));

        match scene.save(&path) {
            Ok(()) => {
                self.written.fetch_add(1, Ordering::Relaxed);
            }
            Err(error) => {
                self.failures.fetch_add(1, Ordering::Relaxed);
                eprintln!("mow-viz: could not write {}: {error}", path.display());
            }
        }
    }
}

impl Observer for SvgObserver {
    fn observe(&self, event: &Event<'_>) {
        match *event {
            Event::Started { problem, label } => self.save("started", Scene::new(problem).title(label)),

            Event::Segments { problem, segments } => {
                self.save(
                    "segments",
                    Scene::new(problem).title("segments").segments(segments),
                );
            }

            Event::Ordered {
                problem,
                segments,
                order,
            } => self.save(
                "ordered",
                Scene::new(problem).title("ordered").order(segments, order),
            ),

            // drawn with its way home added, which shows where a split closes
            Event::Walk { problem, step, walk } => {
                let name = format!("walk-{}", file_safe(step));
                match walk.closed() {
                    Ok(route) => self.save(&name, Scene::new(problem).title(step).routes(&[route])),
                    Err(error) => eprintln!("mow-viz: cannot draw walk {step}: {error}"),
                }
            }

            Event::Routes {
                problem,
                step,
                routes,
            } => {
                let name = format!("routes-{}", file_safe(step));
                self.save(&name, Scene::new(problem).title(step).routes(routes));
            }

            Event::Finished { problem, solution } => {
                self.save(
                    "finished",
                    Scene::new(problem).title("finished").routes(solution.routes()),
                );
            }
        }
    }
}

/// `text` with everything but ASCII letters, digits, `-` and `_` replaced
/// by `-`, so a step name cannot leave the folder or break a file name.
fn file_safe(text: &str) -> String {
    let safe: String = text
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => c,
            _ => '-',
        })
        .collect();

    if safe.is_empty() {
        "step".to_string()
    } else {
        safe
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_names_stay_inside_the_folder() {
        assert_eq!(file_safe("repair"), "repair");
        assert_eq!(file_safe("../x y"), "---x-y");
        assert_eq!(file_safe(""), "step");
    }
}
