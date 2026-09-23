use super::{Algorithm, Hash};

#[test]
fn text_form_names_the_algorithm_and_round_trips() {
    let h = Hash::of(b"overt");
    let text = h.to_string();
    assert!(text.starts_with("sha256:"), "{text}");
    assert_eq!(Hash::parse(&text), Some(h));
    assert_eq!(h.algorithm(), Algorithm::Sha256);
    assert_eq!(Hash::parse("sha256-00"), None);
    assert_eq!(Hash::parse("md5:00"), None);
    assert_eq!(Hash::parse("sha256:zz"), None);
}
