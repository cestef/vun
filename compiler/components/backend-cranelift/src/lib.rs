//! The Cranelift backend as a component of the toolchain (decision 498): linked into `vun`, or
//! built with the `export` feature as a library of its `components` directory. The native
//! backend assists it with what Cranelift cannot express, inline assembly and naked functions.

use std::sync::Arc;

use vun_backend_cranelift::CraneliftBackend;
use vun_backend_native::NativeBackend;

/// The backend this component serves.
#[must_use]
pub fn component() -> impl vun_backend::Backend + Send + Sync + 'static {
    CraneliftBackend::assisted_by(Arc::new(NativeBackend))
}

#[cfg(feature = "export")]
vun_component::export!(vun_backend_component::Backends, component());
