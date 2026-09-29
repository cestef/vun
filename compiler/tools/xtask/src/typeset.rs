//! `cargo xtask typeset`: the Typst documents of the repository, compiled with the repository as
//! their root. They highlight Vun with the generated editor syntax, which `cargo gen` keeps
//! current.

use std::io;
use std::path::Path;

use vun_command::{Args, Outcome, Shell};
use vun_diag::{DiagCtxt, DiagId, Diagnostic, Msg};
use vun_harness::repo_root;
use vun_intern::Symbol;
use vun_json::Object;
use vun_term::{Record, Status, Style, Text, Verb};
use vun_tool::{Arg, Executable, Failure, Invocation, Tool};

use crate::task::Program;

pub(crate) struct Document {
    /// The Typst file, relative to the repository root.
    pub(crate) source: &'static str,
    /// The file compiled, relative to the repository root; Typst reads its format from its
    /// extension.
    pub(crate) output: &'static str,
    pub(crate) raster: Option<Raster>,
    /// Values the document reads from `sys.inputs`.
    pub(crate) inputs: &'static [Input],
}

/// How an image output is rasterized.
#[derive(Clone, Copy)]
pub(crate) struct Raster {
    pub(crate) ppi: u16,
}

pub(crate) struct Input {
    pub(crate) key: &'static str,
    pub(crate) value: &'static str,
}

const CARD: &str = "assets/code/card.typ";
const CARD_RASTER: Option<Raster> = Some(Raster { ppi: 200 });
const SCHEME: &str = "scheme";

pub(crate) const ALL: &[Document] = &[
    Document { source: "whitepaper/main.typ", output: "whitepaper/main.pdf", raster: None, inputs: &[] },
    Document {
        source: CARD,
        output: "assets/code-light.png",
        raster: CARD_RASTER,
        inputs: &[Input { key: SCHEME, value: "light" }],
    },
    Document {
        source: CARD,
        output: "assets/code-dark.png",
        raster: CARD_RASTER,
        inputs: &[Input { key: SCHEME, value: "dark" }],
    },
];

const FAILED: DiagId = DiagId::new("xtask.typeset.failed");

/// Typst's command and options.
const COMPILE: &str = "compile";
const ROOT: &str = "--root";
const PPI: &str = "--ppi";
const INPUT: &str = "--input";
const ASSIGN: &str = "=";

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
    for document in ALL {
        match document.compile(&root, &typst) {
            Ok(()) => shell.status(&Typeset { output: document.output }),
            Err(failure) => {
                let msg =
                    Msg::new().code(Program::Typst.name()).text(" could not compile ").code(document.output);
                diag.error(Diagnostic::error(FAILED, msg).caused_by(&failure));
                outcome = Outcome::Failure;
            }
        }
    }
    Ok(outcome)
}

impl Document {
    /// Compiles the document under `root`.
    fn compile(&self, root: &Path, typst: &Executable) -> Result<(), Failure> {
        let (source, output) = (root.join(self.source), root.join(self.output));
        let mut args = vec![COMPILE.into(), ROOT.into(), Arg::Path(root.to_owned())];
        if let Some(raster) = self.raster {
            args.extend([PPI.into(), Arg::Text(Symbol::intern(&raster.ppi.to_string()))]);
        }
        for input in self.inputs {
            args.extend([
                INPUT.into(),
                Arg::Text(Symbol::intern(&[input.key, ASSIGN, input.value].concat())),
            ]);
        }
        args.extend([Arg::Path(source.clone()), Arg::Path(output.clone())]);
        let inv = Invocation { args, inputs: vec![source], outputs: vec![output], ..Invocation::default() };
        typst.check(&inv).map(drop)
    }
}

/// A document compiled.
struct Typeset {
    output: &'static str,
}

impl Record for Typeset {
    fn kind(&self) -> &'static str {
        "typeset"
    }

    fn text(&self) -> Text {
        Text::of(Style::Strong, self.output)
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("output", self.output)
    }
}

impl Status for Typeset {
    fn verb(&self) -> Verb {
        Verb::good("Compiled")
    }
}
