//! Narrowing truncations (spec 13, section 3): the low bits of a sum, difference, product or
//! bitwise operation are that operation on its operands' low bits, and those of a widened value
//! are the value's. A truncation of such an operation, read nowhere else, computes it at the
//! narrow width from its operands narrowed in turn; the wide operation, several instructions
//! once legalized when wider than the machine, is left unread and swept (`Function::sweep`).
//! `(x * k).wrap(u64)` of a `u64` becomes one 64-bit product, not a 128-bit one.

use vun_index::IndexVec;
use vun_origin::OriginId;

use crate::entity::{Block, Inst, Value};
use crate::func::{Def, Function};
use crate::inst::{BinOp, CastOp, InstKind};
use crate::replaced::Replaced;
use crate::ty::Scalar;

/// How many operations deep a truncation is pushed into those computing its operand.
const DEEPEST: u32 = 16;

impl Function {
    /// Computes each truncation of an operation its low bits depend on alone at the narrow width
    /// (see the module) on a machine whose addresses are `pointer_bits` wide, returning how many
    /// were.
    pub fn narrow(&mut self, pointer_bits: u32) -> usize {
        let mut narrowing = Narrowing {
            word: pointer_bits,
            uses: self.uses(),
            replaced: Replaced::default(),
            insts: Vec::new(),
        };
        let blocks: Vec<Block> = self.blocks.iter().map(|(b, _)| b).collect();
        for block in blocks {
            for inst in std::mem::take(&mut self.blocks[block].insts) {
                let data = &self.insts[inst];
                if let (InstKind::Cast { op: CastOp::Trunc, value, to }, Some(result)) =
                    (&data.kind, data.result)
                    && let (value, to, origin) = (*value, *to, data.origin)
                    && let Some(low) = narrowing.low(self, value, to, origin, DEEPEST)
                {
                    narrowing.replaced.insert(result, low);
                    continue;
                }
                narrowing.insts.push(inst);
            }
            self.blocks[block].insts = std::mem::take(&mut narrowing.insts);
        }
        let count = narrowing.replaced.len();
        if count > 0 {
            for (_, data) in self.blocks.iter_mut() {
                for &inst in &data.insts {
                    self.insts[inst].kind.map_values(|v| narrowing.replaced.resolve(v));
                }
                data.term.map_values(|v| narrowing.replaced.resolve(v));
            }
            self.compact();
        }
        count
    }
}

struct Narrowing {
    /// The machine's word, in bits: an operation this wide or narrower is narrowed only where an
    /// operand narrows without a truncation, since truncations would otherwise replace it.
    word: u32,
    uses: IndexVec<Value, u32>,
    replaced: Replaced,
    /// The instructions of the block being narrowed, those made for it included.
    insts: Vec<Inst>,
}

impl Narrowing {
    /// The low `to` bits of `value` computed without truncating it, where they can be; new
    /// instructions come from `origin` when they stand for no wide one.
    fn low(
        &mut self,
        f: &mut Function,
        value: Value,
        to: Scalar,
        origin: OriginId,
        depth: u32,
    ) -> Option<Value> {
        let value = self.replaced.resolve(value);
        if f.ty(value) == to {
            return Some(value);
        }
        let Def::Inst(def) = f.values[value].def else { return None };
        let at = f.insts[def].origin;
        let kind = match f.insts[def].kind {
            InstKind::Const { bits, .. } => InstKind::Const { ty: to, bits: to.truncate(bits.cast_signed()) },
            InstKind::Cast { op: op @ (CastOp::ZExt | CastOp::SExt), value: inner, .. } => {
                if f.ty(inner).bits(0) >= to.bits(0) {
                    return Some(self.low_or_truncated(f, inner, to, origin, depth));
                }
                InstKind::Cast { op, value: inner, to }
            }
            InstKind::Cast { op: CastOp::Trunc, value: inner, .. } => {
                return Some(self.low_or_truncated(f, inner, to, origin, depth));
            }
            InstKind::Binary {
                op: op @ (BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::And | BinOp::Or | BinOp::Xor),
                a,
                b,
            } if depth > 0 && self.uses.get(value) == Some(&1) => {
                let (la, lb) = (self.low(f, a, to, at, depth - 1), self.low(f, b, to, at, depth - 1));
                if la.is_none() && lb.is_none() && f.ty(value).bits(0) <= self.word {
                    return None;
                }
                let a = la.unwrap_or_else(|| self.truncated(f, a, to, at));
                let b = lb.unwrap_or_else(|| self.truncated(f, b, to, at));
                InstKind::Binary { op, a, b }
            }
            _ => return None,
        };
        Some(self.define(f, kind, to, at))
    }

    /// The low `to` bits of `value`: computed narrow where they can be, truncated otherwise.
    fn low_or_truncated(
        &mut self,
        f: &mut Function,
        value: Value,
        to: Scalar,
        origin: OriginId,
        depth: u32,
    ) -> Value {
        self.low(f, value, to, origin, depth).unwrap_or_else(|| self.truncated(f, value, to, origin))
    }

    fn truncated(&mut self, f: &mut Function, value: Value, to: Scalar, origin: OriginId) -> Value {
        let value = self.replaced.resolve(value);
        self.define(f, InstKind::Cast { op: CastOp::Trunc, value, to }, to, origin)
    }

    fn define(&mut self, f: &mut Function, kind: InstKind, ty: Scalar, origin: OriginId) -> Value {
        f.emit(kind, Some(ty), origin, &mut self.insts).expect("a typed instruction defines its value")
    }
}
