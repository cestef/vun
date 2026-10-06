//! What a function literal leaves out that nothing supplies (spec 01, section 4): a parameter
//! written without a type, and a result written nowhere whose body is not `()`.

use vun_diag::{Diagnostic, Msg, errors};
use vun_hir::tree::{PatId, PatKind};
use vun_origin::OriginId;
use vun_sema_ty::Ty;

use super::{Known, Typer};
use vun_sema::env::Place;

impl Typer<'_, '_, '_> {
    /// Reports that a function writing no result, whose result is `result` for that, has a body
    /// of the type `found`: the type to write is that.
    pub(super) fn unwritten_result(&mut self, at: OriginId, found: Known, result: Ty) {
        let msg =
            Msg::new().text("expected ").code(result.to_string()).text(", found ").ty(found.ty.to_string());
        let help = Msg::new()
            .text("this function writes no result, so it is ")
            .code(result.to_string())
            .text("; write the type it gives after its parameters, as ")
            .code(format!("-> {}", found.ty));
        self.diag.error(Diagnostic::error(errors::TYPES_MISMATCH, msg).at(at, None).help(help));
    }

    /// Reports the parameter bound by `pat` as written without a type: nothing else says what a
    /// caller passes it. `given` is the number of parameters its place gives types to, when it
    /// does: the type this one has is past them.
    pub(super) fn untyped(&mut self, pat: PatId, given: Option<(Place, usize, usize)>) {
        let hir = self.hir();
        let origin = hir.pats[pat].origin;
        let name = match hir.pats[pat].kind {
            PatKind::Bind(b) => hir.binders[b].name.as_str(),
            _ => "x",
        };
        let msg = Msg::new().text("this parameter has no type");
        let help = Msg::new().text("write its type after its name, as ").code(format!("{name}: T"));
        let mut d = Diagnostic::error(errors::TYPES_UNTYPED_PARAM, msg).at(origin, None).help(help);
        if let Some((place, has, takes)) = given {
            let count = |n: usize| if n == 1 { "1 parameter".to_string() } else { format!("{n} parameters") };
            let note = if has == takes {
                // The place has a parameter here, but what it says names something not known yet.
                format!(
                    "{} gives it a type that is not known yet: it names a `$` parameter that the other arguments leave open",
                    place.describe()
                )
            } else {
                format!("{} has {}, and this function takes {}", place.describe(), count(has), count(takes))
            };
            d = d.note(note);
        }
        self.diag.error(d);
    }
}
