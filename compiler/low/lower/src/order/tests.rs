use super::*;

fn layers(callees: &[Vec<usize>]) -> Vec<usize> {
    Inlining::layered(callees).into_iter().map(|(_, layer)| layer).collect()
}

#[test]
fn a_function_comes_after_its_callees() {
    // 0 calls 1 and 2, 1 calls 2.
    assert_eq!(layers(&[vec![1, 2], vec![2], vec![]]), [2, 1, 0]);
}

#[test]
fn functions_calling_each_other_and_themselves_have_a_layer() {
    // 0 and 1 call each other, 1 calls itself, 2 calls 0.
    assert_eq!(layers(&[vec![1], vec![0, 1], vec![0]]).len(), 3);
    let all = layers(&[vec![1], vec![0, 1], vec![0]]);
    assert!(all[2] > all[0]);
}
