use vun_diag::{DiagCtxt, DiagCx, DiagId, errors};
use vun_fs::Tree;
use vun_harness::repo_root;
use vun_origin::OriginTable;
use vun_span::{EXTENSION, FileId, SourceFiles};
use vun_syntax::cst::Cst;
use vun_syntax::literal::Decl;
use vun_syntax::tokens::Tokens;

use super::{DepKind, Identity, Manifest, Source, Version};
use crate::layout::MANIFEST;

fn parsed<R>(text: &str, f: impl FnOnce(&[Decl<'_>], FileId, &mut DiagCx<'_>) -> R) -> (R, Vec<DiagId>) {
    let file = SourceFiles::new().add(MANIFEST, text);
    let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
    let mut cx = DiagCx { diag: &mut diag, origins: &origins };
    let tokens = Tokens::lex(file, text, &mut cx);
    let cst = Cst::parse(&tokens, text, &mut cx);
    let result = f(&cst.declarations(&tokens, text), file, &mut cx);
    (result, diag.sorted(&origins).into_iter().map(|d| d.id.clone()).collect())
}

fn declares(text: &str) -> bool {
    parsed(text, |decls, _, _| Manifest::declared(decls)).0
}

fn manifest(text: &str) -> (Option<Manifest>, Vec<DiagId>) {
    parsed(text, Manifest::read)
}

#[test]
fn every_manifest_reads() {
    let root = repo_root();
    let mut read = 0;
    for dir in ["lib", "corpus"] {
        for path in root.join(dir).files_with(EXTENSION).unwrap() {
            let text = std::fs::read_to_string(&path).unwrap();
            if path.file_name().is_some_and(|n| n == MANIFEST) && declares(&text) {
                let (read_once, ids) = manifest(&text);
                assert!(read_once.is_some() && ids.is_empty(), "{}: {ids:?}", path.display());
                // What the writer writes reads back as the same manifest.
                let written = read_once.unwrap().source().unwrap();
                let (again, ids) = manifest(&written);
                assert_eq!(
                    (again.unwrap().source().unwrap(), ids),
                    (written, Vec::new()),
                    "{}",
                    path.display()
                );
                read += 1;
            }
        }
    }
    assert_eq!(read, 11, "manifests found");
}

#[test]
fn reads_every_field() {
    let text = r#"package := .{
        .name = "blinky", .id = 0x6a80, .version = "0.3.1", .edition = 2026, .toolchain = "0.4.2",
        .include = .{ "build.vun", "blinky" }, .prelude = .{ "std.prelude", "support" },
        .deps = .{
            .hal = .{ .version = "1.3.0", .hash = "blake3:k3f2aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", .urls = .{ "https://x" } },
            .cmsis = .{ .version = "5.9.0", .hash = "blake3:m7qdaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", .kind = .foreign },
            .util = .{ .path = "../util" },
        },
        .replace = .{ .hal = .{ .id = 0xc0ffee01, .major = 1, .path = "../hal-fork" } },
    };"#;
    let (Some(m), ids) = manifest(text) else { panic!("unread") };
    assert!(ids.is_empty(), "{ids:?}");
    assert_eq!((m.name.as_str(), m.id, m.edition), ("blinky", 0x6a80, 2026));
    assert_eq!(m.toolchain, Version::parse("0.4.2"));
    assert_eq!(m.include, ["build.vun", "blinky"]);
    assert_eq!(m.prelude, ["std.prelude", "support"]);
    let deps: Vec<_> = m.deps.iter().map(|d| (d.name.as_str(), d.kind)).collect();
    assert_eq!(deps, [("hal", DepKind::Vun), ("cmsis", DepKind::Foreign), ("util", DepKind::Vun)]);
    assert_eq!(m.deps[2].source, Source::Path("../util".into()));
    let [hal] = m.replace.as_slice() else { panic!("{:?}", m.replace) };
    assert_eq!(
        (hal.identity, &hal.source),
        (Identity { id: 0xc0ff_ee01, major: 1 }, &Source::Path("../hal-fork".into()))
    );
    let written = m.source().unwrap();
    let again = manifest(&written).0.unwrap();
    assert_eq!(
        (again.replace[0].identity, &again.replace[0].source),
        (hal.identity, &hal.source),
        "{written}"
    );
}

