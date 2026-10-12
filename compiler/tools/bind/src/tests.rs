use std::collections::HashMap;

use crate::macros::{Names, Value};

fn words(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_owned).collect()
}

fn value(macros: &[(&str, &str)], constants: &[(&str, i128)], text: &str) -> Option<Value> {
    let macros: HashMap<String, Vec<String>> =
        macros.iter().map(|&(name, text)| (name.to_owned(), words(text))).collect();
    let constants: HashMap<String, i128> = constants.iter().map(|&(name, n)| (name.to_owned(), n)).collect();
    Names { macros: &macros, constants: &constants }.value(&words(text))
}

fn int(text: &str) -> Option<Value> {
    value(&[], &[], text)
}

#[test]
fn integer_macros_read_in_every_form_c_writes() {
    assert_eq!(int("( 0x10U )"), Some(Value::Int(16)));
    assert_eq!(int("- 1"), Some(Value::Int(-1)));
    assert_eq!(int("010"), Some(Value::Int(8)));
    assert_eq!(int("0b101"), Some(Value::Int(5)));
    assert_eq!(int("1'000ULL"), Some(Value::Int(1000)));
    assert_eq!(int("'a'"), Some(Value::Int(97)));
    assert_eq!(int("A + 1"), None);
    assert_eq!(int(""), None);
}

#[test]
fn operators_bind_as_they_do_in_c() {
    assert_eq!(int("1 + 2 * 3"), Some(Value::Int(7)));
    assert_eq!(int("( 1 + 2 ) * 3"), Some(Value::Int(9)));
    assert_eq!(int("1 << 4 | 2"), Some(Value::Int(18)));
    assert_eq!(int("0xff & 0x0f ^ 3"), Some(Value::Int(12)));
    assert_eq!(int("1 << 200"), None);
    assert_eq!(int("1 / 0"), None);
}

#[test]
fn floats_and_strings_are_constants_too() {
    assert_eq!(int("1.5"), Some(Value::Float(1.5)));
    assert_eq!(int("2.5f"), Some(Value::Float(2.5)));
    assert_eq!(int("1e3"), Some(Value::Float(1000.0)));
    assert_eq!(int("- 0.5"), Some(Value::Float(-0.5)));
    assert_eq!(int("\"gtk\""), Some(Value::Text("gtk".to_owned())));
    assert_eq!(int("\"a\" \"b\\n\""), Some(Value::Text("ab\n".to_owned())));
    assert_eq!(int("\"a\\0b\""), None);
    assert_eq!(int("\"a\" + 1"), None);
}

#[test]
fn a_macro_may_name_other_macros_and_enum_constants() {
    assert_eq!(value(&[("A", "4")], &[], "A + 1"), Some(Value::Int(5)));
    assert_eq!(value(&[("A", "B"), ("B", "( 2 )")], &[], "A * A"), Some(Value::Int(4)));
    assert_eq!(value(&[], &[("FILL", 3)], "FILL | 4"), Some(Value::Int(7)));
    assert_eq!(value(&[("A", "A")], &[], "A"), None);
    assert_eq!(value(&[("A", "f ( 1 )")], &[], "A"), None);
}
