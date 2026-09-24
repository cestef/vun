use super::*;

#[test]
fn defaults_register_enabled_components() {
    let r = Registry::with_defaults();
    assert!(r.backend(vun_backend_c::INFO.id).is_some());
    assert!(r.linker(vun_link_lld::INFO.id).is_some());
    assert!(r.backend("").is_none());
}