#[test]
fn reports_where_the_manifest_is_wrong() {
    let cases: &[(&str, &[DiagId])] = &[
        ("build := 1;", &[errors::MANIFEST_MISSING_PACKAGE]),
        ("package := 1;", &[errors::MANIFEST_INVALID_FIELD]),
        (
            r#"package := .{ .name = "a", .id = 1, .version = "1.0.0", .edition = 1, .include = .{ "build.vun" },
                .deps = .{ .x = .{ .version = "1.0.0", .hash = "blake3-m7qd" } } };"#,
            &[errors::MANIFEST_INVALID_FIELD],
        ),
        (
            r#"package := .{ .name = "a", .id = 1, .version = "1.0", .edition = 1, .include = .{ "build.vun" } };"#,
            &[errors::MANIFEST_INVALID_FIELD],
        ),
        (
            r#"package := .{ .name = "a", .id = -1, .version = "1.0.0", .edition = 1, .include = .{ "build.vun" }, .dpes = .{} };"#,
            &[errors::MANIFEST_INVALID_FIELD, errors::MANIFEST_UNKNOWN_FIELD],
        ),
        (
            r#"package := .{ .nme = "a", .id = 1, .version = "1.0.0", .edition = 1, .include = .{ "build.vun" } };"#,
            &[errors::MANIFEST_MISSING_FIELD, errors::MANIFEST_UNKNOWN_FIELD],
        ),
        (
            r#"package := .{ .name = "a", .id = 1, .version = "1.0.0", .edition = 1, .include = .{ "build.vun" },
                .deps = .{ .x = .{ .path = "p", .version = "1.0.0" }, .y = .{ .version = "1.0.0" } } };"#,
            &[errors::MANIFEST_INVALID_FIELD, errors::MANIFEST_MISSING_FIELD],
        ),
        (
            r#"package := .{ .name = "a", .id = 1, .version = "1.0.0", .edition = 1, .include = .{ "src" } };"#,
            &[errors::MANIFEST_INVALID_FIELD],
        ),
        (
            r#"package := .{ .name = "a", .id = 1, .version = "1.0.0", .edition = 1,
                .include = .{ "build.vun", "../secrets" } };"#,
            &[errors::MANIFEST_INVALID_FIELD],
        ),
    ];
    for &(text, expected) in cases {
        assert_eq!(manifest(text).1, expected, "{text}");
    }
}

#[test]
fn declared_deps_read_back() {
    let hash = vun_hash::Hash::of(b"hal");
    let archive = Source::Archive {
        version: Version::parse("1.2.0").unwrap(),
        hash,
        urls: vec!["https://example.org/hal-1.2.0.tar.zst".parse().unwrap()],
    };
    let wanted = [("util", Source::Path("../util".into()), DepKind::Vun), ("hal", archive, DepKind::Foreign)];
    let mut text =
        "package := .{\n    .name = \"app\", .id = 1, .version = \"0.1.0\", .edition = 2026,\n    \
                    .include = .{ \"build.vun\" },\n};\n"
            .to_owned();
    for (name, source, kind) in &wanted {
        let edit = parsed(&text, |decls, file, _| {
            let dep = super::Dep {
                name: (*name).into(),
                source: source.clone(),
                kind: *kind,
                span: vun_span::Span::new(file, 0, 0),
            };
            dep.declare(decls, &text).unwrap()
        })
        .0;
        text = vun_diag::Edit::apply(&[edit], &text).unwrap();
    }
    let (manifest, ids) = manifest(&text);
    assert!(ids.is_empty(), "{ids:?}\n{text}");
    let deps: Vec<_> = manifest.unwrap().deps.into_iter().map(|d| (d.name, d.source, d.kind)).collect();
    let wanted: Vec<_> = wanted.into_iter().map(|(n, s, k)| (n.to_owned(), s, k)).collect();
    assert_eq!(deps, wanted, "{text}");
}
