//! `vun debug coverage`: how far the stages take the functions of a file, and each function
//! literal or function reached they stop at, where and why.

use std::io::{self, Write};
use std::path::Path;

use vun_command::{Args, Outcome};
use vun_mir::Unready;
use vun_origin::Resolve;
use vun_span::SourceMap;

use super::FILE;
use crate::Session;
use crate::coverage::GapKind;

pub(super) fn coverage(session: &mut Session, args: &Args, out: &mut dyn Write) -> io::Result<Outcome> {
    let path = args.value(&FILE).expect("positional parameters are present when a command runs");
    let Some(file) = session.load(Path::new(path)) else { return Ok(session.outcome()) };
    session.check(file);
    let coverage = session.coverage(file);
    writeln!(out, "{coverage}")?;
    let locate = |o| session.sources.locate(session.source_span(o)).to_string();
    for gap in &coverage.gaps {
        let (stage, kind) = gap.kind.stage();
        write!(out, "{}: {stage} {kind}", locate(gap.function))?;
        match &gap.kind {
            GapKind::Unreached => writeln!(out)?,
            GapKind::Unready { unready: Unready::Unbuilt(unbuilt), .. } => {
                writeln!(out, " at {}, by {}", locate(unbuilt.at), unbuilt.by)?;
            }
            GapKind::Unready { unready, .. } => writeln!(out, " at {}: {unready}", locate(unready.at()))?,
            GapKind::Unlowered(unlowered) => writeln!(out, " at {}: {unlowered}", locate(unlowered.at))?,
        }
    }
    Ok(if coverage.gaps.is_empty() { Outcome::Success } else { Outcome::Failure })
}
