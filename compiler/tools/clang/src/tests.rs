use crate::resource::major;
use std::path::PathBuf;

use crate::{Clang, Kind, TypeKind};

/// A header in a fresh directory of the system's temporary one.
fn header(name: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vun-clang-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("h.h");
    std::fs::write(&path, text).unwrap();
    path
}

#[test]
fn a_header_reads_as_declarations_with_target_sizes() {
    // Machines without libclang cannot run this; `vun util ffi bind` reports the same absence.
    let Ok(clang) = Clang::load() else {
        eprintln!("libclang not found; skipped");
        return;
    };
    let path = header(
        "read",
        "#define LIMIT 0x10\nstruct point { int x; long y; };\nint area(const struct point *p, ...);\n",
    );
    let unit = clang.parse(&path, &["--target=x86_64-linux-gnu".to_owned()]).unwrap();
    assert!(unit.diagnostics().is_empty());
    let mine: Vec<_> =
        unit.root().children().into_iter().filter(|c| c.location().file.as_ref() == Some(&path)).collect();
    let kinds: Vec<_> = mine.iter().map(|c| (c.kind(), c.name())).collect();
    assert_eq!(
        kinds,
        [(Kind::Macro, "LIMIT".into()), (Kind::Struct, "point".into()), (Kind::Function, "area".into())]
    );
    assert_eq!(mine[0].tokens(), ["LIMIT", "0x10"]);
    let fields = mine[1].children();
    assert_eq!(fields[1].ty().kind(), TypeKind::Integer { signed: true });
    assert_eq!(fields[1].ty().size(), Some(8), "`long` is 8 bytes on x86_64 Linux");
    assert_eq!(mine[1].ty().size(), Some(16));
    assert!(mine[2].is_variadic());
    let param = mine[2].params()[0].ty();
    assert_eq!(param.kind(), TypeKind::Pointer);
    // `struct point` is the record it names, whichever version of clang reads it.
    assert_eq!(param.pointee().kind(), TypeKind::Record);
    assert_eq!(param.pointee().declaration().name(), "point");
}

#[test]
fn major_versions_are_read_from_library_names() {
    assert_eq!(major("libclang.so.21.1.8"), Some(21));
    assert_eq!(major("libclang-18.so.1"), Some(18));
    assert_eq!(major("libclang.so"), None);
    assert_eq!(major("libclang.dylib"), None);
}
