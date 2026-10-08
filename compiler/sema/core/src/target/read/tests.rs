use super::*;

#[test]
fn a_convention_is_named_as_the_variant_of_lang_target_it_reads_from() {
    for &(item, convention) in Convention::VARIANTS {
        assert_eq!(item.symbol().as_str(), convention.name());
    }
}
