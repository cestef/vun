//! `vun debug smith`: programs made by chance, each tried through every stage and every backend
//! (spec 11): what the checks refuse, what does not lower or verify, what the backends run
//! differently.

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use std::fmt;

use vun_campaign::{Campaign, Divergence, Tried};
use vun_command::{Args, Outcome, Param, PathKind, Values};
use vun_data::Object;
use vun_driver::trial::{Finding, Reach, Trial};
use vun_driver::{Persistence, Session};
use vun_fs::Edit;
use vun_intern::Symbol;
use vun_smith::draw::Config;
use vun_term::{Record, Shell, Status, Style, Text, Verb};

pub(super) const SEED: Param =
    Param::option("seed").about("the seed of the first program (0)").values(Values::Count);
pub(super) const COUNT: Param =
    Param::option("count").about("how many programs, a seed each (20)").values(Values::Count);
const OUT: Param = Param::option("out")
    .about("a directory to keep the program of each finding in, under its seed")
    .values(Values::Path(PathKind::Dir));
const LIMIT: Param = Param::option("limit").about("the seconds a program may run (10)").values(Values::Count);

const REACH: Param = Param::option("reach")
    .about("how far each program is taken: `front` (checks, MIR, LIR), `code` (and code generation) or `run`")
    .values(Values::Choice(|| Reach::ALL.iter().map(|r| r.name()).collect()));

pub(super) const JOBS: Param = Param::option("jobs")
    .about("how many processes try programs at once, each a chunk of seeds (1: this one)")
    .values(Values::Count);
pub(super) const CHUNK: Param =
    Param::option("chunk").about("how many seeds a process of `--jobs` tries (8)").values(Values::Count);
const ALL: Param =
    Param::flag("all").about("keep the program of every seed in `--out`, not only of findings");
const REDUCE: Param =
    Param::flag("reduce").about("make each finding's program smaller while it still finds it");
const TRIALS: Param = Param::option("trials")
    .about("the most programs tried to reduce one finding (300)")
    .values(Values::Count);

pub(super) const PARAMS: &[Param] =
    &[SEED, COUNT, OUT, LIMIT, REACH, REDUCE, ALL, TRIALS, JOBS, CHUNK, vun_driver::codegen::BACKEND];

/// What a program found: the compiler against itself, or against the model.
enum Cause<'a> {
    Finding(&'a Finding),
    Divergence(&'a Divergence),
}

impl Cause<'_> {
    fn stage(&self) -> String {
        match self {
            Self::Finding(finding) => finding.stage().to_string(),
            Self::Divergence(_) => "model".to_owned(),
        }
    }
}

impl fmt::Display for Cause<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Finding(finding) => finding.fmt(f),
            Self::Divergence(divergence) => divergence.fmt(f),
        }
    }
}

/// A program that found something.
struct Found<'a> {
    tried: &'a Tried,
    finding: Cause<'a>,
    kept: Option<PathBuf>,
}

impl Record for Found<'_> {
    fn kind(&self) -> &'static str {
        "finding"
    }

    fn text(&self) -> Text {
        let head = Text::of(Style::Strong, format!("seed {}", self.tried.seed));
        let found = head.plain(": ").styled(Style::Error, self.finding.to_string());
        match &self.kept {
            Some(path) => found.plain(" in ").styled(Style::Literal, path.display().to_string()),
            None => found,
        }
    }

    fn fields(&self) -> Object<'static> {
        Object::new()
            .with("seed", self.tried.seed)
            .with("stage", self.finding.stage())
            .with("finding", self.finding.to_string())
            .with("bytes", self.tried.source.len())
            .with("path", self.kept.as_ref().map(|p| p.display().to_string()))
    }
}

/// What a campaign came to.
struct Summary {
    tried: u64,
    found: u64,
    /// Programs a backend said it cannot express, which the others were held to.
    lacked: u64,
}

impl Record for Summary {
    fn kind(&self) -> &'static str {
        "summary"
    }

    fn text(&self) -> Text {
        Text::of(
            Style::Strong,
            format!(
                "{} programs, {} found something, {} lacked a backend",
                self.tried, self.found, self.lacked
            ),
        )
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("tried", self.tried).with("found", self.found).with("lacked", self.lacked)
    }
}

impl Status for Summary {
    fn verb(&self) -> Verb {
        if self.found == 0 { Verb::good("Agreed") } else { Verb::failed("Found") }
    }
}

pub(super) fn smith(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    // Nothing of programs made once is worth keeping beside them.
    session.persistence = Persistence::Off;
    let first = args.count(&SEED).unwrap_or(0);
    let count = args.count(&COUNT).unwrap_or(20);
    let jobs = args.count(&JOBS).unwrap_or(1);
    if jobs > 1 {
        return super::workers::run(first, count, args.count(&CHUNK).unwrap_or(8), jobs, shell);
    }
    let limit = Duration::from_secs(args.count(&LIMIT).unwrap_or(10));
    let backends = args.values(&vun_driver::codegen::BACKEND).map(Symbol::intern).collect();
    let reach = args.value(&REACH).and_then(Reach::named).unwrap_or_default();
    let trial = Trial { reach, limit, backends };
    let trials = usize::try_from(args.count(&TRIALS).unwrap_or(300)).unwrap_or(usize::MAX);
    let out = args.value(&OUT).map(PathBuf::from);
    let mut campaign = Campaign::new(session, Config::default(), trial);
    let mut summary = Summary { tried: 0, found: 0, lacked: 0 };
    for seed in first..first.saturating_add(count) {
        let tried = campaign.seed(seed);
        summary.tried += 1;
        summary.lacked += u64::from(tried.result.as_ref().is_ok_and(|agreed| !agreed.lacking.is_empty()));
        let tried = if args.flag(&REDUCE) { campaign.reduced(tried, trials) } else { tried };
        let finding = match (&tried.result, &tried.divergence) {
            (Err(finding), _) => Cause::Finding(finding),
            (Ok(_), Some(divergence)) => Cause::Divergence(divergence),
            (Ok(_), None) => {
                if let (true, Some(dir)) = (args.flag(&ALL), &out) {
                    dir.join(seed.to_string()).join("program.vun").put(&tried.source)?;
                }
                continue;
            }
        };
        summary.found += 1;
        let kept = out.as_ref().map(|dir| dir.join(seed.to_string()).join("program.vun"));
        if let Some(path) = &kept {
            path.put(&tried.source)?;
        }
        shell.result(&Found { tried: &tried, finding, kept })?;
    }
    shell.status(&summary);
    Ok(if summary.found == 0 { Outcome::Success } else { Outcome::Failure })
}
