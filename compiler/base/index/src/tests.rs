use super::*;

index! {
    struct Node;
}

#[test]
fn push_returns_consecutive_indices() {
    let mut v: IndexVec<Node, &str> = IndexVec::new();
    let a = v.push("a");
    let b = v.push("b");
    assert_eq!((a.index(), b.index()), (0, 1));
    assert_eq!(v[b], "b");
    assert_eq!(v.iter().map(|(i, _)| i).collect::<Vec<_>>(), [a, b]);
}
