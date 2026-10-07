//! Trap sites (spec 07, section 3): a call of the link symbol `lang.panic.trap` with the site's
//! `lang.panic.Trap`, whose id indexes the unit's message table, where the site is written; a
//! call of `lang.panic.fail` is one, saying its reason.

use vun_diag::Msg;
use vun_lir::{Address, CallKind, DataId, InstKind, Target, Terminator, Value};
use vun_origin::OriginId;
use vun_sema_ty::item::Site;
use vun_sema_ty::trap::Reason;
use vun_sema_ty::{Ty, Value as Constant};

use super::FnLower;
use crate::piece::{Bytes, SiteAt};
use crate::unlowered::{Form, Unlowered};

impl FnLower<'_, '_, '_> {
    /// Goes on where `holds` is not 0, and traps where it is, saying `why`.
    pub(crate) fn check(&mut self, holds: Value, why: &Reason) -> Result<(), Unlowered> {
        let (ok, fail) = (self.b.block(&[], self.at), self.b.block(&[], self.at));
        let to = |block| Target { block, args: Vec::new() };
        self.b.terminate(Terminator::Branch { cond: holds, then: to(ok), otherwise: to(fail) });
        self.b.switch_to(fail);
        self.trap(Some(why.msg()))?;
        self.b.switch_to(ok);
        Ok(())
    }

    /// Ends the current block with a trap at the origin lowered, saying `msg`.
    pub(crate) fn trap(&mut self, msg: Option<Msg>) -> Result<(), Unlowered> {
        self.site(self.at, msg)?;
        self.b.terminate(Terminator::Unreachable);
        Ok(())
    }

    /// A new site of the message table at `at`, saying `msg`: its id.
    pub(crate) fn new_site(&mut self, at: OriginId, msg: Option<Msg>) -> Result<u32, Unlowered> {
        self.l.site(vun_lir::Site { at, msg }).ok_or(Unlowered::unsupported(self.at, Form::Callee))
    }

    /// A constant of type `ty` holding the id of the site `site` at `offset`, zero elsewhere:
    /// the id the program gives the site, written as the program is assembled.
    pub(crate) fn holding_site(
        &mut self,
        ty: Ty,
        offset: u64,
        width: u64,
        site: u32,
    ) -> Result<DataId, Unlowered> {
        let layout = self.l.types.shape(ty, self.at)?.layout;
        let zeros = vec![0; usize::try_from(layout.size).unwrap_or_default()];
        let sites = vec![SiteAt { offset, width, site }];
        Ok(self.l.constant(Bytes { bytes: zeros, relocs: Vec::new(), sites }, layout.align, self.at))
    }

    /// Calls the trap handler for a new site at `site_at`, saying `msg`.
    pub(crate) fn site(&mut self, site_at: OriginId, msg: Option<Msg>) -> Result<(), Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Callee);
        let site = self.new_site(site_at, msg)?;
        let Constant::Type(ty) = self.l.item(vun_items::lang::panic::Trap, self.at)? else {
            return Err(unsupported());
        };
        let field = vun_items::lang::panic_Trap::site.symbol();
        let (offset, field) = self.l.types.field(ty, field).ok_or_else(unsupported)?;
        let width = self.l.types.shape(field, self.at)?.layout.size;
        let data = self.holding_site(ty, offset, width, site)?;
        let Ok(Site::Decl(handler)) = self.l.env.site(vun_items::lang::panic::trap) else {
            return Err(unsupported());
        };
        let callee = self.l.linked(&handler, self.at)?.callee().ok_or_else(unsupported)?;
        let addr = self.b.addr(Address::Data(data), self.at);
        self.b.push(InstKind::Call { callee, args: vec![addr], kind: CallKind::Plain }, None, self.at);
        Ok(())
    }
}
