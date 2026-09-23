use std::collections::HashSet;

use crate::errors::{self, ALL};
use crate::{ChildKind, Diagnostic};

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
