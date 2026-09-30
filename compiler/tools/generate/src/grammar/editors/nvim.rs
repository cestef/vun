//! Neovim: the queries in `queries/vun`, and the data the hand-written plugin reads
//! (`lua/vun/config.lua`): the file type, how nvim-treesitter builds the parser, and how the
//! language server starts.

use vun_build::layout::MANIFEST;
use vun_data::ToData;
use vun_data::lua::Lua;

use super::{GRAMMAR_REPOSITORY, LANGUAGE, Server, grammar_revision};
use crate::generate::{Data, Emit, Format, Generator, Read, Source};
use crate::grammar::SYNTAX;
use crate::grammar::tree_sitter::{dialects, parser_sources, queries};

pub(crate) const NVIM: Generator =
    Generator { source: SYNTAX, output: "editors/nvim", emit: Emit::Data(emit), keeps: &["plugin"] };

/// What the plugin reads.
#[derive(ToData)]
struct Config {
    filetype: String,
    extension: String,
    parser: Parser,
    server: Server,
    root_markers: Vec<String>,
}

/// nvim-treesitter's `install_info`: it generates the parser from `grammar.js` itself.
#[derive(ToData)]
struct Parser {
    url: String,
    revision: String,
    files: Vec<String>,
    requires_generate_from_grammar: bool,
}

fn emit(sources: &[Source<'_>]) -> Read<Vec<Data>> {
    let config = Config {
        filetype: LANGUAGE.to_owned(),
        extension: vun_span::EXTENSION.to_owned(),
        parser: Parser {
            url: GRAMMAR_REPOSITORY.to_owned(),
            revision: grammar_revision()?,
            files: parser_sources(sources)?.into_iter().map(str::to_owned).collect(),
            requires_generate_from_grammar: false,
        },
        server: Server::of(),
        root_markers: vec![MANIFEST.to_owned()],
    };
    let text = format!("return {:#}\n", Lua(&config.to_data()));
    let mut files =
        vec![Data { name: ["lua/", LANGUAGE, "/config.lua"].concat(), format: Format::Lua, text }];
    files.extend(queries(sources, &dialects::NEOVIM, &["queries/", LANGUAGE].concat())?);
    Ok(files)
}
