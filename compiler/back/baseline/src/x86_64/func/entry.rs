//! Receiving a function's parameters where its convention passes them: in registers, on the
//! caller's stack, or as records split over registers or copied.

use super::super::abi::{ARGS, Carrier, Loc, Ret};
use super::super::{Mem, Reg, Size};
use super::{CALLER_ARGS, FnCompiler, HOME};

impl FnCompiler<'_> {
    /// Receives the signature's values where the convention passes them.
    pub(super) fn entry(&mut self) {
        let Some(entry) = self.f.entry() else { return };
        let params = self.f.blocks[entry].params.clone();
        let locs = self.abi.params.clone();
        // Values in general registers are bound first, so that receiving one from an SSE
        // register, which takes a general register, overwrites none still to be received.
        let (sse, rest): (Vec<_>, Vec<_>) =
            params.iter().zip(&locs).partition(|(_, loc)| matches!(loc, Loc::Reg(Carrier::Sse(_))));
        for (&v, loc) in rest.into_iter().chain(sse) {
            match loc {
                Loc::Reg(carrier) => {
                    let bytes = u64::from(Size::of(self.f.ty(v)).map_or(8, Size::bytes));
                    self.receive(v, *carrier, bytes);
                }
                Loc::Stack(offset) => {
                    self.home[v] = Some(CALLER_ARGS + i32::try_from(*offset).expect("fits"));
                    self.saved[v] = true;
                    if let Some(r) = self.fixed[v] {
                        let home = self.home(v);
                        self.asm.load(Size::B64, r, home);
                    }
                }
                Loc::Copied { offset, .. } => {
                    self.frame_addr.insert(v, CALLER_ARGS + i32::try_from(*offset).expect("fits"));
                }
                Loc::Split(parts) => {
                    let buffer = self.reserve(parts.len() as u64 * HOME, HOME);
                    for (i, &(carrier, _)) in parts.iter().enumerate() {
                        let at = Mem::at(Reg::Rbp, buffer + i32::try_from(i as u64 * HOME).expect("fits"));
                        self.store_part(at, carrier, HOME);
                    }
                    self.frame_addr.insert(v, buffer);
                }
                Loc::Returned => {
                    let Ret::Split(parts) = &self.abi.ret else { continue };
                    let buffer = self.reserve(parts.len() as u64 * HOME, HOME);
                    self.frame_addr.insert(v, buffer);
                    self.gathered = Some(buffer);
                }
            }
        }
        if self.abi.ret == Ret::Hidden {
            let home = self.reserve(HOME, HOME);
            self.asm.store(Size::B64, Mem::at(Reg::Rbp, home), ARGS[0]);
            self.hidden = Some(home);
        }
        // An entry that is also jumped to receives its parameters in their homes.
        if !self.cfg.preds[entry].is_empty() {
            self.spill_live();
            self.clear();
        }
    }
}
