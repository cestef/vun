//! Reducing a file to the least text that fails as it does (`vun debug reduce`, spec 13,
//! section 10). Each candidate is compiled in this process as the file it stands for, through
//! every check and the backend's code generation, over the same queries: the library is compiled
//! once, and each candidate recomputes what its text reaches.

use std::panic::{self, AssertUnwindSafe, PanicHookInfo};
use std::path::Path;
use std::sync::Mutex;

use vun_diag::{DiagCtxt, DiagId, Diagnostic, ErrorGuaranteed, Msg, Severity, errors};
use vun_reduce::{Reduced, Uninteresting};
use vun_span::{FileId, FileRef, SourceMap};
use vun_trace::tracing;

use crate::Session;
use crate::codegen::Listed;

/// How compiling a text fails.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Failure {
    /// The compiler panicked at this place of its own source.
    Crash(Site),
    /// The text reports this error.
    Report(DiagId),
}

/// A place in the compiler's source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Site {
    file: String,
    line: u32,
}

/// Where the last panic was, as the hook installed while reducing records it.
static PANICKED: Mutex<Option<Site>> = Mutex::new(None);

/// The panic hook while candidates are compiled: a crash is recorded, not printed. The hook it
/// replaced is restored when this is dropped.
struct Recording(Option<Hook>);

/// A panic hook, as `std::panic` holds it.
type Hook = Box<dyn Fn(&PanicHookInfo<'_>) + Sync + Send>;

impl Recording {
    fn install() -> Self {
        let previous = panic::take_hook();
        panic::set_hook(Box::new(|info| {
            let site = info.location().map(|at| Site { file: at.file().to_owned(), line: at.line() });
            *PANICKED.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = site;
        }));
        Self(Some(previous))
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        if let Some(previous) = self.0.take() {
            panic::set_hook(previous);
        }
    }
}

impl Session {
    /// The file `file` reduced to the least text that fails as it does: with the error `keep`
    /// when one is named, else with its first failure.
    ///
    /// # Errors
    /// Returns once it is reported that the file does not fail as asked.
    pub fn reduce(&mut self, file: FileId, keep: Option<DiagId>) -> Result<Reduced, ErrorGuaranteed> {
        let path = self.sources.file(file).path.clone();
        let key = self.file_key(Path::new(&path));
        let text = self.text(file).to_owned();
        let recording = Recording::install();
        let first = self.failures(&key, &path, &text).into_iter().next();
        let kept = keep.map(Failure::Report).or(first);
        let reduced = kept.as_ref().ok_or(Uninteresting).and_then(|kept| {
            tracing::info!(?kept, "reducing");
            let mut interest = |text: &str| self.failures(&key, &path, text).contains(kept);
            Reduced::of(&text, &mut interest)
        });
        drop(recording);
        self.diag = DiagCtxt::default();
        reduced.map_err(|_| {
            let asked = kept.map_or_else(
                || Msg::new().text(" does not fail"),
                |kept| Msg::new().text(" does not fail as asked: ").append(kept.msg()),
            );
            let msg = Msg::new().code(path).append(asked);
            self.diag.error(Diagnostic::error(errors::TOOL_NOTHING_TO_REDUCE, msg))
        })
    }

    /// How `text`, standing for the file `key` at `path`, fails: its crash, else the errors it
    /// reports, in order.
    fn failures(&mut self, key: &FileRef, path: &str, text: &str) -> Vec<Failure> {
        self.diag = DiagCtxt::default();
        let file = self.sources.add_as(key.clone(), path, text);
        let ran = panic::catch_unwind(AssertUnwindSafe(|| {
            self.check(file);
            let _ = self.code(file, Listed::No);
        }));
        if ran.is_err() {
            let site = PANICKED.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
            return site.map(Failure::Crash).into_iter().collect();
        }
        let errors = self.diag.sorted(self.origins).into_iter().filter(|d| d.severity == Severity::Error);
        errors.map(|d| Failure::Report(d.id.clone())).collect()
    }
}

impl Failure {
    fn msg(&self) -> Msg {
        match self {
            Self::Crash(site) => Msg::new().text("a crash at ").code(format!("{}:{}", site.file, site.line)),
            Self::Report(id) => Msg::new().text("reporting ").code(id.to_string()),
        }
    }
}
