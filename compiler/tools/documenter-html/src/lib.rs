//! The documenter of Vun packages as a component of the toolchain (decision 498): linked into
//! `vun`, or built with the `export` feature as a library of its `components` directory.

/// The documenter this component serves.
#[must_use]
pub fn component() -> impl vun_documenter::Documenter + 'static {
    vun_doc::Html
}

#[cfg(feature = "export")]
vun_component::export!(vun_documenter::Documenters, component());
