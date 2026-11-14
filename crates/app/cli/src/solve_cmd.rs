//! `mow solve <instance.json> <config-label> [--time SECONDS] [--out FILE]
//! [--svg DIR] [--verify] [--kick-scale F]`: one config on one instance.

use std::path::{Path, PathBuf};

use mow_core::{lower_bound, NoObserver, Observer, Problem};
use mow_pipeline::{solve, Attempt, SolveConfig};
use mow_store::safe_name;
use mow_viz::SvgObserver;

use crate::args::parse;
use crate::grid::{time_limit_for, version_tag};
use crate::layout::{repo_root, DEFAULT_OUT};
use crate::meta::{solution_file, Made};
use crate::official::{confirm, VerifierPool};
use crate::registry::Kit;

pub const USAGE: &str = "usage: mow solve <instance.json> <config-label> [--time SECONDS] [--out FILE] \
                         [--svg DIR] [--verify] [--kick-scale F]";

#[derive(Debug)]
pub struct SolveArgs {
    pub instance: PathBuf,
    pub config: SolveConfig,
    pub time: Option<f64>,
    pub out: Option<PathBuf>,
    pub svg: Option<PathBuf>,
    pub verify: bool,
    pub kick_scale: f64,
}

pub fn parse_args(raw: impl Iterator<Item = String>) -> Result<SolveArgs, String> {
    let parsed = parse(raw, &["--time", "--out", "--svg", "--kick-scale"], &["--verify"])?;

    let [instance, label]: [String; 2] = parsed
        .positional
        .clone()
        .try_into()
        .map_err(|_| format!("need an instance file and a config label\n{USAGE}"))?;
    let config = SolveConfig::parse(&label).map_err(|e| e.to_string())?;

    Ok(SolveArgs {
        instance: PathBuf::from(instance),
        config,
        time: parsed.number("--time")?,
        out: parsed.value("--out").map(PathBuf::from),
        svg: parsed.value("--svg").map(PathBuf::from),
        verify: parsed.switch("--verify"),
        kick_scale: parsed.number("--kick-scale")?.unwrap_or(1.0),
    })
}

pub fn run(args: &SolveArgs) -> Result<(), String> {
    let problem = mow_io::load_problem(&args.instance).map_err(|e| e.to_string())?;
    let kit = Kit::new(&args.config, args.kick_scale)?;
    let time_limit = args.time.unwrap_or_else(|| time_limit_for(problem.num_cells()));
    if !(time_limit.is_finite() && time_limit >= 0.0) {
        return Err(format!("--time must be a number of seconds, not {time_limit}"));
    }

    let svg = match &args.svg {
        Some(dir) => {
            Some(SvgObserver::new(dir).map_err(|e| format!("cannot write to {}: {e}", dir.display()))?)
        }
        None => None,
    };
    let observer: &dyn Observer = match &svg {
        Some(svg) => svg,
        None => &NoObserver,
    };

    let attempt = solve(
        &problem,
        &kit.plugins(observer),
        &args.config,
        kit.strategy(),
        crate::mine::job::duration(time_limit),
    );
    print_attempt(&problem, &attempt);
    if let Some(svg) = &svg {
        println!("pictures   {} in {}", svg.written(), svg.dir().display());
    }

    let Some(solution) = &attempt.solution else {
        return Err("no solution".to_owned());
    };
    let made = Made {
        job: format!(
            "{}|{}|s{}|v{}",
            problem.uid(),
            attempt.label,
            args.config.seed(),
            version_tag(args.kick_scale)
        ),
        config: attempt.label.clone(),
        round: None,
        kick_scale: args.kick_scale,
        time_limit,
        seconds: attempt.seconds,
        polished_from: None,
    };
    let file = solution_file(solution, &made);

    let out = args
        .out
        .clone()
        .unwrap_or_else(|| default_out(&problem, &attempt.label));
    mow_io::save_solution_file(&out, &file).map_err(|e| e.to_string())?;
    println!("written    {}", out.display());

    if args.verify {
        verify(&args.instance, &file, solution.objective())?;
    }

    match attempt.feasible() {
        true => Ok(()),
        false => Err("the solution misses cells".to_owned()),
    }
}

fn print_attempt(problem: &Problem, attempt: &Attempt) {
    let bound = lower_bound(problem);

    println!(
        "instance   {} (k = {}, {} cells)",
        problem.uid(),
        problem.k(),
        problem.num_cells()
    );
    println!("config     {}", attempt.label);
    match attempt.objective() {
        Some(objective) => println!(
            "objective  {objective}  ({:.4} x the lower bound {bound})",
            objective as f64 / bound.max(1) as f64
        ),
        None => println!("objective  none  (lower bound {bound})"),
    }
    println!("seconds    {:.2}", attempt.seconds);

    match attempt.errors.is_empty() {
        true => println!("coverage   every cell mowed (Rust pre-check)"),
        false => attempt.errors.iter().for_each(|e| println!("error      {e}")),
    }
}

/// `data/rust/solve/<uid>/<label>.solution.json`.
fn default_out(problem: &Problem, label: &str) -> PathBuf {
    Path::new(DEFAULT_OUT)
        .join("solve")
        .join(problem.uid())
        .join(format!("{}.solution.json", safe_name(label)))
}

fn verify(instance: &Path, file: &mow_io::SolutionFile, objective: u64) -> Result<(), String> {
    let dir = instance.parent().unwrap_or(Path::new("."));
    let pool = VerifierPool::start(&repo_root(), dir, 1).map_err(|e| e.to_string())?;

    confirm(&pool, file, objective)?;
    println!("verified   official verifier agrees, objective {objective}");

    Ok(())
}
