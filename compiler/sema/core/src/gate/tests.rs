use super::*;

#[test]
fn a_feature_is_named_as_the_variant_of_lang_gate_it_reads_from() {
    assert_eq!(Feature::VARIANTS.len(), Feature::ALL.len());
    for &feature in Feature::ALL {
        assert_eq!(feature.item().symbol().as_str(), feature.name());
    }
}
