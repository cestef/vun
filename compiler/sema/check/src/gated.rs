//! Language features (spec 12, section 2): a name that leads to a declaration behind a feature
//! the package does not enable is reported wherever it is written.

use vun_diag::{Diagnostic, Msg, errors};
use vun_hir::tree::{ExprId, ExprKind, Hir};

use super::Checker;

impl Checker<'_, '_, '_> {
    /// Reports the path `expr` when it names a declaration behind a feature not enabled here.
    pub(super) fn gated(&mut self, hir: &Hir, expr: ExprId) {
        if !matches!(hir.exprs[expr].kind, ExprKind::Name(_) | ExprKind::Member { .. }) {
            return;
        }
        let Some((_, path)) = hir.path(expr) else { return };
        let Some(feature) = self.env.resolve(expr).ok().and_then(|site| self.env.disabled(&site)) else {
            return;
        };
        let msg = Msg::new()
            .path(path.iter().map(|s| s.as_str()))
            .text(" is behind the feature ")
            .code(feature.name());
        let help = Msg::new()
            .text("enable it in the manifest: ")
            .code(format!(".features = .{{ .{} }}", feature.name()));
        self.diag
            .error(Diagnostic::error(errors::GATE_DISABLED, msg).at(hir.exprs[expr].origin, None).help(help));
    }
}
