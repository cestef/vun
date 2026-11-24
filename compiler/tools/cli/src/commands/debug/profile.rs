//! `vun debug profile`: a compile measured so that two runs compare. Instructions are counted
//! under callgrind, which gives the same count for the same single-threaded run and ranks the
//! functions and crates that spend them; time is sampled from the compiler's own `--timings`
//! and ranked by stage and query, with its spread. Either is kept in a snapshot and set against
//! it by a later run.

use std::io;
use std::path::{Path, PathBuf};

use vun_command::{Args, Outcome, Param, PathKind, Values};
use vun_diag::{Diagnostic, Msg, errors};
use vun_driver::Session;
use vun_driver::testing::Measure;
use vun_profile::{Phase, Runner, Snapshot, Study, Workload};
use vun_term::{Interval, Percent, Shell, TimeUnit};

/// Samples of a timed run unless `--samples` asks for another count.
const SAMPLED: usize = 7;
/// Rows of functions and activities unless `--top` asks for another count.
const TOP: usize = 25;
/// The functions a snapshot keeps.
const KEPT: usize = 300;
/// The change of a count of instructions that is more than the variation between two runs.
const TOLERANCE: Percent = Percent(0.5);
/// The snapshot `--snapshot` names unless it names another.
const DEFAULT_SNAPSHOT: &str = "base";
/// How long a run may last unless `--limit` says (callgrind runs a program some fifty times slower).
const LIMIT: Interval = Interval::new(15, TimeUnit::Minutes);

const WORKLOADS: Param = Param::positional("workloads")
    .arity(vun_command::Arity::Many)
    .about("the workloads to measure; the light ones if none")
    .values(Values::Choice(Workload::names));
const MEASURE: Param = Param::option("measure")
    .about(
        "`instructions` counted under callgrind (the default), or wall-clock `time` sampled from `--timings`",
    )
    .values(Values::Choice(|| vec![Measure::Instructions.name(), Measure::Time.name()]));
const PHASE: Param = Param::option("phase")
    .about("what the compiler has saved when it starts: `cold` or `warm`; both if not given. Programs have neither")
    .values(Values::Choice(Phase::names));
const SAMPLES: Param = Param::option("samples")
    .about("how many timed runs to take samples from (7); instructions are one run")
    .values(Values::Count);
const TOP_ROWS: Param =
    Param::option("top").about("how many functions or activities to list (25)").values(Values::Count);
const THREADS: Param = Param::option("threads")
    .about("how many threads the compiler uses; one when counting instructions, the machine's when timing")
    .values(Values::Count);
const BACKEND: Param = Param::option("backend")
    .about("the backend generating the code of the `run-` workloads' programs; the target's if not given")
    .values(Values::Text);
const CALLERS: Param =
    Param::flag("callers").about("list what calls each function listed, with the cost of the calls");
const EXE: Param = Param::option("exe")
    .about("the compiler to measure; this one if not given. Built with debug information it names its functions and crates")
    .values(Values::Path(PathKind::File));
const ROOT: Param = Param::option("root")
    .about("the repository the workloads are in, and snapshots are kept in (the working directory)")
    .values(Values::Path(PathKind::Dir));
const SNAPSHOT: Param = Param::option("snapshot")
    .about("the snapshot to compare with, and to save as; `base` if not given")
    .values(Values::Text);
const SAVE: Param = Param::flag("save").about("keep the results as the snapshot");
const FAIL_OVER: Param = Param::option("fail-over")
    .about("fail when a quantity grew by more than this, and by more than the noise, over the snapshot")
    .values(Values::Percent);
const LIMIT_TIME: Param = Param::option("limit")
    .about("how long one run of the compiler may last, such as `90s` or `15m` (15m)")
    .values(Values::Interval);

pub(super) const PARAMS: &[Param] = &[
    WORKLOADS, MEASURE, PHASE, SAMPLES, TOP_ROWS, THREADS, BACKEND, CALLERS, EXE, ROOT, SNAPSHOT, SAVE,
    FAIL_OVER, LIMIT_TIME,
];

fn count(args: &Args, param: &Param) -> Option<usize> {
    args.count(param).and_then(|n| usize::try_from(n).ok())
}

pub(super) fn profile(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let failed = |session: &mut Session, msg: Msg, cause: &(dyn std::error::Error + 'static)| {
        session.diag.error(Diagnostic::error(errors::TOOL_PROFILE_FAILED, msg).caused_by(cause));
        Ok(Outcome::Failure)
    };
    let root = PathBuf::from(args.value(&ROOT).unwrap_or("."));
    let exe = match args.value(&EXE) {
        Some(exe) => PathBuf::from(exe),
        None => std::env::current_exe()?,
    };
    let measure = match args.value(&MEASURE) {
        Some(name) => Measure::named(name).unwrap_or_default(),
        None => Measure::Instructions,
    };
    let workloads: Vec<&Workload> =
        match args.values(&WORKLOADS).map(Workload::named).collect::<Option<Vec<_>>>() {
            Some(named) if !named.is_empty() => named,
            _ => Workload::defaults().collect(),
        };
    let phase = args.value(&PHASE).and_then(Phase::named);
    let name = args.value(&SNAPSHOT).unwrap_or(DEFAULT_SNAPSHOT);
    let baseline = match Snapshot::read(&root, name) {
        Ok(baseline) => baseline,
        Err(e) => {
            return failed(session, Msg::new().text("cannot read the snapshot ").code(name.to_owned()), &e);
        }
    };
    let threads = count(args, &THREADS).or((measure == Measure::Instructions).then_some(1));
    let limit = args.interval(&LIMIT_TIME).unwrap_or(LIMIT).into();
    let study = Study {
        runner: Runner { exe: &exe, root: &root, threads, backend: args.value(&BACKEND), limit },
        measure,
        samples: count(args, &SAMPLES).map_or(SAMPLED, |n| n.max(1)),
        top: count(args, &TOP_ROWS).unwrap_or(TOP),
        keep: KEPT,
        callers: args.flag(&CALLERS),
        baseline,
        tolerance: TOLERANCE,
        fail_over: args.percent(&FAIL_OVER),
    };
    let (mut kept, mut regressed) = (Vec::new(), false);
    for workload in workloads {
        for phase in workload.phases(phase) {
            match study.run(workload, phase, shell) {
                Ok(studied) => {
                    regressed |= studied.regressed;
                    kept.extend(studied.entries);
                }
                Err(e) => {
                    return failed(
                        session,
                        Msg::new().text("cannot measure ").code(workload.name.to_owned()),
                        &e,
                    );
                }
            }
        }
    }
    if args.flag(&SAVE)
        && let Err(e) = Snapshot::write(kept, Path::new(&root), name)
    {
        return failed(session, Msg::new().text("cannot write the snapshot ").code(name.to_owned()), &e);
    }
    Ok(if regressed { Outcome::Failure } else { Outcome::Success })
}
