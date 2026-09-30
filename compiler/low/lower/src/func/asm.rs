//! Assembly blocks: an input binds the value it holds, an output the address and scalar type of
//! its place, and a symbol the address of the function, link symbol or static it names. A block
//! of the target's instruction set uses only forms its processor has.

use vun_inline::Rebind;
use vun_items::lang::check as operations;
use vun_lir::{Address, AsmCheck, InstKind, Output, Value};
use vun_mir::{Asm, Operand, Place};
use vun_sema::target::TargetOf;
use vun_sema_ty::Value as Constant;
use vun_target::Check;

use super::{FnLower, Storage};
use crate::unlowered::{Form, Unlowered, Why};

/// The bindings of a block, lowered in order.
struct Binder<'f, 'l, 'a, 'db>(&'f mut FnLower<'l, 'a, 'db>);

impl Binder<'_, '_, '_, '_> {
    fn unsupported(&self) -> Unlowered {
        Unlowered::unsupported(self.0.at, Form::Asm)
    }
}

impl Rebind<Operand, Place, Operand> for Binder<'_, '_, '_, '_> {
    type Value = Value;
    type Place = Output;
    type Symbol = Address;
    type Error = Unlowered;

    fn value(&mut self, value: Operand) -> Result<Value, Unlowered> {
        self.0.scalar_operand(&value)
    }

    fn place(&mut self, place: Place) -> Result<Output, Unlowered> {
        let reached = self.0.reach(&place)?.ok_or_else(|| self.unsupported())?;
        Ok(Output { addr: reached.addr, ty: self.0.scalar_of(reached.ty)? })
    }

    fn symbol(&mut self, symbol: Operand) -> Result<Address, Unlowered> {
        let at = self.0.at;
        match &symbol {
            // A function a block names is its code: a closure's captures have no symbol.
            Operand::Const(value) => {
                let code = !matches!(value, Constant::Closure { .. });
                self.0.l.address(value, code, at)?.ok_or_else(|| self.unsupported())
            }
            Operand::Place(place) => match self.0.slots[place.local] {
                Storage::Global(address) if place.projection.is_empty() => Ok(address),
                _ => Err(self.unsupported()),
            },
        }
    }
}

impl FnLower<'_, '_, '_> {
    /// The block, over the values and addresses its bindings lower to. One that does not return
    /// ends its block, whose terminator is then unreachable.
    pub(super) fn asm(&mut self, asm: &Asm) -> Result<(), Unlowered> {
        self.available(asm)?;
        let block = asm.clone().rebind(&mut Binder(self))?;
        let checked =
            if self.checked && self.l.checks.contains(Check::Asm) { Some(self.asm_check()?) } else { None };
        self.b.push(InstKind::Asm(Box::new(block), checked), None, self.at);
        Ok(())
    }

    /// Refuses a block of the target's instruction set with a form needing a feature the target's
    /// processor lacks. A block of another is the backend's to refuse.
    fn available(&self, asm: &Asm) -> Result<(), Unlowered> {
        let Some(target) = self.l.env.cx.get::<TargetOf>(&()) else { return Ok(()) };
        match asm.needs.iter().find(|n| !target.has(n.needed)) {
            Some(need) if target.isa == asm.arch.symbol() => {
                Err(Unlowered::new(need.at, Why::Unavailable(need.needed)))
            }
            _ => Ok(()),
        }
    }

    /// How the block lowered is checked under the `asm` checks: at a site of its own, through
    /// the helpers of `lang.check` the runtime provides.
    fn asm_check(&mut self) -> Result<AsmCheck, Unlowered> {
        let site = self.new_site(self.at, None)?;
        Ok(AsmCheck {
            site,
            write: self.provided(operations::asm_write)?,
            stack: self.provided(operations::asm_stack)?,
        })
    }
}
