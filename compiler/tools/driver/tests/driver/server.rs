//! Servers as components (decision 498): the language server loaded from its library serves a
//! client that has nothing to say, and returns when the client leaves.

use vun_diag::DiagCtxt;
use vun_driver::{Components, Provider, Registry};

use crate::support::built_component;

const CRATE: &str = "vun-component-server-lsp";

#[test]
fn a_server_loaded_from_a_library_serves_until_its_client_leaves() {
    let mut registry = Registry::default();
    registry.load(&Components::at(built_component(CRATE)));
    assert!(matches!(registry.servers.get("lsp").map(|s| &s.from), Some(Provider::Loaded(_))));
    let mut diag = DiagCtxt::new();
    registry.servers.get("lsp").unwrap().item.serve(&mut diag).unwrap();
    assert!(diag.failed().is_none());
}
