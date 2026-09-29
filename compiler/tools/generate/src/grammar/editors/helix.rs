//! Helix: `languages.toml`, the entries a user's own merges in, and the queries, which go to the
//! runtime directory's `queries/vun`.

use std::collections::BTreeMap;

use vun_build::layout::MANIFEST;
use vun_data::ToData;
use vun_data::toml::Toml;
use vun_syntax::lexer::COMMENT;
use vun_syntax::literal::INDENT;

use super::{GRAMMAR_REPOSITORY, GRAMMAR_REVISION, LANGUAGE, Server};
use crate::generate::{Data, Emit, Format, Generator, Read, Source};
use crate::grammar::SYNTAX;
use crate::grammar::tree_sitter::{dialects, queries};

pub(crate) const HELIX: Generator =
    Generator { source: SYNTAX, output: "editors/helix", emit: Emit::Data(emit), keeps: &[] };

#[derive(ToData)]
struct Languages {
    language: Vec<Language>,
    #[data(name = "language-server")]
    servers: BTreeMap<String, Server>,
    grammar: Vec<GrammarEntry>,
}

#[derive(ToData)]
struct Language {
    name: String,
    scope: String,
    #[data(name = "file-types")]
    file_types: Vec<String>,
    roots: Vec<String>,
    #[data(name = "comment-token")]
    comment: String,
    indent: Indent,
    #[data(name = "language-servers")]
    servers: Vec<String>,
}

#[derive(ToData)]
struct Indent {
    #[data(name = "tab-width")]
    tab_width: usize,
    unit: String,
}

#[derive(ToData)]
struct GrammarEntry {
    name: String,
    source: GitSource,
}

#[derive(ToData)]
struct GitSource {
    git: String,
    rev: String,
}

fn emit(sources: &[Source<'_>]) -> Read<Vec<Data>> {
    let language = Language {
        name: LANGUAGE.to_owned(),
        scope: ["source", LANGUAGE].join("."),
        file_types: vec![vun_span::EXTENSION.to_owned()],
        roots: vec![MANIFEST.to_owned()],
        comment: COMMENT.to_owned(),
        indent: Indent { tab_width: INDENT.len(), unit: INDENT.to_owned() },
        servers: vec![LANGUAGE.to_owned()],
    };
    let source = GitSource { git: GRAMMAR_REPOSITORY.to_owned(), rev: GRAMMAR_REVISION.to_owned() };
    let languages = Languages {
        language: vec![language],
        servers: BTreeMap::from([(LANGUAGE.to_owned(), Server::of())]),
        grammar: vec![GrammarEntry { name: LANGUAGE.to_owned(), source }],
    };
    let data = languages.to_data();
    let text = data.as_object().map(|o| Toml(o).to_string()).unwrap_or_default();
    let mut files = vec![Data { name: "languages.toml".to_owned(), format: Format::Toml, text }];
    files.extend(queries(sources, &dialects::HELIX, &["queries/", LANGUAGE].concat())?);
    Ok(files)
}
