use std::collections::HashSet;

use crate::errors::ALL;

#[test]
fn ids_are_unique() {
    assert_eq!(ALL.iter().collect::<HashSet<_>>().len(), ALL.len());
}
