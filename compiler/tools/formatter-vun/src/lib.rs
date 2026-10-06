//! The formatter of Vun source as a component of the toolchain (decision 498): linked into
//! `vun`, or built with the `export` feature as a library of its `components` directory.

/// The formatter this component serves.
#[must_use]
pub fn component() -> impl vun_formatter::Formatter + 'static {
    vun_fmt::Vun
}

#[cfg(feature = "export")]
vun_component::export!(vun_formatter::Formatters, component());
