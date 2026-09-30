use std::collections::HashSet;

use vun_origin::OriginTable;
use vun_span::{SourceFiles, Span};

use crate::errors::{self, ALL};
use crate::{ChildKind, DiagCtxt, Diagnostic, TraceFrame};

#[test]
fn ids_are_unique() {
    assert_eq!(ALL.iter().collect::<HashSet<_>>().len(), ALL.len());
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
