//! Operands as a call gives them, before a form takes them: a value of one of the architecture's
//! enums read as what the enum names (a condition; a register or a choice, as the enum's
//! attribute, a `Registers` or a `Choice`, says).

use vun_hir::tree::ExprId;
use vun_inline::{Base, Bound, ClassId, CondId, RegRef};
use vun_intern::Symbol;
use vun_isa::ChoiceId;
use vun_items::lang::arch_Registers as registers;
use vun_sema_ty::{Ty, TyKind, Value};

use super::check::Checker;
use super::table::{declared, position, variants_of};
use vun_sema::attributes::Attributed;
use vun_sema::target::Members;

/// An operand as written, as the architectures' assemblers fit it.
pub(crate) type Given = vun_isa::Given<ExprId, ExprId, ExprId>;

/// What an operand is as the register of an address.
pub(crate) trait Addressing {
    /// A register or a value read in one, as the base or index of an address.
    fn base(self) -> Option<Base<ExprId, ExprId>>;
}

impl Addressing for Given {
    fn base(self) -> Option<Base<ExprId, ExprId>> {
        match self {
            Self::Reg { reg, .. } => Some(Base::Reg(reg)),
            Self::Bound { bound: bound @ Bound::In { .. }, .. } => Some(Base::Bound(bound)),
            _ => None,
        }
    }
}

impl Checker<'_, '_, '_> {
    /// What the variant `name` of the enum `ty` names: a condition, where `ty` is the table's;
    /// else a register or a choice, as the enum's attribute says; `None` for any other.
    pub(super) fn word(&self, ty: Ty, name: Symbol) -> Option<Given> {
        let shape = self.arch.shape();
        if shape.cond.and_then(|cond| declared(self.env, cond).ok()) == Some(ty) {
            return Some(Given::Cond(CondId(position(&self.table.conds, name)?)));
        }
        let index = variants_of(self.env, ty).ok()?.iter().position(|&v| v == name)?;
        let TyKind::Nominal(nominal) = &*ty else { return None };
        let registers_ty = declared(self.env, shape.registers).ok()?;
        let choice_ty = shape.choice.and_then(|c| declared(self.env, c).ok());
        nominal.attributes(self.env.cx)?.values().into_iter().find_map(|attr| match attr {
            Value::Struct { ty: of, .. } if of == registers_ty => self.register(&attr, index),
            Value::Variant { ty: of, name: kind, .. } if Some(of) == choice_ty => {
                let kinds = variants_of(self.env, of).ok()?;
                let of = ChoiceId(position(&kinds, kind)?);
                Some(Given::Choice { of, index: u8::try_from(index).ok()? })
            }
            _ => None,
        })
    }

    /// The register at `index` of an enum whose `Registers` is `registers`.
    fn register(&self, registers: &Value, index: usize) -> Option<Given> {
        let fields = Members::of(registers).ok()?;
        let class = ClassId(position(&self.table.classes, fields.case(registers::class).ok()?)?);
        let size = match fields.option(registers::size).ok()? {
            Some(Value::Variant { name, .. }) => Some(self.table.size(*name)?),
            _ => None,
        };
        let step: usize = fields.int(registers::step).ok()?;
        let number = u8::try_from(index.checked_mul(step)?).ok()?;
        Some(Given::Reg { reg: RegRef { class, number }, size })
    }
}
