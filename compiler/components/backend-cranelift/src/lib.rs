//! The Cranelift backend as a component of the toolchain (decision 498): linked into `vun`, or
//! built with the `export` feature as a library of its `components` directory. What Cranelift
//! cannot express, inline assembly and naked functions, is compiled by the host's assist.

/// The backend this component serves.
#[must_use]
pub fn component() -> impl vun_backend::Backend + Send + Sync + 'static {
    vun_backend_cranelift::CraneliftBackend
}

#[cfg(feature = "export")]
vun_component::export!(vun_backend_component::Backends, component());
