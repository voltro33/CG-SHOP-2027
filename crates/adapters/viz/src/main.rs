//! `mow-viz <instance.json> <solution.json> -o out.svg [--title TEXT]`
//!
//! Draws one solution over its instance. Uncovered cells are not computed
//! here, since coverage lives in another crate; the library draws them when
//! the caller hands them over.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use mow_viz::Scene;

const USAGE: &str = "usage: mow-viz <instance.json> <solution.json> -o <out.svg> [--title TEXT]";

#[derive(Debug)]
struct Args {
    instance: PathBuf,
    solution: PathBuf,
    out: PathBuf,
    title: Option<String>,
}

fn parse(mut raw: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut files = Vec::new();
    let (mut out, mut title) = (None, None);

    while let Some(arg) = raw.next() {
        match arg.as_str() {
            "-o" | "--out" => out = Some(raw.next().ok_or("-o needs a file")?),
            "--title" => title = Some(raw.next().ok_or("--title needs text")?),
            "-h" | "--help" => return Err(USAGE.to_string()),
            flag if flag.starts_with('-') => return Err(format!("unknown option {flag}\n{USAGE}")),
            _ => files.push(PathBuf::from(arg)),
        }
    }

    let [instance, solution]: [PathBuf; 2] = files
        .try_into()
        .map_err(|_| format!("need an instance and a solution file\n{USAGE}"))?;
    let out = PathBuf::from(out.ok_or_else(|| format!("missing -o\n{USAGE}"))?);

    Ok(Args {
        instance,
        solution,
        out,
        title,
    })
}

fn run(args: &Args) -> Result<(), String> {
    let started = Instant::now();
    let problem = mow_io::load_problem(&args.instance).map_err(|e| e.to_string())?;
    let solution = mow_io::load_solution(&args.solution).map_err(|e| e.to_string())?;
    let loaded = started.elapsed();

    if solution.uid() != problem.uid() {
        eprintln!(
            "warning: solution is for {}, instance is {}",
            solution.uid(),
            problem.uid()
        );
    }

    let scene = Scene::new(&problem).routes(solution.routes());
    let scene = match &args.title {
        Some(title) => scene.title(title),
        None => scene,
    };
    scene
        .save(&args.out)
        .map_err(|e| format!("could not write {}: {e}", args.out.display()))?;

    let bytes = std::fs::metadata(&args.out).map(|m| m.len()).unwrap_or(0);
    eprintln!(
        "saved {}  {} cells, {} bytes, load {:.2?}, draw {:.2?}",
        args.out.display(),
        problem.num_cells(),
        bytes,
        loaded,
        started.elapsed() - loaded
    );

    Ok(())
}

fn main() -> ExitCode {
    let result = parse(std::env::args().skip(1)).and_then(|args| run(&args));

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
