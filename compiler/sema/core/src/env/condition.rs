//! Conditions: the `lang` type of `if` heads and of `and`, `or` and `!`, and the value on which
//! a condition holds, both found through the table `forms` (appendix B). Comparisons produce
//! conditions, so their values are built here.

use vun_codec::{Decode, Encode};
use vun_desugar::{FORMS, Form};
use vun_hir::tree::{BlockId, Cond, ExprId, ExprKind, PatId};
use vun_sema_ty::call::Staging;
use vun_sema_ty::item::Node;
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Ty, Value};

use super::Env;
use crate::member::Members;

impl Env<'_, '_> {
    /// The item the syntactic form `form` means.
    ///
    /// # Errors
    /// Returns why the item has no value here.
    pub fn form(&self, form: Form) -> Result<Value, Unevaluated> {
        let row = FORMS.iter().find(|row| row.form == form).ok_or(Unevaluated::Absent)?;
        self.item(row.item)
    }

    /// The type of conditions.
    ///
    /// # Errors
    /// Returns why it has no value here.
    pub fn condition(&self) -> Result<Ty, Unevaluated> {
        self.form(Form::Condition)?.as_type().ok_or(Unevaluated::Unsupported)
    }

    /// Whether the condition `value` holds.
    ///
    /// # Errors
    /// Returns [`Unevaluated::Unsupported`] for a value that is no condition.
    pub fn holds(&self, value: &Value) -> Result<bool, Unevaluated> {
        let truth = self.form(Form::Truth)?;
        match value {
            Value::Variant { ty, .. } if *ty == self.condition()? => Ok(*value == truth),
            _ => Err(Unevaluated::Unsupported),
        }
    }

    /// Whether the condition of a `$if` holds here, with what a test `p = e` binds when it does:
    /// the binders of `p` bound to the parts of `e` it takes (spec 03, section 1). `None` when it
    /// does not hold.
    ///
    /// # Errors
    /// Returns why the condition, or the value tested, has no value here.
    pub fn tested(&self, cond: &Cond) -> Result<Option<Vec<Staging>>, Unevaluated> {
        match cond {
            Cond::Expr(c) => Ok(self.holds(&self.eval(*c)?)?.then(Vec::new)),
            Cond::Test { pat, value } => {
                let mut bound = Vec::new();
                if !self.takes(*pat, &self.eval(*value)?, self.type_of(*value), &mut bound)? {
                    return Ok(None);
                }
                let binders = &self.parsed.hir.binders;
                let staged = bound
                    .into_iter()
                    .map(|(id, value)| Staging { binder: Node { id, origin: binders[id].origin }, value });
                Ok(Some(staged.collect()))
            }
        }
    }

    /// The condition that holds when `holds` is: the truth, or the condition type's other
    /// value.
    ///
    /// # Errors
    /// Returns why the condition type or its values are not known here.
    pub fn decide(&self, holds: bool) -> Result<Value, Unevaluated> {
        let truth = self.form(Form::Truth)?;
        if holds {
            return Ok(truth);
        }
        let Value::Variant { ty, name, .. } = &truth else { return Err(Unevaluated::Unsupported) };
        let members = self.cx.get::<Members>(ty).ok_or(Unevaluated::Unsupported)?;
        let mut others = members.variants.iter().filter(|v| v.name != *name);
        match (others.next(), others.next()) {
            (Some(other), None) => Ok(Value::Variant { ty: *ty, name: other.name, payload: Vec::new() }),
            _ => Err(Unevaluated::Unsupported),
        }
    }

    /// The `lang.Ordering` saying where one value is against another.
    ///
    /// # Errors
    /// Returns why `lang.Ordering` is not known here.
    pub fn ordering(&self, order: std::cmp::Ordering) -> Result<Value, Unevaluated> {
        self.item(match order {
            std::cmp::Ordering::Less => vun_items::lang::Ordering::less,
            std::cmp::Ordering::Equal => vun_items::lang::Ordering::equal,
            std::cmp::Ordering::Greater => vun_items::lang::Ordering::greater,
        })
    }

    /// The branch the `if` expression `expr` takes when its conditions are known here: `$if` as
    /// a value selects its branch at compile time (spec 03, section 1), `else if` included.
    ///
    /// # Errors
    /// Returns why a condition, or a value tested, has no value here.
    pub fn select(&self, mut expr: ExprId) -> Result<Selected, Unevaluated> {
        let hir = &self.parsed.hir;
        loop {
            let ExprKind::If { cond, then, otherwise } = &hir.exprs[expr].kind else {
                return Err(Unevaluated::Unsupported);
            };
            if let Some(bound) = self.tested(cond)? {
                return Ok(match cond {
                    Cond::Expr(_) => Selected::Block(*then),
                    Cond::Test { pat, .. } => Selected::Tested { block: *then, pat: *pat, bound },
                });
            }
            match otherwise {
                None => return Ok(Selected::Unit),
                Some(e) if matches!(hir.exprs[*e].kind, ExprKind::If { .. }) => expr = *e,
                Some(e) => return Ok(Selected::Expr(*e)),
            }
        }
    }
}

/// The branch a `$if` takes.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Selected {
    Block(BlockId),
    /// The branch of a test `p = e` that held, with what `p` binds: a step of its own (spec 03,
    /// section 1).
    Tested {
        block: BlockId,
        pat: PatId,
        bound: Vec<Staging>,
    },
    /// An `else` that is not a block, or not a block written as one.
    Expr(ExprId),
    /// No `else`, and no condition held: `()`.
    Unit,
}
