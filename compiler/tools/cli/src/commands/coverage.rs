//! `vun cover [path] [names]... [--lcov file] [--functions]`: runs the tests of a namespace in a
//! program built covered, as `vun test` runs them, one process for all, and reports which lines and
//! functions of the code below the path they reached (spec 11, section 6).

use std::io;
use std::path::Path;

use vun_command::{Args, Command, CommandVerb, Example, Outcome, Param, PathKind, Values};
use vun_diag::{Diagnostic, Msg, errors};
use vun_driver::Session;
use vun_driver::cover::{Lcov, Shared};
use vun_driver::subject::Subject;
use vun_driver::testing::{Mode, Reported};
use vun_fs::Edit;
use vun_hir::db::Namespace;
use vun_sema::cover::Coverage;
use vun_span::FileId;
use vun_term::Shell;

use super::test::{NAMES, tests};
use super::{COMPILE, PATH};

const LCOV: Param = Param::option("lcov")
    .about("write what the tests reached as an lcov tracefile")
    .values(Values::Path(PathKind::File));
const FUNCTIONS: Param = Param::flag("functions").about("list each function and whether the tests called it");

pub(super) const COVER: Command<Session> =
    Command::run(CommandVerb::Cover, "run the tests of a namespace and report the lines they reach", cover)
        .params(&[
            PATH,
            NAMES,
            vun_driver::testing::FILTER,
            vun_driver::testing::SEED,
            vun_driver::testing::CASES,
            LCOV,
            FUNCTIONS,
            vun_driver::codegen::BACKEND,
            vun_driver::codegen::LINKER,
            vun_driver::program::OUTPUT,
        ])
        .sets(&COMPILE)
        .examples(&[
            Example { line: "", about: "the lines the tests of the package around this directory reach" },
            Example { line: "--functions", about: "also each function, reached or not" },
            Example { line: "--lcov cover.info", about: "write an lcov tracefile for other tools" },
        ]);

fn cover(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    session.cover(Coverage::Blocks);
    tests(session, args, shell, Mode::Cover)
}

/// Reports what the run of the tests of `ns`, built from `file`, reached of the code below
/// `scope`.
pub(super) fn report(
    session: &mut Session,
    args: &Args,
    shell: &mut Shell<'_>,
    (file, ns): (FileId, &Namespace),
    (reported, subject, scope): (&Reported, &Subject, &Path),
) -> io::Result<()> {
    let program = session.tests_program(file, ns.clone());
    let (Some(covered), Some(program)) = (&reported.covered, program) else {
        let msg = Msg::new().text("the tests' program wrote no coverage");
        session.diag.error(Diagnostic::error(errors::TEST_NO_COVERAGE, msg));
        return Ok(());
    };
    let files = session.files(subject);
    let reach = session.reached(&program, covered, scope, &files);
    shell.table(&reach.file_rows())?;
    if reach.shared > 0 {
        shell.status(&Shared { blocks: reach.shared });
    }
    if args.flag(&FUNCTIONS) {
        shell.table(&reach.function_rows())?;
    }
    if let Some(path) = args.value(&LCOV) {
        Path::new(path).put(Lcov(&reach).to_string())?;
    }
    Ok(())
}
