//! The tree-sitter grammar of Vun, from the productions of `nodes.vun` and the tables of
//! `grammar.vun`: `grammar.js`, which `tree-sitter generate` turns into the parser in `src/`, its
//! `tree-sitter.json`, and the queries editors read.

use vun_data::ToData;
use vun_tree_sitter::{Config, Grammar as Syntax, Rule};

use super::SYNTAX;
use super::model::{Editors, Model};
use crate::generate::{Data, Emit, Format, Generator, Read, Source};

pub(crate) mod dialects;
mod lexical;
pub(crate) mod queries;
mod rules;
mod scanner;

use rules::Rules;

pub(crate) const TREE_SITTER: Generator = Generator {
    source: SYNTAX,
    output: "editors/tree-sitter",
    emit: Emit::Data(emit),
    keeps: &[PARSER, CONFLICTS],
};

/// Where `tree-sitter generate` writes the parser, beside the grammar.
const PARSER: &str = "src";
/// The sets of rules tree-sitter must keep apart, which `cargo xtask tree-sitter` finds.
pub(crate) const CONFLICTS: &str = "conflicts.json";
pub(crate) const LANGUAGE: &str = "vun";
const HIGHLIGHTS: &str = "queries/highlights.scm";

/// The query files of `dialect`, in the directory `dir` of an editor's package.
pub(crate) fn queries(sources: &[Source<'_>], dialect: &dialects::Dialect, dir: &str) -> Read<Vec<Data>> {
    let model = Model::read(sources)?;
    let editors = Editors::read(&model, sources)?;
    let rules = Rules::of(&model, &editors);
    dialect
        .files
        .iter()
        .map(|&file| {
            let text = dialect.query(&rules, file).to_string();
            Ok(Data { name: [dir, "/", file.name()].concat(), format: Format::Query, text })
        })
        .collect()
}

fn emit(sources: &[Source<'_>]) -> Read<Vec<Data>> {
    let model = Model::read(sources)?;
    let editors = Editors::read(&model, sources)?;
    let rules = Rules::of(&model, &editors);
    let mut all = rules.all();
    all.extend(rules.wrapped());
    all.extend(rules.lexical()?);
    let mut extras = vec![Rule::Pattern(r"\s".to_owned())];
    for &extra in &editors.tree_sitter.extras {
        let name = model.token_name(extra);
        all.push((name.to_owned(), lexical::rule(name).ok_or_else(|| rules::missing(name))?));
        extras.push(Rule::symbol(name));
    }
    let syntax = Syntax {
        name: LANGUAGE.to_owned(),
        word: Some(rules::hidden(model.token_name(editors.tree_sitter.word))),
        extras,
        externals: vec![Rule::symbol(scanner::LINE_END)],
        conflicts: Vec::new(),
        conflicts_file: Some(CONFLICTS.to_owned()),
        rules: all,
    };
    let config = Config::of(LANGUAGE, vun_span::EXTENSION, HIGHLIGHTS, env!("CARGO_PKG_VERSION"));
    Ok(vec![
        Data { name: "grammar.js".to_owned(), format: Format::JavaScript, text: syntax.to_string() },
        Data {
            name: "tree-sitter.json".to_owned(),
            format: Format::Json,
            text: format!("{:#}\n", vun_data::json::Json(&config.to_data())),
        },
        Data { name: "src/scanner.c".to_owned(), format: Format::C, text: scanner::source() },
        Data {
            name: HIGHLIGHTS.to_owned(),
            format: Format::Query,
            text: dialects::STANDARD.query(&rules, queries::File::Highlights).to_string(),
        },
    ])
}
