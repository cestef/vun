//! Views: each representation of a file as text, with origins, as `vun debug` prints it and the
//! language server shows it. A view is one row of [`View`]: its name, what it shows, and what it
//! is a view of, which says the files it applies to.

mod focus;
mod front;
mod mid;
mod sema;

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use vun_data::{FromData, ToData};
use vun_diag::{Diagnostic, Msg, errors};
use vun_fs::Tree;
use vun_span::{FileId, Place, SourceMap as _};
use vun_variants::Variants;

use self::focus::Focus;
use crate::Session;

/// The directory of the work directory views are written in.
const VIEWS: &str = "views";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Variants, ToData, FromData)]
pub enum View {
    Tokens,
    Cst,
    Manifest,
    Plan,
    Hir,
    Types,
    Values,
    Mir,
    Layout,
    Program,
    Lir,
    Code,
    Frames,
}

/// How much of a file a view shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ToData, FromData)]
pub enum Scope {
    /// Everything the view shows of the file.
    File,
    /// What the unit of the view ([`View::unit`]) holding the byte `offset` has; a view that has
    /// no unit shows the file.
    At { offset: u32 },
}

/// The part of a file a view can be narrowed to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Function,
    Declaration,
}

impl Unit {
    /// Its name, which a narrowed view is titled and named by.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Declaration => "declaration",
        }
    }
}

/// What a view is a view of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Subject {
    /// Any file.
    File,
    /// A package's `build.vun`.
    Package,
    /// The program a root file exports, which compiling it builds.
    Program,
}

impl View {
    /// Its name, which `vun debug` names its command by.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Tokens => "tokens",
            Self::Cst => "cst",
            Self::Manifest => "manifest",
            Self::Plan => "plan",
            Self::Hir => "hir",
            Self::Types => "types",
            Self::Values => "values",
            Self::Mir => "mir",
            Self::Layout => "layout",
            Self::Program => "program",
            Self::Lir => "lir",
            Self::Code => "code",
            Self::Frames => "frames",
        }
    }

    /// What it shows.
    #[must_use]
    pub const fn about(self) -> &'static str {
        match self {
            Self::Tokens => "tokens and trivia with byte ranges",
            Self::Cst => "the concrete syntax tree, one node per line",
            Self::Manifest => "the package manifest read from a build.vun",
            Self::Plan => "the steps a build.vun's build records, one per line",
            Self::Hir => "the high-level representation, names resolved, with origins",
            Self::Types => "what typing found in each function, plain and in each instantiation reached",
            Self::Values => "the value and type of each declaration, as compile time computes them",
            Self::Mir => "the mid-level representation of each function, with origins",
            Self::Layout => "the layout of each type the file declares: sizes, offsets, tags and niches",
            Self::Program => "the exports of a root file and the MIR of every function they reach",
            Self::Lir => "the low-level representation of the program a root file exports, with origins",
            Self::Code => "what a backend generates for the program a root file exports, when it is text",
            Self::Frames => {
                "the frame size of each function of the program a root file exports, as its tools report them"
            }
        }
    }

    /// What it can be narrowed to, if it shows one thing per unit of the file.
    #[must_use]
    pub const fn unit(self) -> Option<Unit> {
        match self {
            Self::Types | Self::Mir => Some(Unit::Function),
            Self::Values => Some(Unit::Declaration),
            _ => None,
        }
    }

    #[must_use]
    pub const fn subject(self) -> Subject {
        match self {
            Self::Manifest | Self::Plan => Subject::Package,
            Self::Program | Self::Lir | Self::Code | Self::Frames => Subject::Program,
            Self::Tokens | Self::Cst | Self::Hir | Self::Types | Self::Values | Self::Mir | Self::Layout => {
                Subject::File
            }
        }
    }
}

impl Session {
    /// The scope of the unit holding `place` in `file`; reports a place the file lacks.
    pub fn scope_at(&mut self, file: FileId, place: Place) -> Option<Scope> {
        let offset = self.sources.file(file).offset(place);
        if offset.is_none() {
            let msg = Msg::new().code(place.to_string()).text(" is past the end of the file");
            self.diag.error(Diagnostic::error(errors::CLI_INVALID_VALUE, msg));
        }
        offset.map(|offset| Scope::At { offset })
    }

    /// The views that apply to `file`: every view of a file, those of a package when it is a
    /// package's `build.vun`, and those of a program when it exports one.
    pub fn views(&mut self, file: FileId) -> Vec<View> {
        let package = self.is_package(file);
        let program = !self.program(file).exports.is_empty();
        let applies = |subject| match subject {
            Subject::File => true,
            Subject::Package => package,
            Subject::Program => program,
        };
        View::ALL.iter().copied().filter(|v| applies(v.subject())).collect()
    }

    /// Writes the view `view` of `file`, as much of it as `scope` says, to `out`; what computing
    /// it reports is recorded. A scope no unit holds shows nothing.
    ///
    /// # Errors
    /// Returns an error if `out` cannot be written.
    pub fn view(&mut self, view: View, file: FileId, scope: Scope, out: &mut dyn Write) -> io::Result<()> {
        let focus = match (scope, view.unit()) {
            (Scope::At { offset }, Some(unit)) => match self.unit_at(unit, file, offset) {
                Some(focus) => Some(focus),
                None => return Ok(()),
            },
            _ => None,
        };
        match view {
            View::Tokens => self.view_tokens(file, out),
            View::Cst => self.view_cst(file, out),
            View::Manifest => self.view_manifest(file, out),
            View::Plan => self.view_plan(file, out),
            View::Hir => self.view_hir(file, out),
            View::Types => self.view_types(file, focus.as_ref().and_then(Focus::expr), out),
            View::Values => self.view_values(file, focus.as_ref().and_then(Focus::decl), out),
            View::Mir => self.view_mir(file, focus.as_ref().and_then(Focus::expr), out),
            View::Layout => self.view_layout(file, out),
            View::Program => self.view_program(file, out),
            View::Lir => self.view_lir(file, out),
            View::Code => self.dump_code(file, out),
            View::Frames => self.dump_frames(file, out),
        }
    }
}

impl Session {
    /// Where what an editor shows of the file at `source` is written, as `shown` names it (a
    /// view's name, or the output of a run): in its project's work directory, at the file's path
    /// in the project, so that files of one name in two directories do not meet.
    pub fn shown_at(&mut self, source: &Path, shown: &str) -> PathBuf {
        let project = self.project_dir(source).resolved();
        let file = source.resolved();
        let relative = match (&file, &project) {
            (Ok(file), Ok(project)) => file.strip_prefix(project).ok(),
            _ => None,
        };
        let relative =
            relative.map_or_else(|| PathBuf::from(source.file_name().unwrap_or_default()), Path::to_path_buf);
        let mut name = relative.into_os_string();
        name.push(".");
        name.push(shown);
        self.work(source).join(VIEWS).join(name)
    }
}
