//! The diagnostic model: labels, fixes, severities, sinks (spec 10, section 2).
//!
//! Diagnostics are data; rendering is a separate concern behind [`DiagSink`].

#[rustfmt::skip]
pub mod errors;
mod ctxt;
mod diagnostic;
mod fix;
mod id;
mod msg;
pub mod suggest;
#[cfg(test)]
mod tests;

pub use ctxt::{DiagCtxt, DiagCx, DiagSink, ErrorGuaranteed};
pub use diagnostic::{Child, ChildKind, Diagnostic, TraceFrame};
pub use fix::{Edit, Fix, Hole, Label, Safety};
pub use id::{DiagId, DiagSource, Severity};
pub use msg::{Msg, MsgPart};
