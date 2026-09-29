use super::Lua;
use crate::{Object, Value};

#[test]
fn tables_name_fields_and_list_items() {
    let value = Value::Object(
        Object::new()
            .with("cmd", Value::Array(vec!["vun".into(), "lsp".into()]))
            .with("end", true)
            .with("file-types", Value::Array(vec![]))
            .with("absent", Value::Null),
    );
    assert_eq!(Lua(&value).to_string(), r#"{ cmd = { "vun", "lsp" }, ["end"] = true, ["file-types"] = {} }"#);
    assert_eq!(
        format!("{:#}", Lua(&value)),
        "{\n  cmd = {\n    \"vun\",\n    \"lsp\",\n  },\n  [\"end\"] = true,\n  [\"file-types\"] = {},\n}"
    );
}
