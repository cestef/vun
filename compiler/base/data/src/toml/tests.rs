use super::Toml;
use crate::{Object, Value};

#[test]
fn plain_members_come_before_tables_and_arrays_of_tables() {
    let server =
        Object::new().with("command", "vun").with("args", Value::Array(vec!["util".into(), "lsp".into()]));
    let language = Object::new().with("name", "vun").with("indent", Object::new().with("unit", "    "));
    let root = Object::new()
        .with("servers", Object::new().with("vun-lsp", server))
        .with("language", Value::Array(vec![Value::Object(language)]))
        .with("version", 1)
        .with("absent", Value::Null);
    assert_eq!(
        Toml(&root).to_string(),
        "version = 1\n\n[servers.vun-lsp]\ncommand = \"vun\"\nargs = [\"util\", \"lsp\"]\n\n[[language]]\nname = \"vun\"\n\n[language.indent]\nunit = \"    \"\n"
    );
}

#[test]
fn keys_are_quoted_and_objects_among_values_inline() {
    let root = Object::new()
        .with("a b", true)
        .with("pairs", Value::Array(vec![Value::Object(Object::new().with("open", "(")), "x".into()]));
    assert_eq!(Toml(&root).to_string(), "\"a b\" = true\npairs = [{ open = \"(\" }, \"x\"]\n");
}
