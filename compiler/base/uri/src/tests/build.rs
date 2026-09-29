use std::net::Ipv6Addr;

use crate::{HostKind, Invalid, Reference, Uri, Why};

#[test]
fn components_are_escaped_as_each_requires() {
    let uri = Uri::builder("https")
        .userinfo("me@home")
        .host("example.org")
        .port(8443)
        .segment("a b")
        .segment("c/d")
        .pair("q", "x&y=z")
        .pair("n", "1")
        .fragment("f g")
        .build()
        .unwrap();
    assert_eq!(uri.as_str(), "https://me%40home@example.org:8443/a%20b/c%2Fd?q=x%26y%3Dz&n=1#f%20g");
    uri.verify().unwrap();
    assert_eq!(uri.query().unwrap().get("q").unwrap().decode_utf8().unwrap(), "x&y=z");
}

#[test]
fn hosts_are_read_as_addresses_when_they_are() {
    let uri = Uri::builder("http").host("::1").build().unwrap();
    let host = uri.host().unwrap();
    assert_eq!(
        (uri.as_str(), host.kind(), host.ip()),
        ("http://[::1]", HostKind::Ipv6, Some(Ipv6Addr::LOCALHOST.into()))
    );
    uri.verify().unwrap();
}

#[test]
fn what_cannot_be_built_is_reported() {
    assert_eq!(Uri::builder("1x").build(), Err(Invalid::new(Why::Scheme, 0)));
    assert_eq!(Uri::builder("a b").build(), Err(Invalid::new(Why::Char(crate::Set::Scheme), 1)));
    let relative = Reference::builder().path("a:b").build().unwrap();
    assert_eq!(relative.as_str(), "./a:b");
    relative.verify().unwrap();
    let pathless = Reference::builder().host("h").path("x").build();
    assert_eq!(pathless, Err(Invalid::new(Why::AuthorityPath, 0)));
}

#[test]
fn segments_extend_a_path() {
    let base: Uri = "https://index.example/v1/?token=x#top".parse().unwrap();
    let listing = base.with_segments(["7", "1", "a b"]);
    assert_eq!(listing.as_str(), "https://index.example/v1/7/1/a%20b?token=x");
    listing.verify().unwrap();
    let file: Uri = "file:///srv".parse().unwrap();
    assert_eq!(file.with_segments(["list"]).as_str(), "file:///srv/list");
}
