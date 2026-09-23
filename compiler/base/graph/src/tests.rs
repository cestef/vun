use super::{OrderError, Requirements};

#[test]
fn requirements_come_first_and_ties_keep_insertion_order() {
    let mut r = Requirements::new();
    r.add("borrows", ["places"]);
    r.add("places", []);
    r.add("lints", []);
    assert_eq!(r.order(), Ok(vec![1, 0, 2]));
}

#[test]
fn cycles_and_missing_requirements_are_errors() {
    let mut r = Requirements::new();
    r.add("a", ["b"]);
    r.add("b", ["a"]);
    assert!(matches!(r.order(), Err(OrderError::Cycle(_))));
    let mut r = Requirements::new();
    r.add("a", ["z"]);
    assert_eq!(r.order(), Err(OrderError::Missing { node: "a", requires: "z" }));
}
