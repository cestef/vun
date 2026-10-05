//! Rust sources generated from Vun sources, which stay the single source of truth.
//!
//! A [`Generator`] reads the literal declarations of the `.vun` files at its source, one file or a
//! directory of them, and emits typed Rust modules, which become the files of its output
//! directory, each one concern, with a `mod.rs` declaring them; reading, parsing, formatting and
//! the staleness check are shared here. Adding a generator is adding an entry to [`ALL`].

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use vun_build::layout::{Location, SEPARATOR};
use vun_diag::{ChildKind, DiagCtxt, DiagCx, DiagId, Diagnostic, Msg};
use vun_fs::{Relative, Tree};
use vun_origin::OriginTable;
use vun_span::{EXTENSION, SourceFiles};
use vun_syntax::cst::Cst;
use vun_syntax::literal::Decl;
use vun_syntax::tokens::Tokens;

use crate::{asm, errors, grammar, items, primitives};

mod data;
mod module;

pub(crate) use data::{Data, Format};
pub(crate) use module::{Access, JOIN, Module};

pub(crate) struct Generator {
    /// The Vun file, or the directory of them, relative to the repository root.
    pub(crate) source: &'static str,
    /// The output directory, relative to the repository root, which holds nothing else.
    pub(crate) output: &'static str,
    pub(crate) emit: Emit,
    /// Directories of the output that other tools write, which it leaves alone.
    pub(crate) keeps: &'static [&'static str],
}

