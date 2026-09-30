//! `vun fix` (spec 10, section 6): the fixes the diagnostics of a check propose, applied to the
//! files of the package checked and checked again, to a fixed point of at most [`ROUNDS`] rounds.
//! Each diagnostic gives its preferred fix of the safety asked for, or else its first; a fix whose
//! edits overlap one taken before waits for the next round. A file with more errors after a round
//! than before gets back its text of before, and fixing ends.

use std::collections::BTreeMap;
use std::path::PathBuf;

use vun_diag::{DiagCtxt, Edit, Fix, Safety};
use vun_origin::Resolve;
use vun_span::{FileId, SourceMap};
use vun_trace::tracing;

use crate::Session;
use crate::packages::PackageKey;

/// How many times fixes are applied and the files checked again, at most.
pub const ROUNDS: usize = 4;

/// Which fixes are applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applied {
    /// Those preserving what the program means: [`Safety::Safe`].
    Safe,
    /// Those that may change it as well: [`Safety::Unsafe`].
    Unsafe,
}

/// A file fixes changed: its path, how many fixes it took, and its text now.
pub struct Fixed {
    pub path: PathBuf,
    pub fixes: usize,
    pub text: String,
}

impl Applied {
    fn admits(self, safety: Safety) -> bool {
        match safety {
            Safety::Safe => true,
            Safety::Unsafe => self == Self::Unsafe,
            Safety::Placeholder | Safety::DisplayOnly => false,
        }
    }
}

/// The edits taken for one file in a round, and how many fixes they make.
#[derive(Default)]
struct Taken {
    edits: Vec<Edit>,
    fixes: usize,
}

/// A file fixing changed: its text before, the version the session reads now, and the fixes
/// kept.
struct Change {
    original: String,
    file: FileId,
    fixes: usize,
}

impl Session {
    /// Applies the fixes `applied` admits to the package of `root` until none is left, and
    /// returns the files changed; the session reads each at its fixed text, and the diagnostics
    /// of the last check are left in it.
    pub fn fix(&mut self, mut root: FileId, applied: Applied) -> Vec<Fixed> {
        let package = self.package_of(root);
        let mut files: BTreeMap<PathBuf, Change> = BTreeMap::new();
        let mut reported = self.checked(root);
        for round in 0..ROUNDS {
            let taken = self.taken(&reported, applied, package);
            if taken.is_empty() {
                break;
            }
            let mut edited = Vec::new();
            for (file, taken) in taken {
                let text = self.text(file).to_owned();
                let Ok(fixed) = Edit::apply(&taken.edits, &text) else { continue };
                let path = self.path_of(file);
                let opened = self.reopen(&mut root, file, &path, fixed);
                let entry =
                    files.entry(path.clone()).or_insert(Change { original: text.clone(), file, fixes: 0 });
                (entry.file, entry.fixes) = (opened, entry.fixes + taken.fixes);
                edited.push((path, text, taken.fixes));
            }
            let errors = self.errors_by_path(&reported);
            reported = self.checked(root);
            let now = self.errors_by_path(&reported);
            let worse: Vec<_> =
                edited.into_iter().filter(|(path, ..)| now.get(path) > errors.get(path)).collect();
            if worse.is_empty() {
                continue;
            }
            // Undone, and fixing ends: another round would take the same fixes.
            for (path, text, fixes) in worse {
                tracing::info!(path = %path.display(), round, "fixes undone: they add errors");
                if let Some(entry) = files.get_mut(&path) {
                    let reopened = self.reopen(&mut root, entry.file, &path, text);
                    (entry.file, entry.fixes) = (reopened, entry.fixes - fixes);
                }
            }
            reported = self.checked(root);
            break;
        }
        self.diag = reported;
        let changed = files.into_iter().filter_map(|(path, change)| {
            let text = self.text(change.file).to_owned();
            (text != change.original).then_some(Fixed { path, fixes: change.fixes, text })
        });
        changed.collect()
    }

    /// Reads `text` as a new version of `file` at `path`, which `root` follows when it is that
    /// file; returns the new version.
    fn reopen(&mut self, root: &mut FileId, file: FileId, path: &std::path::Path, text: String) -> FileId {
        let opened = self.open(path, text);
        if *root == file {
            *root = opened;
        }
        opened
    }

    fn path_of(&self, file: FileId) -> PathBuf {
        PathBuf::from(&self.sources.file(file).path)
    }

    /// What checking `root` reports, taken out of the session.
    fn checked(&mut self, root: FileId) -> DiagCtxt {
        self.check(root);
        std::mem::take(&mut self.diag)
    }

    /// The edits of the fixes `applied` admits among those `reported` proposes, by file: in files
    /// of `package`, of the text the session reads now, none overlapping another.
    fn taken(
        &mut self,
        reported: &DiagCtxt,
        applied: Applied,
        package: PackageKey,
    ) -> BTreeMap<FileId, Taken> {
        let mut taken: BTreeMap<FileId, Taken> = BTreeMap::new();
        for d in reported.sorted(&*self) {
            let fits = |f: &&Fix| applied.admits(f.safety) && !f.edits.is_empty();
            let Some(fix) = d.fixes.iter().filter(fits).find(|f| f.preferred).or(d.fixes.iter().find(fits))
            else {
                continue;
            };
            let current = fix.edits.iter().all(|e| self.current(e.span.file) == e.span.file);
            if !current || !fix.edits.iter().all(|e| self.package_of(e.span.file) == package) {
                continue;
            }
            let overlaps = fix.edits.iter().any(|e| {
                taken
                    .get(&e.span.file)
                    .is_some_and(|t| t.edits.iter().any(|o| o.span.range.overlaps(e.span.range)))
            });
            if overlaps {
                continue;
            }
            for edit in &fix.edits {
                taken.entry(edit.span.file).or_default().edits.push(edit.clone());
            }
            if let Some(first) = fix.edits.first() {
                taken.entry(first.span.file).or_default().fixes += 1;
            }
        }
        taken
    }

    /// How many errors `reported` holds in each file, by path.
    fn errors_by_path(&self, reported: &DiagCtxt) -> BTreeMap<PathBuf, usize> {
        let mut errors = BTreeMap::new();
        for d in reported.sorted(self).into_iter().filter(|d| d.severity == vun_diag::Severity::Error) {
            let Some(primary) = &d.primary else { continue };
            let file = self.source_span(primary.origin).file;
            *errors.entry(self.path_of(file)).or_default() += 1;
        }
        errors
    }
}
