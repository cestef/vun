//! Trap sites (spec 07, section 3): a call of the link symbol `lang.panic.trap` with the site's
//! `lang.panic.Trap`, whose id indexes the unit's message table, where the site is written.

use vun_lir::{Address, Callee, InstKind, Target, Terminator, Value};
use vun_sema::int::Integer;
use vun_sema::item::Site;
use vun_sema::ty::Value as Constant;

use super::FnLower;
use crate::unlowered::{Form, Unlowered};

impl FnLower<'_, '_, '_> {
    /// Goes on where `holds` is not 0, and traps where it is.
    pub(crate) fn check(&mut self, holds: Value) -> Result<(), Unlowered> {
        let (ok, fail) = (self.b.block(&[], self.at), self.b.block(&[], self.at));
        let to = |block| Target { block, args: Vec::new() };
        self.b.terminate(Terminator::Branch { cond: holds, then: to(ok), otherwise: to(fail) });
        self.b.switch_to(fail);
        self.trap()?;
        self.b.switch_to(ok);
        Ok(())
    }

    /// Ends the current block with a trap at the origin lowered.
    pub(crate) fn trap(&mut self) -> Result<(), Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Callee);
        let site = u32::try_from(self.l.unit.sites.len()).map_err(|_| unsupported())?;
        self.l.unit.sites.push(self.at);
        let Constant::Type(ty) = self.l.item(vun_items::lang::panic::Trap, self.at)? else {
            return Err(unsupported());
        };
        let fields =
            vec![(vun_items::lang::panic_Trap::site.symbol(), Constant::Int(Integer::from(site as usize)))];
        let bytes = self.l.encode(&Constant::Struct { ty, fields }, ty, self.at)?;
        let align = self.l.types.shape(ty, self.at)?.layout.align;
        let data = self.l.constant(bytes.bytes, bytes.relocs, align, self.at);
        let Ok(Site::Decl(handler)) = self.l.env.site(vun_items::lang::panic::trap) else {
            return Err(unsupported());
        };
        let handler = self.l.import(&handler, self.at)?;
        let addr = self.b.addr(Address::Data(data), self.at);
        self.b.push(InstKind::Call { callee: Callee::Import(handler), args: vec![addr] }, None, self.at);
        self.b.terminate(Terminator::Unreachable);
        Ok(())
    }
}
