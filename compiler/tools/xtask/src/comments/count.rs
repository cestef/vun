use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use proc_macro2::{TokenStream, TokenTree};
use syn::visit::Visit;
use vun_par::Parallel;

use crate::lines::CHECKED;

/// Directories whose files are written by generators, relative to the repository root.
const GENERATED_DIRS: &[&str] = &["compiler/base/diag/src/errors", "compiler/lang/items/src"];
/// A directory name under which every file is generated.
const GENERATED: &str = "generated";
/// The line comments that are not counted: documentation, test annotations and the soundness
/// arguments of `unsafe`.
const EXEMPT: &[&str] = &["///", "//!", "//~", "// SAFETY"];
const LINE_COMMENT: &str = "//";

/// The comment lines inside function bodies, by group of crates (the directory under `compiler/`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Counts(pub(crate) BTreeMap<String, u32>);

/// Why a source file's comments are not counted.
#[derive(Debug)]
pub(crate) enum CountError {
    Read { path: PathBuf, source: io::Error },
    Parse { path: PathBuf, source: syn::Error },
}

impl fmt::Display for CountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, .. } => write!(f, "`{}` cannot be read", path.display()),
            Self::Parse { path, .. } => write!(f, "`{}` is not Rust", path.display()),
        }
    }
}

impl Error for CountError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
        }
    }
}

impl Counts {
    /// The comment lines of every source under `root/compiler` that is not generated.
    ///
    /// # Errors
    /// Returns why a file is not read or not parsed.
    pub(crate) fn of(root: &Path) -> Result<Self, CountError> {
        let all = CHECKED[0].files(root).map_err(|source| CountError::Read { path: root.into(), source })?;
        let kept: Vec<PathBuf> = all.into_iter().filter(|f| !Self::generated(f, root)).collect();
        let counted = kept.par_map(|path| Source::read(path).ok().map(|source| source.comments()));
        let mut counts = Self::default();
        for (path, count) in kept.iter().zip(counted) {
            let Some(count) = count else {
                return Err(Source::read(path).err().expect("a file that failed"));
            };
            let relative = path.strip_prefix(root).unwrap_or(path);
            let group = relative.components().nth(1).map(|c| c.as_os_str().to_string_lossy().into_owned());
            *counts.0.entry(group.unwrap_or_default()).or_default() += count;
        }
        Ok(counts)
    }

    fn generated(path: &Path, root: &Path) -> bool {
        let relative = path.strip_prefix(root).unwrap_or(path);
        GENERATED_DIRS.iter().any(|dir| relative.starts_with(dir))
            || relative.components().any(|c| c.as_os_str() == GENERATED)
    }

    pub(crate) fn total(&self) -> u32 {
        self.0.values().sum()
    }
}

/// A parsed source file, with the lines its tokens occupy.
pub(crate) struct Source {
    text: String,
    file: syn::File,
    tokened: BTreeSet<usize>,
}

impl Source {
    fn read(path: &Path) -> Result<Self, CountError> {
        let text =
            std::fs::read_to_string(path).map_err(|source| CountError::Read { path: path.into(), source })?;
        Self::parse(text).map_err(|source| CountError::Parse { path: path.into(), source })
    }

    pub(crate) fn parse(text: String) -> Result<Self, syn::Error> {
        let stream = TokenStream::from_str(&text)?;
        let mut tokened = BTreeSet::new();
        Self::mark(stream.clone(), &mut tokened);
        Ok(Self { file: syn::parse2(stream)?, text, tokened })
    }

    fn mark(stream: TokenStream, lines: &mut BTreeSet<usize>) {
        for tree in stream {
            let (start, end) = match &tree {
                TokenTree::Group(group) => {
                    Self::mark(group.stream(), lines);
                    (group.span_open().start().line, group.span_close().end().line)
                }
                other => (other.span().start().line, other.span().end().line),
            };
            lines.extend([start, end]);
            if let TokenTree::Literal(_) = tree {
                lines.extend(start..=end);
            }
        }
    }

    /// The `//` comment lines inside function bodies: lines holding no token, so not the trailing
    /// comments of code, and not documentation, test annotations or `SAFETY` arguments.
    pub(crate) fn comments(&self) -> u32 {
        let mut bodies = Bodies::default();
        bodies.visit_file(&self.file);
        let lines: Vec<&str> = self.text.lines().collect();
        let counted = bodies.inside.iter().filter(|&&line| {
            let text = lines.get(line - 1).map_or("", |l| l.trim_start());
            !self.tokened.contains(&line)
                && text.starts_with(LINE_COMMENT)
                && !EXEMPT.iter().any(|exempt| text.starts_with(exempt))
        });
        u32::try_from(counted.count()).unwrap_or(u32::MAX)
    }
}

/// The lines strictly inside the braces of every function body.
#[derive(Default)]
struct Bodies {
    inside: BTreeSet<usize>,
}

impl Bodies {
    fn body(&mut self, block: &syn::Block) {
        let (open, close) = (block.brace_token.span.open().start(), block.brace_token.span.close().start());
        self.inside.extend(open.line + 1..close.line);
    }
}

impl<'ast> Visit<'ast> for Bodies {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.body(&node.block);
        syn::visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.body(&node.block);
        syn::visit::visit_impl_item_fn(self, node);
    }

    fn visit_trait_item_fn(&mut self, node: &'ast syn::TraitItemFn) {
        if let Some(block) = &node.default {
            self.body(block);
        }
        syn::visit::visit_trait_item_fn(self, node);
    }
}
