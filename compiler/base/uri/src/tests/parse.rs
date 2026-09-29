use std::net::{Ipv4Addr, Ipv6Addr};

use crate::{Encoded, HostKind, Invalid, Reference, Set, Uri, Why};

#[test]
fn components_are_split_and_typed() {
    let uri: Uri = "HTTPS://user:pw@Example.org:8080/a/b%20c?x=1&y#top".parse().unwrap();
    assert!(uri.scheme().is("https"));
    let authority = uri.authority().unwrap();
    assert_eq!(authority.userinfo().map(Encoded::as_str), Some("user:pw"));
    assert_eq!((authority.host().as_str(), authority.host().kind()), ("Example.org", HostKind::Name));
    assert_eq!(authority.port(), Some(8080));
    let segments: Vec<_> = uri.path().segments().map(|s| s.decode_utf8().unwrap().into_owned()).collect();
    assert_eq!(segments, ["a", "b c"]);
    let query = uri.query().unwrap();
    assert_eq!(query.get("x").map(Encoded::as_str), Some("1"));
    assert_eq!(query.pairs().count(), 2);
    assert_eq!(uri.fragment().map(Encoded::as_str), Some("top"));
    assert_eq!(uri.without_fragment().as_str(), "HTTPS://user:pw@Example.org:8080/a/b%20c?x=1&y");
    uri.verify().unwrap();
}

#[test]
fn hosts_are_names_or_addresses() {
    let host = |text: &str| text.parse::<Uri>().unwrap().host().unwrap().kind();
    let ip = |text: &str| text.parse::<Uri>().unwrap().host().unwrap().ip();
    assert_eq!(
        (host("http://127.0.0.1/"), ip("http://127.0.0.1/")),
        (HostKind::Ipv4, Some(Ipv4Addr::LOCALHOST.into()))
    );
    assert_eq!(
        (host("http://[::1]:80/"), ip("http://[::1]:80/")),
        (HostKind::Ipv6, Some(Ipv6Addr::LOCALHOST.into()))
    );
    assert_eq!(host("http://[v7.x:y]/"), HostKind::Future);
    // Not dotted decimal: a registered name.
    assert_eq!(host("http://127.0.0.01/"), HostKind::Name);
    assert_eq!(host("file:///x"), HostKind::Name);
}

#[test]
fn references_may_be_relative_where_uris_may_not() {
    let reference: Reference = "../a?b".parse().unwrap();
    assert!(reference.is_relative());
    assert_eq!(reference.path().as_str(), "../a");
    assert_eq!("../a".parse::<Uri>(), Err(Invalid::new(Why::Relative, 0)));
    assert_eq!(Reference::parse("").map(|r| r.path().is_empty()), Ok(true));
    let uri: Uri = "urn:isbn:0451450523".parse().unwrap();
    assert_eq!((uri.path().as_str(), uri.authority()), ("isbn:0451450523", None));
}

#[test]
fn malformed_text_is_reported_where_it_goes_wrong() {
    let why = |text: &str| Reference::parse(text).unwrap_err();
    assert_eq!(why("1a:b"), Invalid::new(Why::Scheme, 0));
    assert_eq!(why("a b"), Invalid::new(Why::Char(Set::Path), 1));
    assert_eq!(why("/a%2"), Invalid::new(Why::Escape, 2));
    assert_eq!(why("http://h:99999/"), Invalid::new(Why::Port, 9));
    assert_eq!(why("http://h:8a/"), Invalid::new(Why::Port, 9));
    assert_eq!(why("http://[::1/"), Invalid::new(Why::Bracket, 7));
    assert_eq!(why("http://[::1]x/"), Invalid::new(Why::Bracket, 12));
    assert_eq!(why("http://[zz]/"), Invalid::new(Why::IpLiteral, 7));
    assert_eq!(why("http://a b/"), Invalid::new(Why::Char(Set::Host), 8));
    assert_eq!(why("?a#b#c"), Invalid::new(Why::Char(Set::Fragment), 4));
    assert_eq!(why("//u@s@h/"), Invalid::new(Why::Char(Set::UserInfo), 3));
}

#[test]
fn escapes_decode_on_request() {
    let uri: Uri = "x:/%E2%82%AC%ff".parse().unwrap();
    assert_eq!(&*uri.path().encoded().decode(), b"/\xE2\x82\xAC\xFF");
    assert!(uri.path().encoded().decode_utf8().is_err());
    assert_eq!(Set::Segment.encode(b"a/b c"), "a%2Fb%20c");
    assert_eq!(Set::Pair.encode(b"x&y=z+"), "x%26y%3Dz%2B");
}

#[test]
fn references_are_json_strings() {
    let uri: Uri = "https://example.org/".parse().unwrap();
    let json = vun_json::ToJson::to_json(&uri);
    assert_eq!(json.as_str(), Some("https://example.org/"));
    assert_eq!(json.read::<Uri>().unwrap(), uri);
    let relative = vun_json::Value::from("a b");
    assert_eq!(relative.read::<Uri>().unwrap_err().expected, vun_json::Expected::Syntax("a URI"));
}

#[test]
fn a_uri_stays_small_enough_to_move() {
    assert!(size_of::<Uri>() <= 112, "{} bytes", size_of::<Uri>());
}
