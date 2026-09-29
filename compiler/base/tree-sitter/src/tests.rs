use crate::{Assoc, Config, Grammar, Pattern, Query, Rule, Tree};

#[test]
fn rules_are_written_as_grammar_js_calls() {
    let rule = Rule::seq(vec![Rule::symbol("a").field("left"), Rule::text("+"), Rule::Pattern("a/b".into())])
        .prec(Assoc::Left, 3);
    assert_eq!(rule.to_string(), r#"prec.left(3, seq(field("left", $.a), "+", /a\/b/))"#);
    assert_eq!(Rule::seq(vec![Rule::text("x")]), Rule::text("x"));
}

#[test]
fn a_grammar_lists_its_parts_and_rules() {
    let grammar = Grammar {
        name: "t".into(),
        word: Some("id".into()),
        extras: vec![Rule::Pattern(r"\s".into())],
        conflicts: vec![vec!["a".into(), "b".into()]],
        rules: vec![
            ("root".into(), Rule::symbol("id").repeat()),
            ("id".into(), Rule::Pattern("[a-z]+".into())),
        ],
        conflicts_file: Some("conflicts.json".into()),
        ..Grammar::default()
    };
    let js = grammar.to_string();
    assert!(js.contains("  word: $ => $.id,\n"));
    assert!(js.contains(r#"  conflicts: $ => [[$.a, $.b], ...require("./conflicts.json").map((set) => set.map((name) => $[name]))],"#));
    assert!(js.contains("    root: $ => repeat($.id),\n"));
    assert!(!js.contains("externals"));
}

#[test]
fn queries_write_nodes_fields_tokens_and_predicates() {
    let call = Pattern::node("call").child(Some("callee"), Pattern::node("name").capture("function"));
    let keywords = Pattern::alternatives([Pattern::token("if"), Pattern::token("else")]).capture("keyword");
    let types = Pattern::node("name").capture("type").matching("type", "^[A-Z]");
    let query = Query { patterns: vec![call, keywords, types] };
    let renamed = Pattern::node("name").capture(1).map(&|c: i32| (c == 1).then_some("one"));
    assert_eq!(renamed.map(|p| p.to_string()).as_deref(), Some("(name) @one"));
    assert!(Pattern::node("name").capture(2).map(&|c: i32| (c == 1).then_some("one")).is_none());
    assert_eq!(
        query.to_string(),
        "(call callee: (name) @function)\n[\"if\" \"else\"] @keyword\n((name) @type (#match? @type \"^[A-Z]\"))\n"
    );
}

#[test]
fn printed_trees_read_back() {
    let text = "(file [0, 0] - [1, 0]\n  members: (binding [0, 0] - [0, 6]\n    pattern: (name [0, 0] - [0, 1])\n    (MISSING \";\" [0, 6] - [0, 6])))\nfile.vun\tParse: 0.1 ms\n";
    let tree = Tree::read(text).expect("a tree");
    let [file] = tree.roots.as_slice() else { panic!("one tree") };
    assert_eq!(file.kind, "file");
    let binding = &file.children[0];
    assert_eq!((binding.kind.as_str(), binding.field.as_deref()), ("binding", Some("members")));
    assert_eq!(binding.children[0].field.as_deref(), Some("pattern"));
    assert!(binding.children[1].missing && binding.has_error());
    assert!(Tree::read("(file [0, 0] - [1").is_err());
}

#[test]
fn the_config_names_the_grammar_and_its_files() {
    let config = vun_data::ToData::to_data(&Config::of("vun", "vun", "queries/highlights.scm", "1"));
    assert_eq!(config.pointer("/grammars/0/file-types/0").and_then(|v| v.as_str()), Some("vun"));
    assert_eq!(config.pointer("/grammars/0/scope").and_then(|v| v.as_str()), Some("source.vun"));
}
