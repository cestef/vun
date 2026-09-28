//! Terminators: jump arguments as halves, a switch on a wide integer as a chain of equalities,
//! a wide result written through the address the function receives.

use vun_origin::OriginId;

use vun_index::Idx;

use super::Rewrite;
use vun_lir::{Access, Target, Terminator};

impl Rewrite<'_> {
    pub(super) fn target(&self, t: &Target) -> Target {
        Target {
            block: self.blocks[t.block.index()],
            args: t.args.iter().flat_map(|&a| self.flat(a)).collect(),
        }
    }

    pub(super) fn terminator(&mut self, term: &Terminator, at: OriginId) {
        let term = match term {
            Terminator::Jump(t) => Terminator::Jump(self.target(t)),
            Terminator::Branch { cond, then, otherwise } => Terminator::Branch {
                cond: self.one(*cond),
                then: self.target(then),
                otherwise: self.target(otherwise),
            },
            Terminator::Switch { value, cases, default } if self.is_wide(*value) => {
                // Case by case, each an equality of pairs.
                let value = self.pair(*value);
                for (bits, t) in cases {
                    let mut h = self.halves(at);
                    let case = h.pair(*bits);
                    let equal = h.compared(vun_lir::Cond::Eq, value, case);
                    let next = self.b.block(&[], at);
                    let then = self.target(t);
                    self.b.terminate(Terminator::Branch {
                        cond: equal,
                        then,
                        otherwise: Target { block: next, args: Vec::new() },
                    });
                    self.b.switch_to(next);
                }
                Terminator::Jump(self.target(default))
            }
            Terminator::Switch { value, cases, default } => Terminator::Switch {
                value: self.one(*value),
                cases: cases.iter().map(|(bits, t)| (*bits, self.target(t))).collect(),
                default: self.target(default),
            },
            Terminator::Return(Some(v)) if self.is_wide(*v) => {
                let (value, addr) =
                    (self.pair(*v), self.result.expect("a wide result's address is received"));
                self.store_pair(value, addr, Access::Plain, at);
                Terminator::Return(None)
            }
            Terminator::Return(v) => Terminator::Return(v.map(|v| self.one(v))),
            Terminator::Unreachable => Terminator::Unreachable,
        };
        self.b.terminate(term);
    }
}
