//! Zed: the extension's manifest, the language's configuration and queries, and the server's
//! launch, which the hand-written extension (`src/lib.rs`, compiled to WebAssembly) includes.

use std::collections::BTreeMap;

use quote::quote;
use vun_data::ToData;
use vun_data::toml::Toml;
use vun_syntax::lexer::COMMENT;
use vun_syntax::literal::INDENT;

use super::{GRAMMAR_REPOSITORY, LANGUAGE, LAUNCH, Pair, REPOSITORY, SERVER, TITLE, grammar_revision, pairs};
use crate::generate::{Data, Emit, Format, Generator, Read, Source};
use crate::grammar::SYNTAX;
use crate::grammar::tree_sitter::{dialects, queries};

pub(crate) const ZED: Generator = Generator {
    source: SYNTAX,
    output: "editors/zed",
    emit: Emit::Data(emit),
    // Beside the hand-written extension, what Zed writes building it as a dev extension.
    keeps: &["Cargo.toml", "Cargo.lock", "src/lib.rs", "target", "extension.wasm", "grammars"],
};

/// The version of Zed's extension manifest.
const SCHEMA: u32 = 1;

#[derive(ToData)]
struct Extension {
    id: String,
    name: String,
    version: String,
    schema_version: u32,
    description: String,
    repository: String,
    language_servers: BTreeMap<String, LanguageServer>,
    grammars: BTreeMap<String, GrammarSource>,
}

#[derive(ToData)]
struct LanguageServer {
    name: String,
    languages: Vec<String>,
}

#[derive(ToData)]
struct GrammarSource {
    repository: String,
    rev: String,
}

/// `languages/vun/config.toml`.
#[derive(ToData)]
struct Language {
    name: String,
    grammar: String,
    path_suffixes: Vec<String>,
    line_comments: Vec<String>,
    tab_size: usize,
    brackets: Vec<Bracket>,
}

#[derive(ToData)]
struct Bracket {
    start: String,
    end: String,
    close: bool,
    newline: bool,
}

fn emit(sources: &[Source<'_>]) -> Read<Vec<Data>> {
    let extension = Extension {
        id: LANGUAGE.to_owned(),
        name: TITLE.to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        schema_version: SCHEMA,
        description: format!("{TITLE}: syntax, and the language server `{SERVER} {}`", LAUNCH.join(" ")),
        repository: REPOSITORY.to_owned(),
        language_servers: BTreeMap::from([(
            LANGUAGE.to_owned(),
            LanguageServer { name: TITLE.to_owned(), languages: vec![TITLE.to_owned()] },
        )]),
        grammars: BTreeMap::from([(
            LANGUAGE.to_owned(),
            GrammarSource { repository: GRAMMAR_REPOSITORY.to_owned(), rev: grammar_revision()? },
        )]),
    };
    let brackets = pairs(sources)?.into_iter().map(|Pair { open, close }| Bracket {
        newline: open != close,
        start: open,
        end: close,
        close: true,
    });
    let language = Language {
        name: TITLE.to_owned(),
        grammar: LANGUAGE.to_owned(),
        path_suffixes: vec![vun_span::EXTENSION.to_owned()],
        line_comments: vec![[COMMENT, " "].concat()],
        tab_size: INDENT.len(),
        brackets: brackets.collect(),
    };
    let toml =
        |value: &dyn ToData| value.to_data().as_object().map(|o| Toml(o).to_string()).unwrap_or_default();
    let dir = ["languages/", LANGUAGE].concat();
    let mut files = vec![
        Data { name: "extension.toml".to_owned(), format: Format::Toml, text: toml(&extension) },
        Data { name: [dir.as_str(), "/config.toml"].concat(), format: Format::Toml, text: toml(&language) },
        Data { name: "src/server.rs".to_owned(), format: Format::Rust, text: server() },
    ];
    files.extend(queries(sources, &dialects::ZED, &dir)?);
    Ok(files)
}

/// The program and the words starting the server, for the extension to include.
fn server() -> String {
    let words = LAUNCH.iter();
    let items = quote! {
        /// The program, found on the worktree's path.
        pub const COMMAND: &str = #SERVER;
        /// The words starting the server.
        pub const ARGS: &[&str] = &[#(#words),*];
    };
    prettyplease::unparse(&syn::parse2(items).expect("items parse"))
}
