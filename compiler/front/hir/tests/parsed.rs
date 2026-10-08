//! A parsed file saved and loaded in another database is the tree that was saved.

use std::sync::Arc;

use vun_hir::db::{FileRef, FileText, PERSISTED, Parsed};
use vun_intern::Symbol;
use vun_origin::OriginTable;
use vun_query::{Db, Event};
use vun_span::PackageId;

const TEXT: &str =
    "limit := (a: u32) -> u32 { a + 1 };\nname := \"a\\n\";\nbig := 1_000_000_000_000_000_000_000;\n";

fn database() -> (Db, FileRef) {
    let db = Db::new();
    for persist in PERSISTED {
        persist(&db);
    }
    let key = FileRef { package: PackageId::Loose, path: Symbol::intern("saved.vun") };
    db.share::<FileText>(&key, Some(Arc::from(TEXT)));
    (db, key)
}

#[test]
fn a_parsed_file_is_read_back_whole() {
    let (first, key) = database();
    let parsed = first.get::<Parsed>(&key).unwrap();
    let saved = first.save();
    let (second, key) = database();
    second.load(saved).unwrap();
    let read = second.get::<Parsed>(&key).unwrap();
    assert_eq!(second.tally().count("parsed", Event::ValueRead), 1);
    assert_eq!(
        (read.hir.exprs.len(), read.hir.decls.len()),
        (parsed.hir.exprs.len(), parsed.hir.decls.len())
    );
    assert_eq!(read.resolution.names.len(), parsed.resolution.names.len());
    read.tokens.verify(TEXT).unwrap();
    read.cst.verify(&read.tokens, TEXT).unwrap();
    read.hir.verify(OriginTable::global()).unwrap();
}
