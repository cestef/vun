//! What comparisons give: a condition, the variant of `lang`'s condition type that holds or the
//! other one (the table `forms`), or a `lang.Ordering`, each written by branching on the tests.

use vun_desugar::Form as Syntax;
use vun_intern::Symbol;
use vun_lir::{Target, Terminator, Value};
use vun_sema::member::Members;
use vun_sema_ty::{Ty, Value as Constant};

use super::FnLower;
use crate::unlowered::{Form, Unlowered};

impl FnLower<'_, '_, '_> {
    /// The type of conditions, the variant holding, and the other one (the table `forms`).
    pub(super) fn truth(&self) -> Result<(Ty, Symbol, Symbol), Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Constant);
        let condition =
            self.l.env.form(Syntax::Condition).ok().and_then(|v| v.as_type()).ok_or_else(unsupported)?;
        let Ok(Constant::Variant { name: truth, .. }) = self.l.env.form(Syntax::Truth) else {
            return Err(unsupported());
        };
        let members = self.l.types.cx.get::<Members>(&condition).ok_or_else(unsupported)?;
        let other = members.variants.iter().find(|v| v.name != truth).ok_or_else(unsupported)?.name;
        Ok((condition, truth, other))
    }

    /// Writes into `dest`, a condition, whether `holds` is not 0.
    pub(crate) fn condition(&mut self, holds: Value, dest: Option<Value>, ty: Ty) -> Result<(), Unlowered> {
        let Some(dest) = dest else { return Ok(()) };
        let (_, truth, other) = self.truth()?;
        let (yes, no, join) =
            (self.b.block(&[], self.at), self.b.block(&[], self.at), self.b.block(&[], self.at));
        let to = |block| Target { block, args: Vec::new() };
        self.b.terminate(Terminator::Branch { cond: holds, then: to(yes), otherwise: to(no) });
        for (block, variant) in [(yes, truth), (no, other)] {
            self.b.switch_to(block);
            self.set_variant(dest, ty, variant)?;
            self.b.terminate(Terminator::Jump(to(join)));
        }
        self.b.switch_to(join);
        Ok(())
    }

    /// Writes into `dest`, a `lang.Ordering`, `less` where `less` is not 0, `equal` where `equal`
    /// is not 0, and `greater` otherwise.
    pub(crate) fn ordering(
        &mut self,
        less: Value,
        equal: Value,
        dest: Option<Value>,
        ty: Ty,
    ) -> Result<(), Unlowered> {
        let Some(dest) = dest else { return Ok(()) };
        let block = |this: &mut Self| this.b.block(&[], this.at);
        let (lt, rest, eq, gt, join) = (block(self), block(self), block(self), block(self), block(self));
        let to = |block| Target { block, args: Vec::new() };
        self.b.terminate(Terminator::Branch { cond: less, then: to(lt), otherwise: to(rest) });
        self.b.switch_to(rest);
        self.b.terminate(Terminator::Branch { cond: equal, then: to(eq), otherwise: to(gt) });
        let order = [
            (lt, vun_items::lang::Ordering::less),
            (eq, vun_items::lang::Ordering::equal),
            (gt, vun_items::lang::Ordering::greater),
        ];
        for (block, variant) in order {
            self.b.switch_to(block);
            self.set_variant(dest, ty, variant.symbol())?;
            self.b.terminate(Terminator::Jump(to(join)));
        }
        self.b.switch_to(join);
        Ok(())
    }

    /// Writes into `dest`, an option of an address, `.some(addr)` where `holds` is not 0, or
    /// always when there is no `holds`, and `.none` otherwise.
    pub(crate) fn optional(
        &mut self,
        holds: Option<Value>,
        addr: Value,
        dest: Value,
        ty: Ty,
    ) -> Result<(), Unlowered> {
        let (some, none) = (vun_items::lang::Option::some.symbol(), vun_items::lang::Option::none.symbol());
        let Some(holds) = holds else { return self.some(addr, dest, ty, some) };
        let (yes, no, join) =
            (self.b.block(&[], self.at), self.b.block(&[], self.at), self.b.block(&[], self.at));
        let to = |block| Target { block, args: Vec::new() };
        self.b.terminate(Terminator::Branch { cond: holds, then: to(yes), otherwise: to(no) });
        self.b.switch_to(yes);
        self.some(addr, dest, ty, some)?;
        self.b.terminate(Terminator::Jump(to(join)));
        self.b.switch_to(no);
        self.set_variant(dest, ty, none)?;
        self.b.terminate(Terminator::Jump(to(join)));
        self.b.switch_to(join);
        Ok(())
    }

    fn some(&mut self, addr: Value, dest: Value, ty: Ty, some: Symbol) -> Result<(), Unlowered> {
        self.set_variant(dest, ty, some)?;
        let at = self.payload(dest, ty, some, 0)?;
        self.b.store(addr, at, vun_lir::Access::Plain, self.at);
        Ok(())
    }
}
