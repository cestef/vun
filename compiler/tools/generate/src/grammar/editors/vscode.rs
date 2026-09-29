//! VS Code: the extension's manifest, the language configuration and the `TextMate` grammar, which
//! the Sublime syntax's rules are written as. The hand-written client (`extension.js`) starts
//! the server the manifest names under `server`.

use vun_data::ToData;
use vun_data::json::Json;
use vun_syntax::lexer::COMMENT;

use super::{LANGUAGE, Pair, Server, TITLE, pairs};
use crate::generate::{Data, Emit, Format, Generator, Read, Source};
use crate::grammar::SYNTAX;
use crate::grammar::sublime::syntax;

pub(crate) const VSCODE: Generator = Generator {
    source: SYNTAX,
    output: "editors/vscode",
    emit: Emit::Data(emit),
    keeps: &["extension.js", "node_modules", "package-lock.json"],
};

/// The oldest VS Code the extension runs in, and the client library it starts the server with.
const ENGINE: &str = "^1.82.0";
const CLIENT: (&str, &str) = ("vscode-languageclient", "^10.1.2");
const MAIN: &str = "./extension.js";
const CONFIGURATION: &str = "language-configuration.json";
const GRAMMAR: &str = "syntaxes/vun.tmLanguage.json";

#[derive(ToData)]
#[data(case = camel)]
struct Manifest {
    name: String,
    display_name: String,
    description: String,
    version: String,
    publisher: String,
    engines: Engines,
    main: String,
    activation_events: Vec<String>,
    contributes: Contributes,
    dependencies: std::collections::BTreeMap<String, String>,
    /// Read by the client: how the server starts.
    server: Server,
}

#[derive(ToData)]
struct Engines {
    vscode: String,
}

#[derive(ToData)]
struct Contributes {
    languages: Vec<LanguageEntry>,
    grammars: Vec<GrammarEntry>,
}

#[derive(ToData)]
struct LanguageEntry {
    id: String,
    aliases: Vec<String>,
    extensions: Vec<String>,
    configuration: String,
}

#[derive(ToData)]
#[data(case = camel)]
struct GrammarEntry {
    language: String,
    scope_name: String,
    path: String,
}

#[derive(ToData)]
#[data(case = camel)]
struct Configuration {
    comments: Comments,
    brackets: Vec<Vec<String>>,
    auto_closing_pairs: Vec<Vec<String>>,
    surrounding_pairs: Vec<Vec<String>>,
}

#[derive(ToData)]
#[data(case = camel)]
struct Comments {
    line_comment: String,
}

fn emit(sources: &[Source<'_>]) -> Read<Vec<Data>> {
    let scope = ["source", LANGUAGE].join(".");
    let manifest = Manifest {
        name: LANGUAGE.to_owned(),
        display_name: TITLE.to_owned(),
        description: format!("{TITLE}: syntax, and the language server"),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        publisher: LANGUAGE.to_owned(),
        engines: Engines { vscode: ENGINE.to_owned() },
        main: MAIN.to_owned(),
        activation_events: Vec::new(),
        contributes: Contributes {
            languages: vec![LanguageEntry {
                id: LANGUAGE.to_owned(),
                aliases: vec![TITLE.to_owned()],
                extensions: vec![[".", vun_span::EXTENSION].concat()],
                configuration: ["./", CONFIGURATION].concat(),
            }],
            grammars: vec![GrammarEntry {
                language: LANGUAGE.to_owned(),
                scope_name: scope,
                path: ["./", GRAMMAR].concat(),
            }],
        },
        dependencies: std::collections::BTreeMap::from([(CLIENT.0.to_owned(), CLIENT.1.to_owned())]),
        server: Server::of(),
    };
    let pairs: Vec<Vec<String>> =
        pairs(sources)?.into_iter().map(|Pair { open, close }| vec![open, close]).collect();
    let brackets = pairs.iter().filter(|pair| pair[0] != pair[1]).cloned().collect();
    let configuration = Configuration {
        comments: Comments { line_comment: COMMENT.to_owned() },
        brackets,
        auto_closing_pairs: pairs.clone(),
        surrounding_pairs: pairs,
    };
    let json = |value: &dyn ToData| format!("{:#}\n", Json(&value.to_data()));
    Ok(vec![
        Data { name: "package.json".to_owned(), format: Format::Json, text: json(&manifest) },
        Data { name: CONFIGURATION.to_owned(), format: Format::Json, text: json(&configuration) },
        Data { name: GRAMMAR.to_owned(), format: Format::Json, text: syntax(sources)?.textmate() },
    ])
}
