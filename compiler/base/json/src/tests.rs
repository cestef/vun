use std::borrow::Cow;

use crate::{Error, ErrorKind, Number, Object, Parser, Value, parse};

/// Texts RFC 8259 makes JSON, in the manner of the `JSONTestSuite`'s `y_` cases.
const ACCEPTED: &[&str] = &[
    "null",
    "true",
    "false",
    "0",
    "-0",
    "1",
    "-1",
    "1.5",
    "0.25e-3",
    "1E+2",
    "1e2",
    "-0.0e0",
    "123456789012345678901234567890",
    "\"\"",
    "\"a\"",
    "[]",
    "{}",
    "[[]]",
    " [ 1 , 2 ] ",
    "{\"a\":{\"b\":[null]}}",
    "\"\\u0000\"",
    "\"\\\"\\\\\\/\\b\\f\\n\\r\\t\"",
    "\"\\ud83d\\ude00\"",
    "\"\u{1F600}\"",
    "\"\u{7f}\"",
    "{\"a\":1,\"a\":2}",
    "\u{feff}[1]",
    "\t\n\r 1 \t\n\r",
];

/// Texts that are not, as the `n_` cases.
const REJECTED: &[(&str, ErrorKind)] = &[
    ("", ErrorKind::UnexpectedEnd),
    (" ", ErrorKind::UnexpectedEnd),
    ("[1,]", ErrorKind::Unexpected),
    ("{\"a\":1,}", ErrorKind::Unexpected),
    ("[1 2]", ErrorKind::Unexpected),
    ("{\"a\" 1}", ErrorKind::Unexpected),
    ("{a:1}", ErrorKind::Unexpected),
    ("{1:1}", ErrorKind::Unexpected),
    ("[", ErrorKind::UnexpectedEnd),
    ("[1", ErrorKind::UnexpectedEnd),
    ("\"abc", ErrorKind::UnexpectedEnd),
    ("01", ErrorKind::Trailing),
    ("-", ErrorKind::InvalidNumber),
    ("1.", ErrorKind::InvalidNumber),
    (".5", ErrorKind::Unexpected),
    ("1e", ErrorKind::InvalidNumber),
    ("+1", ErrorKind::Unexpected),
    ("0x10", ErrorKind::Trailing),
    ("NaN", ErrorKind::Unexpected),
    ("tru", ErrorKind::Unexpected),
    ("nul", ErrorKind::Unexpected),
    ("[1]]", ErrorKind::Trailing),
    ("1 2", ErrorKind::Trailing),
    ("\"\t\"", ErrorKind::ControlInString),
    ("\"a\nb\"", ErrorKind::ControlInString),
    ("\"\\x\"", ErrorKind::InvalidEscape),
    ("\"\\u12zz\"", ErrorKind::InvalidEscape),
    ("\"\\u12", ErrorKind::UnexpectedEnd),
    ("\"\\ud800\"", ErrorKind::LoneSurrogate),
    ("\"\\udc00\"", ErrorKind::LoneSurrogate),
    ("\"\\ud800\\u0041\"", ErrorKind::LoneSurrogate),
    ("[1]\u{0}", ErrorKind::Trailing),
    ("\u{a0}1", ErrorKind::Unexpected),
    ("// c\n1", ErrorKind::Unexpected),
    ("['a']", ErrorKind::Unexpected),
];

#[test]
fn the_grammar_is_read_exactly() {
    for text in ACCEPTED {
        assert!(parse(text).is_ok(), "{text:?}: {:?}", parse(text));
    }
    for (text, kind) in REJECTED {
        assert_eq!(parse(text).map_err(|e| e.kind), Err(*kind), "{text:?}");
    }
}

