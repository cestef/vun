//! Type checking (spec 02 and 04): so far, a declaration's value against its annotated type,
//! wherever both are known without evaluating function bodies.

use std::sync::Arc;

use overt_diag::{ChildKind, DiagCtxt, Diagnostic, Msg, errors};
use overt_hir::db::FileRef;
use overt_query::{Cx, Query};

use crate::env::Env;

/// What checking a file reports; `None` for a file the driver did not read.
pub struct Checked;

impl Query for Checked {
    const NAME: &'static str = "checked";
    type Key = FileRef;
    type Value = Option<Arc<DiagCtxt>>;

    fn compute(cx: &Cx<'_>, key: &FileRef) -> Self::Value {
        let env = Env::of(cx, key)?;
        let hir = &env.parsed.hir;
        let mut diag = DiagCtxt::new();
        for (_, decl) in hir.decls.iter() {
            let (Some(annotation), Some(value)) = (decl.ty, decl.value) else { continue };
            let Some(expected) = env.eval(annotation).ok().and_then(|v| v.as_type()) else { continue };
            let Some(found) = env.type_of(value) else { continue };
            if expected.accepts(&found) {
                continue;
            }
            let written = env.source(annotation);
            // Integers that do not fit are their own diagnostic (spec 04, rule 2).
            let d = if expected.interval().is_some() && found.interval().is_some() {
                let msg = Msg::new().text("value does not fit the type ").code(&written);
                let note = Msg::new().text("the value is ").code(found.to_string()).text(", and ");
                let note = note.code(&written).text(" is ").code(expected.to_string());
                Diagnostic::error(errors::INT_DOES_NOT_FIT, msg).child(ChildKind::Note, note)
            } else {
                let msg =
                    Msg::new().text("expected ").code(&written).text(", found ").code(found.to_string());
                Diagnostic::error(errors::TYPES_MISMATCH, msg)
            };
            let expected_here = Msg::new().text("expected because of this type");
            let d =
                d.at(hir.exprs[value].origin, None).label(hir.exprs[annotation].origin, Some(expected_here));
            diag.error(d);
        }
        Some(Arc::new(diag))
    }
}
