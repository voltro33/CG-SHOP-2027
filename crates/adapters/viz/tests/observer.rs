mod support;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use mow_core::{Event, Observer, Order, Route, Segment, Solution};
use mow_viz::SvgObserver;
use support::{assert_well_formed, points, rectangle};

/// A fresh, empty folder under the system temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let unique = format!(
            "mow-viz-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );

        let path = std::env::temp_dir().join(unique);
        let _ = fs::remove_dir_all(&path);
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn file_names(dir: &PathBuf) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

#[test]
fn writes_one_numbered_picture_per_event() {
    let temp = TempDir::new("events");
    let observer = SvgObserver::new(&temp.0).unwrap();

    let problem = rectangle(10, 6, 3, 1, &[]);
    let segments = [
        Segment::horizontal(1, 1, 8).unwrap(),
        Segment::horizontal(4, 1, 8).unwrap(),
    ];
    let order = Order::identity(2);
    let routes = [Route::through(&order.waypoints(&segments)).unwrap()];
    let solution = Solution::new("rect", routes.to_vec());

    observer.observe(&Event::Started {
        problem: &problem,
        label: "demo",
    });
    observer.observe(&Event::Segments {
        problem: &problem,
        segments: &segments,
    });
    observer.observe(&Event::Ordered {
        problem: &problem,
        segments: &segments,
        order: &order,
    });
    observer.observe(&Event::Routes {
        problem: &problem,
        step: "re/pair",
        routes: &routes,
    });
    observer.observe(&Event::Finished {
        problem: &problem,
        solution: &solution,
    });

    let names = file_names(&temp.0);
    assert_eq!(
        names,
        [
            "000-started.svg",
            "001-segments.svg",
            "002-ordered.svg",
            "003-routes-re-pair.svg",
            "004-finished.svg",
        ]
    );
    for name in &names {
        assert_well_formed(&fs::read_to_string(temp.0.join(name)).unwrap());
    }
    assert_eq!(observer.failures(), 0);
    assert_eq!(observer.written(), 5);
}

#[test]
fn write_errors_are_counted_not_raised() {
    let temp = TempDir::new("broken");
    let observer = SvgObserver::new(&temp.0).unwrap();

    // the folder becomes a file, so every write fails
    fs::remove_dir_all(&temp.0).unwrap();
    fs::write(&temp.0, b"in the way").unwrap();

    let problem = rectangle(4, 4, 1, 1, &[]);
    observer.observe(&Event::Started {
        problem: &problem,
        label: "x",
    });

    assert_eq!(observer.failures(), 1);
    assert_eq!(observer.written(), 0);
    fs::remove_file(&temp.0).unwrap();
}

#[test]
fn shared_between_threads() {
    let temp = TempDir::new("threads");
    let observer = SvgObserver::new(&temp.0).unwrap();
    let problem = rectangle(4, 4, 1, 1, &[]);
    let route = Route::new(points(&[(0, 0), (3, 0)])).unwrap();

    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                observer.observe(&Event::Routes {
                    problem: &problem,
                    step: "improve",
                    routes: std::slice::from_ref(&route),
                })
            });
        }
    });

    let names = file_names(&temp.0);
    assert_eq!(names.len(), 4);
    assert_eq!(names[3], "003-routes-improve.svg");
}
