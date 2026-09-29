//! `cargo xtask typeset`: the Typst documents of the repository, compiled with the repository as
//! their root. They highlight Vun with the generated editor syntax, which `cargo gen` keeps
//! current. A document with variants declares them itself (see [`Variant`]), and is compiled once
//! per variant.

mod error;
mod variant;

use std::io;
use std::path::{Path, PathBuf};

use vun_command::{Args, Outcome, Shell};
use vun_data::Object;
use vun_diag::{DiagCtxt, DiagId, Diagnostic, Msg};
use vun_fs::Edit;
use vun_harness::repo_root;
use vun_intern::Symbol;
use vun_par::Parallel;
use vun_term::{Record, Status, Style, Text, Verb};
use vun_tool::{Arg, Executable, Invocation, Tool};

use crate::task::Program;

use error::{Failed, Unread};
use variant::Variant;

pub(crate) struct Document {
    /// The Typst file, relative to the repository root.
    pub(crate) source: &'static str,
    pub(crate) outputs: Outputs,
    pub(crate) format: Format,
}

/// Where a document's files go, relative to the repository root and without their extension.
pub(crate) enum Outputs {
    /// One file.
    File(&'static str),
    /// A file per variant, at the variant's path in this directory.
    Variants(&'static str),
}

#[derive(Clone, Copy)]
pub(crate) enum Format {
    Pdf,
    /// An image rasterized at this many pixels per inch.
    Png {
        ppi: u16,
    },
}

/// The cards are drawn in points at 72 pixels per inch, so a point is a pixel.
const PIXEL: Format = Format::Png { ppi: 72 };

pub(crate) const ALL: &[Document] = &[
    Document {
        source: "whitepaper/main.typ",
        outputs: Outputs::File("whitepaper/main"),
        format: Format::Pdf,
    },
    Document {
        source: "assets/cards/code/card.typ",
        outputs: Outputs::Variants("assets/cards/code"),
        format: Format::Png { ppi: 200 },
    },
    Document {
        source: "assets/cards/social/card.typ",
        outputs: Outputs::Variants("assets/cards/social"),
        format: PIXEL,
    },
    Document {
        source: "assets/cards/icon/card.typ",
        outputs: Outputs::Variants("assets/cards/icon"),
        format: PIXEL,
    },
];

const FAILED: DiagId = DiagId::new("xtask.typeset.failed");
const UNREAD: DiagId = DiagId::new("xtask.typeset.variants");

/// Typst's command and options; `-` names the standard output.
const COMPILE: &str = "compile";
const ROOT: &str = "--root";
const FORMAT: &str = "--format";
const PPI: &str = "--ppi";
const INPUT: &str = "--input";
const ASSIGN: &str = "=";
const STDOUT: &str = "-";

/// One compilation: a document, or one variant of it.
struct Job {
    /// The Typst file, relative to the repository root.
    source: &'static str,
    format: Format,
    /// The output, relative to the repository root.
    output: PathBuf,
    inputs: Vec<(String, String)>,
}

#[expect(clippy::unnecessary_wraps, reason = "every command has the `Run` signature")]
pub(crate) fn compile_all(diag: &mut DiagCtxt, _: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let typst = match Program::Typst.find() {
        Ok(typst) => typst,
        Err(d) => {
            diag.error(*d);
            return Ok(Outcome::Failure);
        }
    };
    let root = repo_root();
    let mut outcome = Outcome::Success;
    let mut jobs = Vec::new();
    for (document, found) in ALL.iter().zip(ALL.par_map(|d| d.jobs(&root, &typst))) {
        match found {
            Ok(found) => jobs.extend(found),
            Err(unread) => {
                let msg =
                    Msg::new().text("the variants of ").code(document.source).text(" could not be read");
                diag.error(Diagnostic::error(UNREAD, msg).caused_by(&unread));
                outcome = Outcome::Failure;
            }
        }
    }
    for (job, result) in jobs.iter().zip(jobs.par_map(|job| job.compile(&root, &typst))) {
        let output = job.output.display().to_string();
        match result {
            Ok(()) => shell.status(&Typeset { output }),
            Err(failure) => {
                let msg = Msg::new().text("could not typeset ").code(output);
                diag.error(Diagnostic::error(FAILED, msg).caused_by(&failure));
                outcome = Outcome::Failure;
            }
        }
    }
    Ok(outcome)
}

impl Document {
    /// Its compilations: one, or one per variant it declares.
    fn jobs(&self, root: &Path, typst: &Executable) -> Result<Vec<Job>, Unread> {
        let file = |path: &Path| path.with_extension(self.format.extension());
        Ok(match self.outputs {
            Outputs::File(path) => vec![self.job(file(Path::new(path)), Vec::new())],
            Outputs::Variants(dir) => Variant::read(root, self.source, typst)?
                .into_iter()
                .map(|v| {
                    let output = v.path.iter().fold(PathBuf::from(dir), |p, part| p.join(part));
                    self.job(file(&output), v.inputs.into_iter().collect())
                })
                .collect(),
        })
    }

    const fn job(&self, output: PathBuf, inputs: Vec<(String, String)>) -> Job {
        Job { source: self.source, format: self.format, output, inputs }
    }
}

impl Format {
    const fn extension(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Png { .. } => "png",
        }
    }
}

impl Job {
    /// Compiles it under `root` and writes the output, creating its directory.
    fn compile(&self, root: &Path, typst: &Executable) -> Result<(), Failed> {
        let source = root.join(self.source);
        let format = self.format;
        let mut args = vec![COMPILE.into(), ROOT.into(), Arg::Path(root.to_owned())];
        args.extend([FORMAT.into(), format.extension().into()]);
        if let Format::Png { ppi } = format {
            args.extend([PPI.into(), Arg::Text(Symbol::intern(&ppi.to_string()))]);
        }
        for (key, value) in &self.inputs {
            args.extend([INPUT.into(), Arg::Text(Symbol::intern(&[key, ASSIGN, value].concat()))]);
        }
        args.extend([Arg::Path(source.clone()), STDOUT.into()]);
        let inv = Invocation { args, inputs: vec![source], ..Invocation::default() };
        let out = typst.check(&inv).map_err(Failed::Typst)?;
        root.join(&self.output).put(out.stdout).map_err(Failed::Write)
    }
}

/// A document compiled.
struct Typeset {
    output: String,
}

impl Record for Typeset {
    fn kind(&self) -> &'static str {
        "typeset"
    }

    fn text(&self) -> Text {
        Text::of(Style::Strong, self.output.clone())
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("output", self.output.clone())
    }
}

impl Status for Typeset {
    fn verb(&self) -> Verb {
        Verb::good("Compiled")
    }
}
