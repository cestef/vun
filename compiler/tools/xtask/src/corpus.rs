//! `cargo xtask corpus`: runs `corpus/accept` and `corpus/reject` through the driver's stages and
//! compares what they report with what the files expect (spec 10, section 7).
//!
//! A rejected file whose missing diagnostics all belong to stages that do not exist yet is
//! pending, not failing, so the corpus can be written ahead of the compiler (roadmap phase 0).

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fmt::Write;
use std::path::{Path, PathBuf};

use overt_diag::{ChildKind, DiagId, Diagnostic, errors};
use overt_driver::Session;
use overt_driver::stages::implemented;
use overt_harness::{self as harness, At, Mismatch, Reported, compare, discover, expectations};
use overt_span::{EXTENSION, SourceMap};

/// Where the corpus lives, relative to the repository root, and what its files expect.
pub(crate) const SUITES: &[(&str, Expect)] =
    &[("corpus/accept", Expect::Clean), ("corpus/reject", Expect::Annotated)];

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Expect {
    /// No diagnostics.
    Clean,
    /// Exactly the `//~` annotations.
    Annotated,
}

pub(crate) enum Verdict {
    Pass,
    /// Expected diagnostics from stages that do not exist yet.
    Pending(BTreeSet<At>),
    Fail(Mismatch),
    /// Annotations naming ids that `lang.errors` does not declare.
    Undeclared(BTreeSet<At>),
    Malformed(harness::Malformed),
    /// An example shown as a command transcript, which no file check covers.
    Shown,
    /// A diagnostic whose explanation page is missing or does not show it.
    Unexplained(crate::explanations::Unexplained),
}

pub(crate) struct Outcome {
    pub(crate) path: PathBuf,
    pub(crate) verdict: Verdict,
}

/// Runs every corpus file under `root`, and the examples of the explanation pages.
///
/// # Errors
/// Returns an error if a corpus directory or file cannot be read.
pub(crate) fn run(root: &Path) -> std::io::Result<Vec<Outcome>> {
    let mut outcomes = crate::explanations::run(root)?;
    for &(dir, expect) in SUITES {
        for path in discover(&root.join(dir), EXTENSION)? {
            let verdict = check(&path, expect)?;
            outcomes.push(Outcome { path: path.strip_prefix(root).unwrap_or(&path).to_path_buf(), verdict });
        }
    }
    Ok(outcomes)
}

/// The verdict of the file at `path`, expected to report nothing or exactly its annotations.
///
/// # Errors
/// Returns an error if the file cannot be read.
pub(crate) fn check(path: &Path, expect: Expect) -> std::io::Result<Verdict> {
    let text = std::fs::read_to_string(path)?;
    let mut session = Session::new();
    if let Some(file) = session.load(path) {
        session.check(file);
    }
    let reported: Vec<Reported> = session.diag.sorted().iter().map(|d| reported(&session, d)).collect();
    let expected = match expect {
        Expect::Clean => Vec::new(),
        Expect::Annotated => match expectations(&text) {
            Ok(expected) => expected,
            Err(malformed) => return Ok(Verdict::Malformed(malformed)),
        },
    };
    let undeclared: BTreeSet<At> = expected
        .iter()
        .map(|e| &e.at)
        .filter(|at| !errors::ALL.contains(&compiler_id(&at.id)))
        .cloned()
        .collect();
    if !undeclared.is_empty() {
        return Ok(Verdict::Undeclared(undeclared));
    }
    let mismatch = compare(&expected, &reported);
    Ok(if mismatch.is_empty() {
        Verdict::Pass
    } else if mismatch.unexpected.is_empty()
        && mismatch.differing.is_empty()
        && mismatch.missing.iter().all(|at| !implemented(&compiler_id(&at.id)))
    {
        Verdict::Pending(mismatch.missing)
    } else {
        Verdict::Fail(mismatch)
    })
}

/// A diagnostic as the harness compares it: position, id, and message with its children.
fn reported(session: &Session, d: &Diagnostic) -> Reported {
    let (line, col) = d.primary.as_ref().map_or((0, 0), |label| {
        let span = session.origins.source_span(label.origin);
        let at = session.sources.file(span.file).line_col(span.range.lo);
        (at.line, at.col)
    });
    let mut text = d.message.to_string();
    for child in &d.children {
        let _ = write!(text, "\n{}: {}", child.kind, child.msg);
    }
    for fix in &d.fixes {
        let _ = write!(text, "\n{}: {}", ChildKind::Help, fix.msg);
    }
    Reported { at: At { line, id: d.id.to_string() }, col, text }
}

/// The full id of an annotation, which is written relative to the compiler's errors.
fn compiler_id(relative: &str) -> DiagId {
    DiagId(Cow::Owned(format!("{}{relative}", DiagId::COMPILER)))
}
