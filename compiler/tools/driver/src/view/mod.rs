//! Views: each representation of a file as text, with origins, as `vun debug` prints it and the
//! language server shows it. A view is one row of [`View`]: its name, what it shows, and what it
//! is a view of, which says the files it applies to.

mod front;
mod mid;
mod sema;

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use vun_data::{FromData, ToData};
use vun_fs::Tree;
use vun_span::FileId;
use vun_variants::Variants;

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

    /// Writes the view `view` of `file` to `out`; what computing it reports is recorded.
    ///
    /// # Errors
    /// Returns an error if `out` cannot be written.
    pub fn view(&mut self, view: View, file: FileId, out: &mut dyn Write) -> io::Result<()> {
        match view {
            View::Tokens => self.view_tokens(file, out),
            View::Cst => self.view_cst(file, out),
            View::Manifest => self.view_manifest(file, out),
            View::Plan => self.view_plan(file, out),
            View::Hir => self.view_hir(file, out),
            View::Types => self.view_types(file, out),
            View::Values => self.view_values(file, out),
            View::Mir => self.view_mir(file, out),
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
