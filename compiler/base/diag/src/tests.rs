use std::collections::HashSet;

use vun_origin::OriginTable;
use vun_span::{SourceFiles, Span};

use crate::errors::{self, ALL};
use crate::{ChildKind, DiagCtxt, DiagId, Diagnostic, TraceFrame};

#[test]
fn ids_are_unique() {
    assert_eq!(ALL.iter().collect::<HashSet<_>>().len(), ALL.len());
}

#[test]
fn a_compiler_id_is_one_value_however_it_is_named() {
    let named = DiagId::compiler("int.does_not_fit");
    assert_eq!(named, errors::INT_DOES_NOT_FIT);
    assert_eq!(DiagId::parse("lang.errors.int.does_not_fit"), named);
    assert_eq!(named.to_string(), "int.does_not_fit");
    assert_eq!(named.segments(), ["lang", "errors", "int", "does_not_fit"]);
}

#[test]
fn a_package_id_is_its_names_and_lies_in_the_namespaces_of_its_path() {
    let lint = DiagId::parse("std.lints.style.unused");
    assert!(matches!(lint, DiagId::Package(_)));
    assert_eq!(lint, DiagId::package(["std", "lints", "style", "unused"].map(vun_intern::Symbol::intern)));
    assert_eq!(lint.to_string(), "std.lints.style.unused");
    assert!(lint.within(&DiagId::parse("std.lints.style")));
    assert!(!lint.within(&DiagId::parse("std.lints.sty")));
    assert!(errors::INT_DOES_NOT_FIT.within(&errors::INT));
    assert!(!errors::INT_DOES_NOT_FIT.within(&errors::TYPES));
}

#[test]
fn an_id_is_its_path_whatever_variant_holds_it() {
    let tool = DiagId::new("std.lints.style.unused");
    let package = DiagId::parse("std.lints.style.unused");
    assert_eq!(tool, package);
    assert_eq!(HashSet::from([tool, package]).len(), 1);
}

#[test]
fn ids_are_ordered_by_binding_path_whatever_declares_them() {
    let tool = DiagId::new("command.failed");
    assert!(tool < errors::INT_DOES_NOT_FIT);
    assert!(errors::INT_DOES_NOT_FIT < DiagId::parse("std.lints.style.unused"));
}

#[test]
fn causes_follow_the_source_chain() {
    let inner = std::io::Error::other("connection refused");
    let outer = std::io::Error::new(std::io::ErrorKind::NotFound, inner);
    let d = Diagnostic::error(errors::IO_UNREADABLE_FILE, "cannot read").caused_by(&outer);
    let causes: Vec<_> = d.children.iter().map(|c| (c.kind, c.msg.to_string())).collect();
    assert_eq!(causes, [(ChildKind::Cause, "connection refused".to_owned())]);
}

#[test]
fn a_report_reached_several_ways_is_kept_once_reached_the_shortest_way() {
    let file = SourceFiles::new().add("a.vun", "f(1); f(2); y: u8 = 300;");
    let origins = OriginTable::new();
    let [first, second, at] =
        [(0, 4), (6, 10), (20, 23)].map(|(lo, hi)| origins.source(Span::new(file, lo, hi)));
    let plain = Diagnostic::error(errors::INT_DOES_NOT_FIT, "does not fit").at(at, None);
    let other = Diagnostic::error(errors::INT_DOES_NOT_FIT, "does not fit either").at(at, None);
    let through = |d: &Diagnostic, origin| {
        let mut d = d.clone();
        d.trace.push(TraceFrame::instantiated(origin));
        d
    };
    let mut diag = DiagCtxt::new();
    for d in [through(&plain, first), plain.clone(), through(&plain, second), through(&other, first)] {
        diag.report(d);
    }
    diag.reached_once();
    let mut expected = DiagCtxt::new();
    expected.report(plain);
    expected.report(through(&other, first));
    assert_eq!(diag, expected);
}
