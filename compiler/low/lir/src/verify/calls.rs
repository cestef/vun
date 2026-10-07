//! Calls: the callee exists, the arguments and result are of the signature's types, and a tail
//! call can replace the frame of its function.

use crate::entity::Inst;
use crate::func::Signature;
use crate::inst::{Address, CallKind, Callee, InstKind, Terminator};
use crate::unit::ImportKind;

use super::{Checker, Entity, Malformed, Reason};

impl Checker<'_> {
    /// The values a call passes, by its callee.
    fn callee_signature<'a>(&'a self, at: Entity, callee: &'a Callee) -> Result<&'a Signature, Malformed> {
        Ok(match callee {
            Callee::Func(f) => {
                self.unit.address(at, Address::Func(*f))?;
                &self.unit.functions[*f].signature
            }
            Callee::Import(i) => {
                self.unit.address(at, Address::Import(*i))?;
                match &self.unit.imports[*i].kind {
                    ImportKind::Func(signature) => signature,
                    ImportKind::Data => {
                        return Self::fail(at, Reason::UnknownAddress(Address::Import(*i)));
                    }
                }
            }
            Callee::Indirect(_, signature) => signature,
        })
    }

    pub(super) fn call(&self, inst: Inst) -> Result<(), Malformed> {
        let at = Entity::Inst(self.id, inst);
        let InstKind::Call { callee, args, kind } = &self.func.insts[inst].kind else { return Ok(()) };
        let signature = self.callee_signature(at, callee)?;
        let (params, returns) = (signature.values(), signature.returns());
        if *kind == CallKind::Tail && !self.replaces(inst, signature) {
            return Self::fail(at, Reason::Tail);
        }
        if params.len() != args.len() {
            return Self::fail(at, Reason::Arity);
        }
        for (&expected, &arg) in params.iter().zip(args) {
            self.expect(at, arg, expected)?;
        }
        let result = self.func.insts[inst].result.map(|v| self.func.ty(v));
        if result == returns { Ok(()) } else { Self::fail(at, Reason::Return) }
    }

    /// Whether the call `inst`, of a callee with `signature`, can replace the frame: it ends its
    /// block by returning its value, as a function of the callee's convention and result, which
    /// takes the result in registers.
    fn replaces(&self, inst: Inst, signature: &Signature) -> bool {
        let Some(block) = self.homes[inst] else { return false };
        let data = &self.func.blocks[block];
        let own = &self.func.signature;
        data.insts.last() == Some(&inst)
            && data.term == Terminator::Return(self.func.insts[inst].result)
            && own.conv == signature.conv
            && own.convention == signature.convention
            && own.returns() == signature.returns()
            && !own.through()
            && !signature.through()
            && signature.variadic.is_none()
    }
}
