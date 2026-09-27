//! `cargo xtask corpus`: runs `corpus/accept`, `corpus/reject` and `corpus/planned` through the
//! driver's stages and compares what they report with what the files expect (spec 10, section 7).
//!
//! A rejected file whose missing diagnostics all belong to stages that do not exist yet, or to
//! stages reading MIR in functions MIR does not lower yet, is pending, not failing, so the corpus
//! can be written ahead of the compiler (roadmap phase 0).

use std::collections::BTreeSet;
use std::fmt::Write;
use std::path::{Path, PathBuf};

use vun_diag::{ChildKind, DiagId, Diagnostic, errors};
use vun_driver::stages::{Input, Stage};
use vun_driver::{Coverage, Session, Shared};
use vun_fs::Tree;
use vun_harness::{self as harness, ACCEPT, At, Expected, Mismatch, PLANNED, REJECT, Reported};
use vun_par::Parallel;
use vun_span::{EXTENSION, SourceMap};

/// Where the corpus lives, relative to the repository root, and what its files expect.
pub(crate) const SUITES: &[(&str, Expect)] =
    &[(ACCEPT, Expect::Clean), (REJECT, Expect::Annotated), (PLANNED, Expect::Planned)];

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Expect {
    /// No diagnostics.
    Clean,
    /// Exactly the `//~` annotations.
    Annotated,
    /// Only names the libraries do not declare yet, until it checks clean.
    Planned,
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
    /// A planned program, with the names it waits for.
    Planned(Vec<Reported>),
    /// A planned program that checks clean, and belongs with the accepted ones.
    Ready,
}

pub(crate) struct Outcome {
    pub(crate) path: PathBuf,
    pub(crate) verdict: Verdict,
}

/// The corpus and the explanation pages of the repository at `root`.
pub(crate) struct Corpus<'r> {
    pub(crate) root: &'r Path,
    /// The queries files are checked in: what one computes, the others of its package reuse.
    shared: Shared,
}

impl<'r> Corpus<'r> {
    pub(crate) fn at(root: &'r Path) -> Self {
        Self { root, shared: Shared::default() }
    }

    /// A session of its own for the file at `path`, over the queries of its package.
    pub(crate) fn session(&self, path: &Path) -> Session {
        self.shared.session(path)
    }

    /// Runs every corpus file, and the examples of the explanation pages.
    ///
    /// # Errors
    /// Returns an error if a corpus directory or file cannot be read.
    pub(crate) fn outcomes(&self) -> std::io::Result<Vec<Outcome>> {
        let mut outcomes = self.explanations()?;
        let files = self.files(|_| true)?;
        let verdicts = files.par_map(|(path, expect)| {
            let verdict = Verdict::of(self.session(path), path, *expect)?;
            Ok(Outcome { path: path.strip_prefix(self.root).unwrap_or(path).to_path_buf(), verdict })
        });
        outcomes.extend(verdicts.into_iter().collect::<std::io::Result<Vec<_>>>()?);
        Ok(outcomes)
    }

    /// Every corpus file of the suites `wanted` keeps, with what it expects.
    fn files(&self, wanted: impl Fn(Expect) -> bool) -> std::io::Result<Vec<(PathBuf, Expect)>> {
        let mut files = Vec::new();
        for &(dir, expect) in SUITES.iter().filter(|(_, e)| wanted(*e)) {
            files.extend(self.root.join(dir).files_with(EXTENSION)?.into_iter().map(|p| (p, expect)));
        }
        Ok(files)
    }

    /// How much of the programs that must compile the stages that exist understand.
    ///
    /// # Errors
    /// Returns an error if the directory cannot be read.
    pub(crate) fn coverage(&self) -> std::io::Result<Coverage> {
        let files = self.files(|e| e == Expect::Clean)?;
        let covered = files.par_map(|(path, _)| {
            let mut session = self.session(path);
            let file = session.load(path)?;
            session.check(file);
            Some(session.coverage(file))
        });
        let mut total = Coverage::default();
        for coverage in covered.into_iter().flatten() {
            total += coverage;
        }
        Ok(total)
    }
}

impl Verdict {
    /// The verdict of the file at `path` checked in `session`, expected to report nothing or
    /// exactly its annotations.
    ///
    /// # Errors
    /// Returns an error if the file cannot be read.
    pub(crate) fn of(mut session: Session, path: &Path, expect: Expect) -> std::io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let mut unlowered = Vec::new();
        if let Some(file) = session.load(path) {
            session.check(file);
            unlowered = session.unlowered(file);
        }
        let reported: Vec<Reported> =
            session.diag.sorted(session.origins).iter().map(|d| reported(&session, d, path)).collect();
        if expect == Expect::Planned {
            return Ok(if reported.is_empty() {
                Self::Ready
            } else if reported.iter().all(|r| DiagId::compiler(&r.at.id) == errors::NAMES_UNRESOLVED) {
                Self::Planned(reported)
            } else {
                let unexpected = reported.iter().map(|r| r.at.clone()).collect();
                Self::Fail(Mismatch { unexpected, ..Mismatch::default() })
            });
        }
        let expected = match expect {
            Expect::Clean | Expect::Planned => Vec::new(),
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
    /// Whether it fails the corpus.
    pub(crate) const fn is_failure(&self) -> bool {
        match self {
            Self::Pass | Self::Pending(_) | Self::Planned(_) | Self::Shown => false,
            Self::Fail(_) | Self::Undeclared(_) | Self::Malformed(_) | Self::Unexplained(_) | Self::Ready => {
                true
            }
        }
    }

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

/// Where a diagnostic stands that has no place in the checked file: before its first line, where
/// no annotation is, so it is always unexpected.
const ELSEWHERE: (u32, u32) = (0, 0);

/// A diagnostic as the harness compares it: position in the file at `checked`, id, and message
/// with its children. A file is known by its path, as each reading of it adds it anew.
fn reported(session: &Session, d: &Diagnostic, checked: &Path) -> Reported {
    let place = d.primary.as_ref().map(|label| session.origins.source_span(label.origin));
    let file = place.map(|span| (span, session.sources.file(span.file)));
    let (line, col) =
        file.filter(|(_, file)| Path::new(&file.path) == checked).map_or(ELSEWHERE, |(span, file)| {
            let at = file.line_col(span.range.lo);
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
