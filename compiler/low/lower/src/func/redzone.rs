//! Redzones (spec 13, section 8.7): under the pointer checks, the slot of a local whose address
//! is taken starts at a shadow granule (`lang.check.granule`) and is followed by
//! `lang.check.redzone` bytes, poisoned while its function runs and made accessible again when
//! it returns, so an access through a raw pointer running past the slot fails its check.

use vun_hash::Set;
use vun_items::Item;
use vun_items::lang::{check, check_Violation as violation};
use vun_lir::{Address, BinOp, CastOp, InstKind, Slot, Value};
use vun_mir::{Local, Operand, Projection, Rvalue};
use vun_sema::checks::ChecksOf;
use vun_sema::item::Site;
use vun_sema::ty::Value as Constant;
use vun_target::Check;

use super::FnLower;
use crate::unlowered::{Form, Unlowered};

impl FnLower<'_, '_, '_> {
    /// The locals whose slots are guarded: under the pointer checks, every one whose address is
    /// taken, borrowed itself or a part of it rather than through a borrow it holds.
    pub(super) fn guarded(&self) -> Set<Local> {
        if !self.checked || !self.l.types.cx.get::<ChecksOf>(&()).contains(Check::Pointers) {
            return Set::default();
        }
        let stmts = self.body.blocks.iter().flat_map(|(_, b)| &b.stmts);
        stmts
            .filter_map(|s| match &s.value {
                Rvalue::Ref { place, .. } if !place.projection.contains(&Projection::Deref) => {
                    Some(place.local)
                }
                _ => None,
            })
            .collect()
    }

    /// A slot of `size` bytes aligned to `align` for `local`, and its redzone when it is
    /// `guarded`.
    pub(super) fn slot(
        &mut self,
        local: Local,
        size: u64,
        align: u64,
        guarded: &Set<Local>,
    ) -> Result<Slot, Unlowered> {
        let origin = self.body.locals[local].origin;
        if !guarded.contains(&local) {
            return Ok(self.b.slot(size, align, origin));
        }
        let (granule, redzone) = (self.bytes(check::granule)?, self.bytes(check::redzone)?);
        let slot = self.b.slot(size.next_multiple_of(granule) + redzone, align.max(granule), origin);
        self.guards.push((slot, size));
        Ok(slot)
    }

    /// Marks each guarded slot accessible, as far as it reaches, and its redzone poisoned.
    pub(super) fn guard(&mut self) -> Result<(), Unlowered> {
        let (granule, redzone) = (self.bytes(check::granule)?, self.bytes(check::redzone)?);
        let outside = self.scalar_operand(&Operand::Const(self.l.item(violation::outside, self.at)?))?;
        for (slot, size) in self.guards.clone() {
            let start = self.address(slot);
            let bytes = self.word(size);
            self.runtime(check::unpoison, vec![start, bytes])?;
            let past = self.word(size.next_multiple_of(granule));
            let past = self.b.binary(BinOp::Add, start, past, self.at);
            let bytes = self.word(redzone);
            self.runtime(check::poison, vec![past, bytes, outside])?;
        }
        Ok(())
    }

    /// Marks each guarded slot and its redzone accessible again, as the function returns: the
    /// stack past it is another function's next.
    pub(super) fn unguard(&mut self) -> Result<(), Unlowered> {
        let (granule, redzone) = (self.bytes(check::granule)?, self.bytes(check::redzone)?);
        for (slot, size) in self.guards.clone() {
            let start = self.address(slot);
            let bytes = self.word(size.next_multiple_of(granule) + redzone);
            self.runtime(check::unpoison, vec![start, bytes])?;
        }
        Ok(())
    }

    /// The bytes the constant `item` of `lang.check` gives.
    fn bytes(&self, item: Item) -> Result<u64, Unlowered> {
        match self.l.item(item, self.at)? {
            Constant::Int(n) => n.to_u128().and_then(|n| u64::try_from(n).ok()),
            _ => None,
        }
        .ok_or(Unlowered::unsupported(self.at, Form::Constant))
    }

    /// The address of `slot`, as an integer of the address's width.
    fn address(&mut self, slot: Slot) -> Value {
        let addr = self.b.slot_addr(slot, self.at);
        self.b.cast(CastOp::PtrToInt, addr, self.l.types.word, self.at)
    }

    fn word(&mut self, n: u64) -> Value {
        self.b.constant(self.l.types.word, u128::from(n), self.at)
    }

    /// Calls the operation `item` of `lang.check`, which the runtime provides, with `args`.
    fn runtime(&mut self, item: Item, args: Vec<Value>) -> Result<(), Unlowered> {
        let callee = self.provided(item)?.callee().ok_or(Unlowered::unsupported(self.at, Form::Callee))?;
        self.b.push(InstKind::Call { callee, args }, None, self.at);
        Ok(())
    }

    /// The address of the operation `item` of `lang.check`, which the runtime provides.
    pub(super) fn provided(&mut self, item: Item) -> Result<Address, Unlowered> {
        let Ok(Site::Decl(decl)) = self.l.env.site(item) else {
            return Err(Unlowered::unsupported(self.at, Form::Callee));
        };
        self.l.linked(&decl, self.at)
    }
}
