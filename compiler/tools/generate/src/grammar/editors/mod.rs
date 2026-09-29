//! The editor packages of Vun, generated from the grammar and the toolchain's facts so none is
//! stated twice: each registers `.vun` files, starts the language server and reads the
//! tree-sitter grammar's queries in its own dialect. What an editor runs as code (a Zed
//! extension, a VS Code client, a Neovim plugin) is written by hand beside the generated files
//! and reads their data.

mod helix;
mod nvim;
mod vscode;
mod zed;

pub(crate) use helix::HELIX;
pub(crate) use nvim::NVIM;
pub(crate) use vscode::VSCODE;
pub(crate) use zed::ZED;

use vun_data::ToData;
use vun_syntax::literal::Literal;

use super::lexical::QUOTE;
use super::tree_sitter::LANGUAGE;
use super::{Grammar, grammar};
use crate::generate::{Read, Source};

/// The language as people read its name.
const TITLE: &str = "Vun";
/// The language server: the program, and the words starting it (`vun util lsp`).
const SERVER: &str = "vun";
const LAUNCH: [&str; 2] = ["util", "lsp"];
/// Where Vun is developed.
const REPOSITORY: &str = "https://github.com/cestef/vun";
/// Where the tree-sitter grammar is published, with its parser, and the revision editors build.
const GRAMMAR_REPOSITORY: &str = "https://github.com/cestef/tree-sitter-vun";
const GRAMMAR_REVISION: &str = "main";
/// The tokens and table the facts read, as `grammar.vun` names them.
const TOKENS: &str = "TokenKind";
const DELIMITERS: &str = "delimiters";
const OPEN: &str = "open";
const CLOSE: &str = "close";

/// How the language server is started.
#[derive(Clone, Debug, ToData)]
struct Server {
    command: String,
    args: Vec<String>,
}

impl Server {
    fn of() -> Self {
        Self { command: SERVER.to_owned(), args: LAUNCH.map(str::to_owned).into() }
    }
}

/// A pair of brackets an editor closes as it is typed.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Pair {
    open: String,
    close: String,
}

/// The pairs of one-character delimiters of the grammar, and the quotes of strings.
fn pairs(sources: &[Source<'_>]) -> Read<Vec<Pair>> {
    let source = grammar(sources);
    let grammar = Grammar::read(&source)?;
    let tokens = grammar.enums.iter().find(|e| e.name == TOKENS);
    let spelled = |name: Option<&str>| {
        let variant = tokens?.variants.iter().find(|v| Some(v.name) == name)?;
        variant.text.as_deref().map(str::to_owned)
    };
    let rows = source
        .decls
        .iter()
        .find(|d| d.name == DELIMITERS)
        .and_then(|d| d.value.as_list())
        .unwrap_or_default();
    let mut pairs: Vec<Pair> = rows
        .iter()
        .filter_map(|r: &Literal<'_>| {
            let open = spelled(r.get(OPEN)?.as_name())?;
            let close = spelled(r.get(CLOSE)?.as_name())?;
            (open.chars().count() == 1).then_some(Pair { open, close })
        })
        .collect();
    pairs.push(Pair { open: QUOTE.to_string(), close: QUOTE.to_string() });
    Ok(pairs)
}
