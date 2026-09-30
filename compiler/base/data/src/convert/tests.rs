use crate::json::{Json, parse};
use crate::{Expected, FromData, Mismatch, Object, Step, ToData, Value};

#[derive(Debug, PartialEq, ToData, FromData)]
#[data(case = camel)]
struct Position {
    line: u32,
    character_offset: u32,
}

#[derive(Debug, PartialEq, ToData, FromData)]
#[data(case = camel)]
struct At {
    text_document: Document,
    #[data(flatten)]
    position: Position,
    #[data(name = "$kind")]
    kind: Option<Kind>,
}

#[derive(Debug, PartialEq, ToData, FromData)]
struct Document {
    uri: String,
}

#[derive(Debug, PartialEq, ToData, FromData)]
#[data(case = camel)]
enum Kind {
    PlainText,
    #[data(name = "md")]
    Markdown,
}

#[derive(Debug, PartialEq, ToData, FromData)]
#[data(number)]
enum Severity {
    Error = 1,
    Warning = 2,
}

#[derive(Debug, PartialEq, ToData, FromData)]
#[data(untagged)]
enum Target {
    One(Document),
    Many(Vec<Document>),
    Nothing,
}

#[derive(Debug, PartialEq, ToData, FromData)]
struct Pair(u8, String);

#[derive(Debug, PartialEq, ToData, FromData)]
enum Line {
    Test { name: String, passed: bool },
    Pair(u8, String),
    One(u8),
    Totals,
}

fn document(uri: &str) -> Document {
    Document { uri: uri.to_owned() }
}

fn round_trip<T: ToData + FromData + PartialEq + std::fmt::Debug>(value: &T, text: &str) {
    let written = value.to_data();
    assert_eq!(written, parse(text).unwrap(), "{}", Json(&written));
    assert_eq!(&written.read::<T>().unwrap(), value);
}

#[test]
fn structs_are_objects_cased_flattened_and_without_absent_options() {
    let position = Position { line: 1, character_offset: 2 };
    let at = At { text_document: document("a"), position, kind: None };
    round_trip(&at, r#"{"textDocument":{"uri":"a"},"line":1,"characterOffset":2}"#);
    let at = At { kind: Some(Kind::Markdown), ..at };
    round_trip(&at, r#"{"textDocument":{"uri":"a"},"line":1,"characterOffset":2,"$kind":"md"}"#);
}

#[test]
fn enums_are_names_numbers_or_their_variants_values() {
    round_trip(&Kind::PlainText, r#""plainText""#);
    round_trip(&Severity::Warning, "2");
    round_trip(&Target::One(document("a")), r#"{"uri":"a"}"#);
    round_trip(&Target::Many(vec![document("a")]), r#"[{"uri":"a"}]"#);
    round_trip(&Target::Nothing, "null");
    round_trip(&Pair(3, "x".to_owned()), r#"[3,"x"]"#);
}

#[test]
fn enums_with_fields_are_tagged_by_their_variant() {
    round_trip(&Line::Test { name: "a".to_owned(), passed: true }, r#"{"test":{"name":"a","passed":true}}"#);
    round_trip(&Line::Pair(1, "x".to_owned()), r#"{"pair":[1,"x"]}"#);
    round_trip(&Line::One(2), r#"{"one":2}"#);
    round_trip(&Line::Totals, r#""totals""#);
    let wrong = parse(r#"{"test":{"name":"a","passed":1}}"#).unwrap().read::<Line>().unwrap_err();
    let path: Vec<Step> = wrong.path().collect();
    assert_eq!((wrong.expected, path), (Expected::Bool, vec![Step::Member("test"), Step::Member("passed")]));
    let two = parse(r#"{"one":2,"totals":null}"#).unwrap().read::<Line>().unwrap_err();
    assert_eq!(two.expected, Expected::Form);
}

#[test]
fn mismatches_name_what_was_expected_where() {
    let read = |text: &str| parse(text).unwrap().read::<At>().unwrap_err();
    let wrong = read(r#"{"textDocument":{"uri":3},"line":1,"characterOffset":2}"#);
    let path: Vec<Step> = wrong.path().collect();
    assert_eq!(
        (wrong.expected, path),
        (Expected::String, vec![Step::Member("textDocument"), Step::Member("uri")])
    );
    assert_eq!(read(r#"{"textDocument":{"uri":"a"},"line":1}"#).expected, Expected::Member);
    assert_eq!(
        read(r#"{"textDocument":{"uri":"a"},"line":-1,"characterOffset":2}"#).expected,
        Expected::Integer
    );
    let kind = parse(r#""html""#).unwrap().read::<Kind>().unwrap_err();
    assert_eq!(kind, Mismatch::new(Expected::OneOf(&["plainText", "md"])));
    assert_eq!(Value::from(7).read::<Target>().unwrap_err().expected, Expected::Form);
    let pair = parse("[1,2]").unwrap().read::<Pair>().unwrap_err();
    assert_eq!((pair.expected, pair.path().collect::<Vec<_>>()), (Expected::String, vec![Step::Item(1)]));
    assert_eq!(Object::new().read::<Option<u8>>("absent"), Ok(None));
}
