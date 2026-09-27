//! The value of an expression, by its form: each form is lowered by the module of its concern.

use vun_grammar::TokenKind;
use vun_hir::tree::{ExprId, ExprKind, Lit};
use vun_sema::env::Unevaluated;
use vun_sema::int::Integer;
use vun_sema::ty::Value;
use vun_sema::typeck::Operation;

use super::operator::Side;
use super::{Builder, Unbuilt};
use crate::{Operand, Rvalue};

impl Builder<'_, '_, '_> {
    pub(super) fn value(&mut self, expr: ExprId) -> Result<Operand, Unbuilt> {
        let hir = &self.env.parsed.hir;
        let origin = hir.exprs[expr].origin;
        let unbuilt = Unbuilt::at(origin);
        match &hir.exprs[expr].kind {
            ExprKind::Lit(Lit::Int(n)) => Ok(Operand::Const(Value::Int(Integer::from(n.clone())))),
            ExprKind::Name(_) => self.named(expr, origin),
            ExprKind::Block(block) => self.block(*block),
            // Attributes are read where they apply, from the expression they mark.
            ExprKind::Attributed { inner, .. } => self.operand(*inner),
            ExprKind::Quote(_) => Ok(self.quote(expr, origin)),
            ExprKind::Member { .. }
            | ExprKind::Index { .. }
            | ExprKind::Postfix { op: TokenKind::DotStar, .. } => self.read(expr),
            ExprKind::Borrow { mode, operand } => {
                self.borrow(expr, mode.ok_or(Unbuilt::at(origin))?, *operand)
            }
            // `unsafe e` runs `e`; what it permits is checked, not lowered.
            ExprKind::Prefix { op: TokenKind::Unsafe, operand, .. } => self.operand(*operand),
            ExprKind::Prefix { op: TokenKind::Dollar, operand, .. }
                if matches!(hir.exprs[*operand].kind, ExprKind::If { .. }) =>
            {
                self.selected(*operand, origin)
            }
            ExprKind::Prefix { op: TokenKind::Dollar, operand, .. }
                if let Ok(Value::Code(code)) = self.env().eval(*operand) =>
            {
                Ok(self.spliced(expr, code, origin))
            }
            ExprKind::Infix { op: op @ (TokenKind::DotDot | TokenKind::DotDotEq), left, right } => {
                let lo = self.operand(*left)?;
                let hi = right.map(|r| self.operand(r)).transpose()?;
                Ok(self.temp(Rvalue::Range { op: *op, lo, hi }, origin))
            }
            ExprKind::Infix { op: op @ (TokenKind::And | TokenKind::Or), left, right: Some(right) } => {
                self.logic(*op, *left, *right, origin)
            }
            ExprKind::Postfix { op: TokenKind::Question, operand } => {
                self.early_return(expr, *operand, origin)
            }
            ExprKind::Yield(value) => self.yielded(*value, origin),
            ExprKind::Closure { captures, func } => self.closure(captures, *func, origin),
            ExprKind::Match { scrutinee, arms } => self.matching(*scrutinee, arms, origin),
            ExprKind::If { cond, then, otherwise } => self.branch(cond, *then, *otherwise, origin),
            ExprKind::For { head, body } => self.repeat(None, head, *body, origin),
            ExprKind::Labeled { label, body } => match &hir.exprs[*body].kind {
                ExprKind::For { head, body } => self.repeat(Some(*label), head, *body, origin),
                _ => Err(unbuilt),
            },
            ExprKind::Break { label, value } => self.leave(*label, *value, origin),
            ExprKind::Continue { label } => self.again(*label, origin),
            ExprKind::Return(value) => self.exit(*value, origin),
            ExprKind::Defer(statement) => Ok(self.deferred(*statement)),
            ExprKind::Infix { left, right: Some(right), .. } => {
                self.operator(Operation::Expr(expr), Side::Expr(*left), vec![Side::Expr(*right)], origin)
            }
            ExprKind::Prefix { op: TokenKind::Minus | TokenKind::Tilde, operand, .. } => {
                self.operator(Operation::Expr(expr), Side::Expr(*operand), Vec::new(), origin)
            }
            ExprKind::Prefix { op: TokenKind::Bang, operand, .. } => {
                let operand = self.operand(*operand)?;
                Ok(self.temp(Rvalue::Not { operand }, origin))
            }
            ExprKind::Call { callee, .. } if matches!(hir.exprs[*callee].kind, ExprKind::DotName(_)) => {
                self.aggregate(expr).unwrap_or(Err(unbuilt))
            }
            ExprKind::Call { callee, args } => self.call(expr, *callee, args, origin),
            ExprKind::Struct(_) | ExprKind::Enum { .. } | ExprKind::Fn { .. } => {
                Ok(self.temp(self.comptime(expr, None), origin))
            }
            // Anything else only as a constant sema can compute, such as `lang.int.u8`; one whose
            // evaluation traps is evaluated by the call running the body, where the trap fires.
            _ => match self.aggregate(expr) {
                Some(built) => built,
                None => match self.env().eval(expr) {
                    Ok(value) => Ok(Operand::Const(value)),
                    Err(Unevaluated::Trapped(_)) => Ok(self.temp(self.comptime(expr, None), origin)),
                    Err(_) => Err(unbuilt),
                },
            },
        }
    }
}
