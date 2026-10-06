//! Diagnostic output formats, chosen with `--diagnostics` (spec 10, section 2). Each is a
//! renderer component the registry holds, linked in or loaded; adding one is adding a component.

use vun_command::{Args, Param, Values};
use vun_diag::Diagnostic;
use vun_intern::Symbol;

use crate::{Registry, Session};

/// The format used without `--diagnostics`.
pub const DEFAULT: &str = "terminal";

pub const DIAGNOSTICS: Param = Param::option("diagnostics")
    .about("how diagnostics are shown")
    .values(Values::Choice(|| {
        Registry::with_defaults().renderers.iter().map(vun_renderer::Renderer::name).collect()
    }))
    .env("VUN_DIAGNOSTICS");

/// Applies `--diagnostics` to the session; parsing checked the name.
///
/// # Errors
/// Never.
pub fn configure(session: &mut Session, args: &Args) -> Result<(), Box<Diagnostic>> {
    if let Some(name) = args.value(&DIAGNOSTICS) {
        session.format = Symbol::intern(name);
    }
    Ok(())
}
