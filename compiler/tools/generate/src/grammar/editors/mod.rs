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

use super::lexical::QUOTE;
use super::model::Model;
use super::tree_sitter::LANGUAGE;
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
    let model = Model::read(sources)?;
    let spelled = |t| model.text(t).map(str::to_owned);
    let mut pairs: Vec<Pair> = model
        .delimiters
        .iter()
        .filter_map(|&(open, close)| {
            let (open, close) = (spelled(open)?, spelled(close)?);
            (open.chars().count() == 1).then_some(Pair { open, close })
        })
        .collect();
    pairs.push(Pair { open: QUOTE.to_string(), close: QUOTE.to_string() });
    Ok(pairs)
}