/// What a generator makes of its sources, given one source per file, in path order.
#[derive(Clone, Copy)]
pub(crate) enum Emit {
    /// Rust modules, formatted, with a `mod.rs` declaring them; files of the output directory
    /// no longer generated are removed.
    Rust(fn(&[Source<'_>]) -> Read<Vec<Module>>),
    /// Files of another format, each written with the header of its format.
    Data(fn(&[Source<'_>]) -> Read<Vec<Data>>),
}

/// A parsed file handed to a generator.
pub(crate) struct Source<'s> {
    /// Relative to the repository root.
    pub(crate) path: String,
    pub(crate) decls: Vec<Decl<'s>>,
}

pub(crate) type Read<T> = Result<T, Box<Diagnostic>>;

pub(crate) const ALL: &[Generator] = &[
    grammar::GENERATOR,
    grammar::DESUGAR,
    grammar::SUBLIME,
    grammar::TREE_SITTER,
    grammar::HELIX,
    grammar::NVIM,
    grammar::ZED,
    grammar::VSCODE,
    errors::GENERATOR,
    primitives::GENERATOR,
    items::LANG,
    items::STD,
    items::TARGETS,
    asm::X86_64,
    asm::ARM64,
    asm::THUMB,
    asm::WASM32,
];

const SYNTAX: DiagId = DiagId::new("generate.generate.syntax");
const EMPTY: DiagId = DiagId::new("generate.generate.empty");
/// Two modules of one output directory with one name, as `a.b_c` and `a_b.c` join.
pub(super) const COLLISION: DiagId = DiagId::new("generate.generate.collision");

/// Where packages live, relative to the repository root.
pub(crate) const PACKAGES: &str = "lib";

impl Generator {
    /// A diagnostic about the declaration `decl` of this generator's source.
    pub(crate) fn error(&self, id: DiagId, decl: &str, msg: Msg) -> Box<Diagnostic> {
        let at = Msg::new().code(decl).text(" in ").code(self.source).text(": ");
        Box::new(Diagnostic::error(id, at.append(msg)))
    }
}

impl Source<'_> {
    /// The binding path of the file, as a namespace of its package in [`PACKAGES`]:
    /// `lib/lang/lang/errors.vun` is `lang.errors`. `None` for a file outside the package's root
    /// namespace, such as its manifest.
    pub(crate) fn namespace(&self) -> Option<Vec<String>> {
        let path = Path::new(&self.path);
        let package = path.strip_prefix(PACKAGES).ok()?.iter().next()?;
        let names = Location::path_of(&Path::new(PACKAGES).join(package), path)?;
        (names.first().map(String::as_str) == package.to_str()).then_some(names)
    }

    /// [`Self::namespace`] joined as written.
    pub(crate) fn namespace_text(&self) -> String {
        self.namespace().unwrap_or_default().join(SEPARATOR)
    }
}

impl Generator {
    /// The formatted files of the output directory for the source under `root`, by file name.
    ///
    /// # Errors
    /// Returns the diagnostics of a source that cannot be read or parsed, or that has a shape the
    /// generator does not translate.
    pub(crate) fn generate(&self, root: &Path) -> Result<BTreeMap<String, String>, Box<Diagnostic>> {
        let unreadable = |e: &std::io::Error, path: &Path| Box::new(Diagnostic::unreadable(path, e));
        let files = root.join(self.source).files().map_err(|e| unreadable(&e, Path::new(self.source)))?;
        let mut texts = Vec::new();
        for file in files.iter().filter(|f| f.extension().is_some_and(|e| e == EXTENSION)) {
            let path = file.relative_to(root).unwrap_or_else(|| file.clone());
            let text = std::fs::read_to_string(file).map_err(|e| unreadable(&e, &path))?;
            texts.push((path.display().to_string(), text));
        }
        if texts.is_empty() {
            return Err(self.error(
                EMPTY,
                self.source,
                Msg::new().text("holds no ").code(EXTENSION).text(" file"),
            ));
        }
        let mut sources = Vec::new();
        for (path, text) in &texts {
            sources.push(Self::parse(path, text)?);
        }
        let header = format!(" Generated by `cargo gen` from `{}`; do not edit.", self.source);
        let modules = match self.emit {
            Emit::Rust(emit) => emit(&sources)?,
            Emit::Data(emit) => {
                let files = emit(&sources)?.into_iter().map(|d| {
                    let text = [d.format.header(&header), d.text].concat();
                    (d.name, text)
                });
                return Ok(files.collect());
            }
        };
        self.files(modules, &header)
    }

    /// How the output directory under `root` differs from `generated`: the files to write, and
    /// the files no longer generated.
    ///
    /// # Errors
    /// Returns an error if the directory cannot be read.
    pub(crate) fn changes(
        &self,
        root: &Path,
        generated: &BTreeMap<String, String>,
    ) -> std::io::Result<Changes> {
        let dir = root.join(self.output);
        // What the output keeps is another's, never read.
        let kept =
            |path: &Path| path.strip_prefix(&dir).is_ok_and(|r| self.keeps.iter().any(|k| r.starts_with(k)));
        let present = if dir.is_dir() { dir.files_except(&kept)? } else { Vec::new() };
        let stale = present
            .into_iter()
            .filter(|path| {
                let relative = path.strip_prefix(&dir).unwrap_or(path);
                relative.to_str().is_none_or(|n| !generated.contains_key(n))
            })
            .collect();
        let differing = generated
            .iter()
            .filter(|(name, text)| {
                std::fs::read_to_string(dir.join(name)).ok().as_deref() != Some(text.as_str())
            })
            .map(|(name, _)| name.clone())
            .collect();
        Ok(Changes { differing, stale })
    }
}

/// What a generator would change in its output directory.
pub(crate) struct Changes {
    /// Files to write, by name.
    pub(crate) differing: Vec<String>,
    /// Files there that are no longer generated.
    pub(crate) stale: Vec<PathBuf>,
}

impl Changes {
    pub(crate) fn is_empty(&self) -> bool {
        self.differing.is_empty() && self.stale.is_empty()
    }
}

impl Generator {
    pub(crate) fn parse<'s>(path: &str, text: &'s str) -> Read<Source<'s>> {
        let file = SourceFiles::new().add(path, text);
        let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
        let mut cx = DiagCx { diag: &mut diag, origins: &origins };
        let tokens = Tokens::lex(file, text, &mut cx);
        let cst = Cst::parse(&tokens, text, &mut cx);
        if diag.error_count() > 0 {
            let run = ["vun debug dump cst", path].join(" ");
            let msg = Msg::new().code(path).text(" does not parse; run ").code(run);
            let errors = diag.sorted(&origins).into_iter().map(|e| e.message.clone());
            let d = errors.fold(Diagnostic::error(SYNTAX, msg), |d, e| d.child(ChildKind::Note, e));
            return Err(Box::new(d));
        }
        Ok(Source { path: path.to_owned(), decls: cst.declarations(&tokens, text) })
    }
}
