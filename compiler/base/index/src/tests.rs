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

#[test]
fn a_bit_set_iterates_ascending_and_compares_by_its_members() {
    let n = Node::new;
    let mut set: BitSet<Node> = [n(70), n(3), n(64)].into_iter().collect();
    assert!(!set.insert(n(3)) && set.insert(n(0)));
    assert_eq!(set.iter().collect::<Vec<_>>(), [n(0), n(3), n(64), n(70)]);
    assert_eq!(set.len(), 4);
    let mut other = BitSet::new();
    other.insert(n(5));
    assert!(set.union(&other) && !set.union(&other));
    assert!(set.remove(n(64)) && set.remove(n(70)) && !set.remove(n(200)));
    // The words emptied at the end hold nothing.
    let same: BitSet<Node> = [n(0), n(3), n(5)].into_iter().collect();
    assert_eq!(set, same);
    assert!(set.contains(n(5)) && !set.contains(n(6)) && !set.contains(n(1000)));
}

#[test]
fn a_bit_matrix_joins_whole_and_reads_by_row() {
    let n = Node::new;
    let mut m: BitMatrix<Node, Node> = BitMatrix::new(3, 70);
    assert!(m.insert(n(1), n(69)) && !m.insert(n(1), n(69)));
    let mut other = BitMatrix::new(3, 70);
    other.insert(n(2), n(0));
    assert!(m.union(&other) && !m.union(&other));
    m.set_row(n(0), &[n(5), n(64)].into_iter().collect());
    assert_eq!(m.iter_row(n(0)).collect::<Vec<_>>(), [n(5), n(64)]);
    assert!(m.contains(n(2), n(0)) && !m.contains(n(0), n(0)));
}
