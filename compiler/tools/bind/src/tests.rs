use crate::macros::integer;

#[test]
fn integer_macros_read_in_every_form_c_writes() {
    let tokens = |ts: &[&str]| ts.iter().map(|&t| t.to_owned()).collect::<Vec<_>>();
    assert_eq!(integer(&tokens(&["(", "0x10U", ")"])), Some(16));
    assert_eq!(integer(&tokens(&["-", "1"])), Some(-1));
    assert_eq!(integer(&tokens(&["010"])), Some(8));
    assert_eq!(integer(&tokens(&["0b101"])), Some(5));
    assert_eq!(integer(&tokens(&["1'000ULL"])), Some(1000));
    assert_eq!(integer(&tokens(&["1.5"])), None);
    assert_eq!(integer(&tokens(&["A", "+", "1"])), None);
    assert_eq!(integer(&[]), None);
}
