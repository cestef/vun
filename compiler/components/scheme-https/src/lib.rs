//! The https scheme as a component of the toolchain (decision 498): linked into `vun`, or built
//! with the `export` feature as a library of its `components` directory.

/// The scheme this component serves.
#[must_use]
pub fn component() -> impl vun_fetch::Scheme + 'static {
    vun_http::Https
}

#[cfg(feature = "export")]
vun_component::export!(vun_scheme_component::Schemes, component());
