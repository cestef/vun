//! Where a report of a generic body is: a body checked once for each instance, whose report
//! names the instance, since the line it points to is the same for every one.

use vun_diag::Msg;
use vun_origin::OriginTable;
use vun_sema_ty::call::Instantiation;

use super::Placed;

impl Placed {
    /// What says which instance a report is of, when the body is generic: the function and the
    /// arguments of its `$` parameters.
    pub(super) fn context(i: &Instantiation) -> Option<Msg> {
        let args: Vec<String> = i.args.iter().flatten().map(ToString::to_string).collect();
        if args.is_empty() {
            return None;
        }
        let function = OriginTable::global().declaration_name(i.f.literal.expr.origin);
        let called = function.map_or_else(|| "the function".to_owned(), |name| name.to_string());
        Some(Msg::new().text("in ").code(called).text(" for ").code_list(args))
    }
}
