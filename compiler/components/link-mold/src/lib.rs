//! The mold linker as a component of the toolchain (decision 498): linked into
//! `vun`, or built with the `export` feature as a library of its `components` directory.

/// The linker this component serves.
#[must_use]
pub fn component() -> impl vun_link::Linker + Send + Sync + 'static {
    vun_link_mold::MoldLinker
}

#[cfg(feature = "export")]
vun_component::export!(vun_link_component::Linkers, component());
