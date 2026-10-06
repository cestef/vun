//! A block of instructions: the block sema checked, each binding rebound here, an input to the
//! operand of its value, an output to its place, and an address to the operand naming its
//! declaration. A block whose end is not reached ends the MIR block, which cannot go on.

use vun_hir::tree::ExprId;
use vun_inline::Rebind;
use vun_origin::OriginId;

use super::{Builder, Unbuilt};
use crate::place::Place;
use crate::{Operand, Rvalue, TerminatorKind};

impl Builder<'_, '_, '_> {
    pub(super) fn asm(&mut self, expr: ExprId, origin: OriginId) -> Result<Operand, Unbuilt> {
        let typed = self.typed().ok_or(Unbuilt::at(origin))?;
        let block = typed.asm.get(&expr).cloned().ok_or(Unbuilt::at(origin))?;
        let block = block.rebind(&mut Bindings(self))?;
        let returns = block.returns;
        let unit = self.temp(Rvalue::Asm(Box::new(block)), origin);
        Ok(if returns { unit } else { self.jump(TerminatorKind::Unreachable, origin) })
    }
}

/// The bindings of a block, built by the builder of the body holding it.
struct Bindings<'b, 'e, 'a, 'db>(&'b mut Builder<'e, 'a, 'db>);

impl Rebind<ExprId, ExprId, ExprId> for Bindings<'_, '_, '_, '_> {
    type Value = Operand;
    type Place = Place;
    type Symbol = Operand;
    type Error = Unbuilt;

    /// A constant is held by a temporary of the type the block was checked with: its own type is
    /// the narrowest its value fits, which a position of the instruction would not take.
    fn value(&mut self, value: ExprId) -> Result<Operand, Unbuilt> {
        let operand = self.0.operand(value)?;
        let ty = self.0.type_of(value);
        match operand {
            Operand::Const(ref constant) if ty.is_some() && constant.ty() != ty => {
                let origin = self.0.env.parsed.hir.exprs[value].origin;
                Ok(self.0.typed_temp(Rvalue::Use(operand), ty, origin))
            }
            _ => Ok(operand),
        }
    }

    fn place(&mut self, place: ExprId) -> Result<Place, Unbuilt> {
        self.0.place(place)
    }

    fn symbol(&mut self, symbol: ExprId) -> Result<Operand, Unbuilt> {
        self.0.operand(symbol)
    }
}
