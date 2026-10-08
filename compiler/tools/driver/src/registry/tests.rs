use super::*;

#[test]
fn defaults_register_enabled_components() {
    let r = Registry::with_defaults();
    assert!(r.backend(vun_backend_c::INFO.id.symbol()).is_some());
    assert!(r.linker(vun_link_lld::INFO.id.symbol()).is_some());
    assert!(r.backend(Symbol::intern("")).is_none());
}

#[test]
fn a_piece_of_a_release_is_named_by_its_kind_and_id() {
    let cranelift = Id { kind: Kind::Backend, name: "cranelift".to_owned() };
    assert_eq!(Id::parse("backend-cranelift"), Some(cranelift));
    // A linker's crate is `link-<id>`, whatever the kind is called.
    let lld = Id::parse("link-lld").unwrap();
    assert_eq!((lld.kind, lld.to_string()), (Kind::Linker, "link-lld".to_owned()));
    assert_eq!(Id::parse("linker-lld"), None);
    assert_eq!(Id::parse("cranelift"), None);
    assert_eq!(Id::parse("gadget-x"), None);
}

#[test]
fn every_registered_component_has_an_id_that_parses() {
    for entry in Registry::with_defaults().entries() {
        assert_eq!(Id::parse(&entry.id.to_string()), Some(entry.id.clone()));
    }
}

#[test]
fn every_kind_of_component_is_served_under_its_name() {
    let mut served: Vec<&str> = components::Served::KINDS.iter().map(|(kind, _)| kind.0).collect();
    let mut kinds: Vec<&str> = Kind::ALL.iter().map(|kind| kind.name()).collect();
    served.sort_unstable();
    kinds.sort_unstable();
    assert_eq!(served, kinds);
}