#[test]
fn values_read_as_written() {
    let doc =
        parse(r#"{"s":"a\u00e9\ud83d\ude00","n":-12,"f":2.5,"b":true,"z":null,"xs":[1,{"k":[]}]}"#).unwrap();
    assert_eq!(doc["s"].as_str(), Some("aé\u{1F600}"));
    assert_eq!(doc["n"].as_i64(), Some(-12));
    assert_eq!(doc["n"].as_u64(), None);
    assert_eq!(doc["f"].as_f64(), Some(2.5));
    assert_eq!(doc["f"].as_i64(), None);
    assert_eq!(doc["b"].as_bool(), Some(true));
    assert!(doc["z"].is_null() && doc["missing"]["deeper"][3].is_null());
    assert_eq!(doc["xs"][1]["k"].as_array().map(<[Value]>::len), Some(0));
    assert_eq!(doc["xs"].items().count(), 2);
    assert_eq!(doc.pointer("/xs/1/k"), Some(&Value::Array(Vec::new())));
    assert_eq!(doc.pointer("/xs/01"), None, "an index has no leading zero");
    assert_eq!(doc.pointer(""), Some(&doc));
}

#[test]
fn strings_without_escapes_borrow_from_the_text() {
    let doc = parse(r#"["plain","esc\n"]"#).unwrap();
    assert!(matches!(doc[0], Value::String(Cow::Borrowed("plain"))));
    assert!(matches!(doc[1], Value::String(Cow::Owned(_))));
    let owned: Value<'static> = doc.into_owned();
    assert_eq!(owned[1].as_str(), Some("esc\n"));
}

#[test]
fn numbers_keep_their_digits() {
    let doc = parse("[18446744073709551615, 18446744073709551616, 1e400, 0.1]").unwrap();
    assert_eq!(doc[0].as_u64(), Some(u64::MAX));
    assert_eq!(doc[1].as_u64(), None);
    assert_eq!(doc[1].as_number().map(Number::as_str), Some("18446744073709551616"));
    assert_eq!(doc[2].as_f64(), Some(f64::INFINITY));
    assert_eq!(doc[3].as_f64(), Some(0.1));
    assert_eq!(Number::from_f64(f64::NAN), None);
    assert_eq!(Number::from_f64(0.1).map(|n| n.as_str().to_owned()), Some("0.1".to_owned()));
    assert_eq!(Number::parse("1e5").map(|n| n.is_integer()), Some(false));
    assert!(Number::parse("01").is_none());
}

#[test]
fn duplicate_names_are_kept_and_the_last_answers() {
    let doc = parse(r#"{"a":1,"a":2}"#).unwrap();
    assert_eq!(doc["a"].as_u64(), Some(2));
    assert_eq!(doc.as_object().map(Object::len), Some(2));
}

#[test]
fn nesting_is_bounded_by_the_limit() {
    let nested = |n: usize| "[".repeat(n) + &"]".repeat(n);
    let limit = Parser::default().max_depth;
    assert_eq!(parse(&nested(limit + 1)).map_err(|e| e.kind), Err(ErrorKind::TooDeep));
    let text = nested(limit);
    let deepest = parse(&text).unwrap();
    assert_eq!(parse(&deepest.to_string()).unwrap(), deepest);
    let over = "[".repeat(5) + &"]".repeat(5);
    assert_eq!(Parser { max_depth: 4 }.parse(&over).map_err(|e| e.offset), Err(4));
}

#[test]
fn errors_say_where() {
    let text = "{\n  \"a\": [1,\n  ]\n}";
    let error = parse(text).unwrap_err();
    assert_eq!(error, Error { kind: ErrorKind::Unexpected, offset: 15 });
    assert_eq!(error.line_col(text), (3, 3));
    assert_eq!(error.to_string(), "unexpected character at byte 15");
}

#[test]
fn writing_escapes_what_it_must_and_reads_back() {
    let value = Value::from(
        Object::new()
            .with("text", "q\"b\\n\n\u{1}\u{7f}é")
            .with("xs", vec![Value::from(1), Value::Null, Value::from(false)])
            .with("empty", Object::new())
            .with("none", Option::<u8>::None),
    );
    let compact = value.to_string();
    assert_eq!(
        compact,
        r#"{"text":"q\"b\\n\n\u0001é","xs":[1,null,false],"empty":{},"none":null}"#
            .replace('\u{1}', "")
            .replace("\\u0001é", "\\u0001\u{7f}é")
    );
    assert_eq!(parse(&compact).unwrap(), value);
    let pretty = format!("{value:#}");
    assert_eq!(parse(&pretty).unwrap(), value);
    assert!(pretty.starts_with("{\n  \"text\": "), "{pretty}");
    assert!(pretty.contains("\"xs\": [\n    1,\n    null,\n    false\n  ],"), "{pretty}");
}

#[test]
fn every_accepted_text_round_trips() {
    for text in ACCEPTED {
        let value = parse(text).unwrap();
        assert_eq!(parse(&value.to_string()).unwrap(), value, "{text:?}");
        assert_eq!(parse(&format!("{value:#}")).unwrap(), value, "{text:?}");
    }
}
