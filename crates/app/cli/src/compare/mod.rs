//! `mow compare A B`: two solutions, or two folders of them, side by side.
//!
//! - Two files ([`pair`]): solutions of the same instance. Both must be
//!   valid, else nothing is compared.
//! - Two folders ([`folders`]): the `*.solution.json` files matched by name.
//!   Each pair is checked and compared as two files are. An invalid solution
//!   is reported and its instance is left out.
//!
//! A solution is valid when our coverage count finds every lawn cell mowed
//! and the official verifier agrees, with the same longest tour. The result
//! is a table in the terminal and a page opened in the browser.

pub mod folders;
mod page;
pub mod pair;

use std::path::PathBuf;

use crate::args::parse;
use crate::layout::{DEFAULT_INSTANCES, DEFAULT_OUT};

pub use folders::{FolderComparison, Group, Skipped};
pub use page::summary_page;
pub use pair::{check_pair, Pair, Side};

pub const USAGE: &str = "usage: mow compare A B [--instances DIR] [--out data/rust] \
                         [--verifiers N] [--no-open]\n       \
                         A and B: two solution files of one instance, or two folders of \
                         solutions matched by file name";

#[derive(Debug, Clone)]
pub struct CompareOptions {
    pub a: PathBuf,
    pub b: PathBuf,
    pub instances: PathBuf,
    /// The pages go to `<out>/view/`.
    pub out: PathBuf,
    /// Official verifier processes, for folders.
    pub verifiers: usize,
    pub open: bool,
}

pub fn parse_args(raw: impl Iterator<Item = String>) -> Result<CompareOptions, String> {
    let valued = ["--instances", "--out", "--verifiers"];
    let parsed = parse(raw, &valued, &["--no-open"])?;

    let [a, b] = parsed.positional.as_slice() else {
        return Err(format!(
            "give two files or two folders, got {:?}\n{USAGE}",
            parsed.positional
        ));
    };

    Ok(CompareOptions {
        a: PathBuf::from(a),
        b: PathBuf::from(b),
        instances: PathBuf::from(parsed.value("--instances").unwrap_or(DEFAULT_INSTANCES)),
        out: PathBuf::from(parsed.value("--out").unwrap_or(DEFAULT_OUT)),
        verifiers: parsed.number("--verifiers")?.unwrap_or(8),
        open: !parsed.switch("--no-open"),
    })
}

/// Two files or two folders; anything else is refused.
pub fn run(options: &CompareOptions) -> Result<(), String> {
    for path in [&options.a, &options.b] {
        if !path.exists() {
            return Err(format!("{} does not exist", path.display()));
        }
    }

    match (options.a.is_dir(), options.b.is_dir()) {
        (false, false) => pair::run(options),
        (true, true) => folders::run(options),
        _ => Err(format!("give two files or two folders, not one of each\n{USAGE}")),
    }
}
