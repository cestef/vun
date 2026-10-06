//! The C binder as a component of the toolchain (decision 498): linked into `vun`, or built with
//! the `export` feature as a library of its `components` directory. It loads libclang when it
//! binds, as it does either way.

/// The binder this component serves.
#[must_use]
pub fn component() -> impl vun_binder::Binder + 'static {
    vun_bind::CBinder
}

#[cfg(feature = "export")]
vun_component::export!(vun_binder::Binders, component());
