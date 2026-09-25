//! Rust sources generated from Vun sources, which stay the single source of truth.
//!
//! A [`Generator`] reads the literal declarations of the `.vun` files at its source, one file or a
//! directory of them, and emits typed Rust modules, which become the files of its output
//! directory, each one concern, with a `mod.rs` declaring them; reading, parsing, formatting and
//! the staleness check are shared here. Adding a generator is adding an entry to [`ALL`].

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use proc_macro2::TokenStream;
use quote::quote;
use vun_build::layout::{Location, SEPARATOR};
use vun_diag::{DiagCtxt, DiagCx, DiagId, Diagnostic, Msg};
use vun_fs::{Relative, Tree};
use vun_origin::OriginTable;
use vun_span::{EXTENSION, SourceFiles};
use vun_syntax::cst::Cst;
use vun_syntax::literal::Decl;
use vun_syntax::tokens::Tokens;

use crate::names::raw;
use crate::{errors, grammar, items, primitives};

pub(crate) struct Generator {
    /// The Vun file, or the directory of them, relative to the repository root.
    pub(crate) source: &'static str,
    /// The generated Rust module's directory, relative to the repository root, which holds
    /// nothing else.
    pub(crate) output: &'static str,
    /// Receives one source per file, in path order.
    pub(crate) emit: fn(&[Source<'_>]) -> Read<Vec<Module>>,
}

/// One file of a generated directory.
pub(crate) struct Module {
    /// The module and file name, without extension.
    pub(crate) name: String,
    pub(crate) access: Access,
    /// Its items, with the imports they need.
    pub(crate) items: TokenStream,
}

/// How the items of a generated module are reached.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Access {
    /// As the directory's own: `pub use name::*`.
    Glob,
    /// Through the module's path: `pub mod name`.
    Path,
    /// It only adds methods to the types of its siblings, reached through them: `mod name`.
    Impl,
}

impl Module {
    /// A module of one generated directory whose items name those of its siblings.
    pub(crate) fn sibling(name: String, items: &TokenStream) -> Self {
        let items = quote! {
            #[allow(unused_imports, clippy::wildcard_imports, reason = "generated modules share one namespace")]
            use super::*;
            #items
        };
        Self { name, access: Access::Glob, items }
    }
}

/// The name of a directory module's root file.
const ROOT: &str = "mod";

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
    errors::GENERATOR,
    primitives::GENERATOR,
    items::LANG,
    items::STD,
    items::TARGETS,
];

const SYNTAX: DiagId = DiagId::new("generate.generate.syntax");

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
        let unreadable = |e: &std::io::Error, path: &str| {
            let msg = Msg::new().text("cannot read ").code(path);
            Box::new(Diagnostic::error(vun_diag::errors::IO_UNREADABLE_FILE, msg).caused_by(e))
        };
        let files = root.join(self.source).files().map_err(|e| unreadable(&e, self.source))?;
        let mut texts = Vec::new();
        for file in files.iter().filter(|f| f.extension().is_some_and(|e| e == EXTENSION)) {
            let path = file.relative_to(root).unwrap_or_else(|| file.clone()).display().to_string();
            let text = std::fs::read_to_string(file).map_err(|e| unreadable(&e, &path))?;
            texts.push((path, text));
        }
        let mut sources = Vec::new();
        for (path, text) in &texts {
            sources.push(Self::parse(path, text)?);
        }
        let modules = (self.emit)(&sources)?;
        let header = format!(" Generated by `cargo gen` from `{}`; do not edit.", self.source);
        let format = |items: TokenStream| {
            let file = syn::parse2(quote! { #![doc = #header] #items }).expect("generators emit Rust items");
            prettyplease::unparse(&file)
        };
        let declared = modules.iter().map(|m| {
            let name = raw(&m.name);
            match m.access {
                Access::Glob => quote! { mod #name; pub use #name::*; },
                Access::Path => quote! { pub mod #name; },
                Access::Impl => quote! { mod #name; },
            }
        });
        let mut files = BTreeMap::from([(format!("{ROOT}.rs"), format(quote! { #(#declared)* }))]);
        for module in modules {
            files.insert(format!("{}.rs", module.name), format(module.items));
        }
        Ok(files)
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
        let present = if dir.is_dir() { dir.files()? } else { Vec::new() };
        let stale = present
            .into_iter()
            .filter(|path| {
                path.file_name().and_then(|n| n.to_str()).is_none_or(|n| !generated.contains_key(n))
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
    fn parse<'s>(path: &str, text: &'s str) -> Read<Source<'s>> {
        let file = SourceFiles::new().add(path, text);
        let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
        let mut cx = DiagCx { diag: &mut diag, origins: &origins };
        let tokens = Tokens::lex(file, text, &mut cx);
        let cst = Cst::parse(&tokens, text, &mut cx);
        if diag.error_count() > 0 {
            let run = format!("vun debug cst {path}");
            let msg = Msg::new().code(path).text(" does not parse; run ").code(run);
            return Err(Box::new(Diagnostic::error(SYNTAX, msg)));
        }
        Ok(Source { path: path.to_owned(), decls: cst.declarations(&tokens, text) })
    }
}
