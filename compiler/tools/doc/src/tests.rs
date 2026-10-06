//! A namespace documented from its source: what is public, how signatures read, and where doc
//! comments link.

use vun_documenter::{Documenter, Source, Types};

use crate::{Docs, Html, Item, Kind};

const FILE: &str = "\
//! Points and colors.
//!
//! See `Point` and `std.other`.

/// A point.
#[derive]
Point := struct {
    /// Across.
    x: u8 = 0;
    _hidden: u8 = 0;
    /// The sum of `x` and nothing.
    sum := (self: Point) -> u8 { self.x };
};

/// Colors.
Color := enum { red, green(u8) };

Make := ($T: type) -> type { struct { v: T; } };

#[link.symbol(\"f\")]
f: unsafe (a: i32) -> u8 = _;

_private := 1;
";

fn docs() -> Docs {
    let mut docs = Docs::default();
    docs.add(vec!["pkg".into()], "pkg.vun".into(), FILE);
    docs.add(vec!["std".into(), "other".into()], "std/other.vun".into(), "//! Other.\n");
    docs
}

fn names(items: &[Item]) -> Vec<(&str, Kind, &str)> {
    items.iter().map(|i| (i.name.as_str(), i.kind, i.signature.as_str())).collect()
}

#[test]
fn public_declarations_are_read_with_their_kinds_and_signatures() {
    let docs = docs();
    let namespace = &docs.namespaces[&vec!["pkg".to_owned()]];
    assert_eq!(namespace.doc, ["Points and colors.", "", "See `Point` and `std.other`."]);
    assert_eq!(
        names(&namespace.items),
        [
            ("Point", Kind::Struct, "Point := struct { ... }"),
            ("Color", Kind::Enum, "Color := enum { ... }"),
            ("Make", Kind::Struct, "Make := ($T: type) -> type { struct { ... } }"),
            ("f", Kind::Function, "f: unsafe (a: i32) -> u8 = _"),
        ]
    );
    let point = &namespace.items[0];
    assert_eq!(point.doc, ["A point."]);
    assert_eq!(point.attributes, ["#[derive]"]);
    assert_eq!(
        names(&point.members),
        [("x", Kind::Field, "x: u8 = 0"), ("sum", Kind::Function, "sum := (self: Point) -> u8")]
    );
    assert_eq!(
        names(&namespace.items[1].members),
        [("red", Kind::Variant, "red"), ("green", Kind::Variant, "green(u8)")]
    );
}

#[test]
fn doc_comments_link_declarations_by_path_or_relative_to_where_they_are() {
    let docs = docs();
    let links = crate::links::Links::of(&docs);
    assert_eq!(links.resolve(&["pkg"], "Point"), Some("pkg.html#Point"));
    assert_eq!(links.resolve(&["pkg"], "std.other"), Some("std.other.html"));
    assert_eq!(links.resolve(&["pkg", "Point"], "x"), Some("pkg.html#Point.x"));
    assert_eq!(links.resolve(&["pkg", "Point"], "sum()"), Some("pkg.html#Point.sum"));
    assert_eq!(links.resolve(&["pkg"], "x"), None, "a member is named through its type");
}

#[test]
fn the_site_has_a_page_per_namespace_an_index_a_stylesheet_and_the_json() {
    let site = docs().site();
    let paths: Vec<&str> = site.iter().map(|w| w.path.as_str()).collect();
    assert_eq!(paths, ["pkg.html", "std.other.html", "index.html", "style.css", "docs.json"]);
    let json = vun_data::json::parse(&site[4].contents).unwrap();
    assert_eq!(json["namespaces"][0]["items"][0]["members"][1]["name"].as_str(), Some("sum"));
}

/// Says that `Make` is a type.
struct Told;

impl Types for Told {
    fn is_type(&self, _: &[String], name: &str) -> bool {
        name == "Make"
    }
}

#[test]
fn the_documenter_retypes_what_the_host_says_is_a_type_and_writes_the_site() {
    let source =
        Source { path: vec!["points".to_owned()], file: "points.vun".to_owned(), text: FILE.to_owned() };
    let site = Html.document(&[source], &Told).unwrap();
    assert_eq!(site.namespaces, 1);
    let paths: Vec<_> = site.files.iter().map(|w| w.path.as_str()).collect();
    assert!(paths.contains(&"index.html") && paths.iter().any(|p| p.ends_with("points.html")), "{paths:?}");
}
