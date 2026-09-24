//! `cargo xtask corpus`: runs `corpus/accept` and `corpus/reject` through the driver's stages and
//! compares what they report with what the files expect (spec 10, section 7).
//!
//! A rejected file whose missing diagnostics all belong to stages that do not exist yet, or to
//! stages reading MIR in functions MIR does not lower yet, is pending, not failing, so the corpus
//! can be written ahead of the compiler (roadmap phase 0).

use overt_fs::Tree;
use std::collections::BTreeSet;
use std::fmt::Write;
use std::path::{Path, PathBuf};

use overt_diag::{ChildKind, DiagId, Diagnostic, errors};
use overt_driver::stages::{Input, Stage};
use overt_driver::{Coverage, Session};
use overt_harness::{self as harness, At, Expected, Mismatch, Reported};
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

/// The corpus and the explanation pages of the repository at `root`.
pub(crate) struct Corpus<'r> {
    pub(crate) root: &'r Path,
}

impl Corpus<'_> {
    /// Runs every corpus file, and the examples of the explanation pages.
    ///
    /// # Errors
    /// Returns an error if a corpus directory or file cannot be read.
    pub(crate) fn outcomes(&self) -> std::io::Result<Vec<Outcome>> {
        let mut outcomes = self.explanations()?;
        for &(dir, expect) in SUITES {
            for path in self.root.join(dir).files_with(EXTENSION)? {
                let verdict = Verdict::of(&path, expect)?;
                outcomes.push(Outcome {
                    path: path.strip_prefix(self.root).unwrap_or(&path).to_path_buf(),
                    verdict,
                });
            }
        }
        Ok(outcomes)
    }

    /// How much of the programs that must compile the stages that exist understand.
    ///
    /// # Errors
    /// Returns an error if the directory cannot be read.
    pub(crate) fn coverage(&self) -> std::io::Result<Coverage> {
        let mut total = Coverage::default();
        for &(dir, expect) in SUITES {
            if expect != Expect::Clean {
                continue;
            }
            for path in self.root.join(dir).files_with(EXTENSION)? {
                let mut session = Session::new();
                if let Some(file) = session.load(&path) {
                    session.check(file);
                    total += session.coverage(file);
                }
            }
        }
        Ok(total)
    }
}

impl Verdict {
    /// The verdict of the file at `path`, expected to report nothing or exactly its annotations.
    ///
    /// # Errors
    /// Returns an error if the file cannot be read.
    pub(crate) fn of(path: &Path, expect: Expect) -> std::io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let mut session = Session::new();
        let mut unlowered = Vec::new();
        if let Some(file) = session.load(path) {
            session.check(file);
            unlowered = session.unlowered(file);
        }
        let reported: Vec<Reported> = session.diag.sorted().iter().map(|d| reported(&session, d)).collect();
        let expected = match expect {
            Expect::Clean => Vec::new(),
            Expect::Annotated => match Expected::all(&text) {
                Ok(expected) => expected,
                Err(malformed) => return Ok(Self::Malformed(malformed)),
            },
        };
        let undeclared: BTreeSet<At> = expected
            .iter()
            .map(|e| &e.at)
            .filter(|at| !errors::ALL.contains(&DiagId::compiler(&at.id)))
            .cloned()
            .collect();
        if !undeclared.is_empty() {
            return Ok(Self::Undeclared(undeclared));
        }
        let mismatch = Mismatch::between(&expected, &reported);
        Ok(if mismatch.is_empty() {
            Self::Pass
        } else if mismatch.unexpected.is_empty()
            && mismatch.differing.is_empty()
            && mismatch.missing.iter().all(|at| Self::later(at, &unlowered))
        {
            Self::Pending(mismatch.missing)
        } else {
            Self::Fail(mismatch)
        })
    }
}

impl Verdict {
    /// Whether the diagnostic expected at `at` belongs to a stage that does not exist yet, or to
    /// one reading MIR in a function MIR does not lower yet (lines `unlowered`).
    fn later(at: &At, unlowered: &[(u32, u32)]) -> bool {
        match Stage::of(&DiagId::compiler(&at.id)) {
            None => true,
            Some(stage) => {
                stage.input == Input::Mir && unlowered.iter().any(|&(lo, hi)| (lo..=hi).contains(&at.line))
            }
        }
    }
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
