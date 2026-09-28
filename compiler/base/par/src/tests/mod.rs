use super::Parallel;

#[test]
fn results_come_back_in_order() {
    let items: Vec<u64> = (0..1000).collect();
    assert_eq!(items.par_map(|n| n * n), items.iter().map(|n| n * n).collect::<Vec<_>>());
}

#[test]
fn no_items_give_no_results() {
    let items: [u8; 0] = [];
    assert!(items.par_map(|n| *n).is_empty());
}

#[test]
fn a_call_inside_a_worker_runs_on_it() {
    let rows: Vec<u64> = (0..64).collect();
    let sums = rows.par_map(|&r| (0..100).collect::<Vec<u64>>().par_map(|c| r * c).iter().sum::<u64>());
    assert_eq!(sums, rows.iter().map(|r| r * 4950).collect::<Vec<_>>());
}

#[test]
fn a_panic_goes_on_in_the_caller_which_works_again_after() {
    let items: Vec<u32> = (0..100).collect();
    let caught = std::panic::catch_unwind(|| items.par_map(|&n| assert_ne!(n, 57)));
    assert!(caught.is_err());
    assert_eq!(items.par_map(|n| n + 1).len(), 100);
}
mod pool;
