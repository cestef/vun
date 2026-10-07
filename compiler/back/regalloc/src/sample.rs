//! Functions and a machine to allocate them on, for tests and fuzzing: a toy architecture of
//! few registers, two-address arithmetic, calls passing arguments in the first registers, and
//! functions generated from a seed, a loop whose body computes, calls and branches.

use std::collections::HashMap;

use vun_index::Idx;
use vun_lir::{
    AbiType, BinOp, Block, Builder, CallKind, Callee, CastOp, Cond, Conv, Ext, FuncId, Function, Inst,
    InstKind, Linkage, Scalar, Signature, Target, Terminator, Value,
};
use vun_origin::OriginId;

use crate::{Bank, Describe, Machine, Need, Needs, Out, PReg, RegSet, Remade};

/// A machine of `regs` registers, the last `preserved` of them kept by calls, passing arguments
/// and results in the first ones.
pub struct Toy {
    machine: Machine,
    constants: HashMap<Value, u128>,
}

impl Toy {
    #[must_use]
    pub fn new(f: &Function, regs: u8, preserved: u8) -> Self {
        let general: Vec<PReg> = (0..regs).map(PReg).collect();
        let kept: Vec<PReg> = (regs - preserved..regs).map(PReg).collect();
        let constants = f.values.iter().filter_map(|(v, _)| Some((v, f.constant(v)?.1))).collect();
        Self { machine: Machine { general, float: Vec::new(), preserved: RegSet::of(&kept) }, constants }
    }

    fn arg(i: usize) -> PReg {
        PReg(u8::try_from(i).unwrap())
    }
}

impl Describe for Toy {
    fn machine(&self) -> &Machine {
        &self.machine
    }

    fn bank(&self, _: Scalar) -> Bank {
        Bank::General
    }

    /// Constants of up to 8 bits are taken as they are, and wider ones held.
    fn remade(&self, v: Value) -> Option<Remade> {
        let wide = |bits: u128| bits > u128::from(u8::MAX);
        self.constants.get(&v).map(|&bits| if wide(bits) { Remade::Held } else { Remade::Used })
    }

    fn inst(&self, f: &Function, inst: Inst, needs: &mut Needs) {
        let data = &f.insts[inst];
        match data.result.and_then(|v| Some((v, self.remade(v)?))) {
            Some((_, Remade::Used)) => return,
            Some((v, Remade::Held)) => return needs.defs.push((v, Out::Reg)),
            None => {}
        }
        let operands = data.kind.operands().filter(|&v| !self.recomputed(v));
        match &data.kind {
            InstKind::Call { .. } => {
                for (i, v) in operands.enumerate() {
                    needs.read(v, Need::Fixed(Self::arg(i)));
                }
                needs.defs.extend(data.result.map(|v| (v, Out::Fixed(PReg(0)))));
                needs.clobbers = self.machine.clobbered();
            }
            // A product through the first register, as x86-64's division goes through `rax`,
            // its result fixed there, the second operand read again after it is written.
            InstKind::Binary { op: BinOp::Mul, .. } => {
                for (i, v) in operands.enumerate() {
                    needs.uses.push(crate::Read { value: v, need: Need::Reg, late: i > 0 });
                }
                needs.reserved = RegSet::of(&[PReg(0)]);
                needs.defs.extend(data.result.map(|v| (v, Out::Fixed(PReg(0)))));
            }
            // A shift by a count in the second register, read after the result is written, as
            // x86-64 shifts by `cl`.
            InstKind::Binary { op: BinOp::Shl, a, b } if !self.recomputed(*a) && !self.recomputed(*b) => {
                needs.read(*a, Need::Reg);
                needs.uses.push(crate::Read { value: *b, need: Need::Fixed(PReg(1)), late: true });
                needs.defs.extend(data.result.map(|v| (v, Out::Reuse(0))));
            }
            // Two-address, as x86-64's arithmetic is.
            InstKind::Binary { .. } => {
                operands.for_each(|v| needs.read(v, Need::Reg));
                let out = if needs.uses.is_empty() { Out::Reg } else { Out::Reuse(0) };
                needs.defs.extend(data.result.map(|v| (v, out)));
            }
            // An extension reads its operand anywhere, as x86-64's `movzx` takes memory.
            InstKind::Cast { .. } => {
                operands.for_each(|v| needs.read(v, Need::Any));
                needs.defs.extend(data.result.map(|v| (v, Out::Reg)));
            }
            _ => {
                operands.for_each(|v| needs.read(v, Need::Reg));
                needs.defs.extend(data.result.map(|v| (v, Out::Reg)));
                needs.temps.push(Bank::General);
            }
        }
    }

    fn term(&self, f: &Function, block: Block, needs: &mut Needs) {
        match &f.blocks[block].term {
            Terminator::Branch { cond, .. } if !self.recomputed(*cond) => needs.read(*cond, Need::Reg),
            Terminator::Return(Some(v)) if !self.recomputed(*v) => {
                needs.read(*v, Need::Fixed(PReg(0)));
            }
            _ => {}
        }
    }

    fn entry(&self, f: &Function, needs: &mut Needs) {
        let params = &f.blocks[f.entry().unwrap()].params;
        needs.defs.extend(params.iter().enumerate().map(|(i, &p)| (p, Out::Fixed(Self::arg(i)))));
    }
}

/// A signature of `params` integers returning one.
#[must_use]
pub fn signature(params: usize) -> Signature {
    let int = AbiType::Int(Scalar::I64, Ext::Zero);
    Signature {
        conv: Conv::Vun,
        params: vec![int.clone(); params],
        result: Some(int),
        variadic: None,
        convention: None,
    }
}

/// A generator of functions: a loop whose body computes, calls and branches over values its
/// blocks dominate.
pub struct Random(pub u64);

impl Random {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        usize::try_from(self.0 % n as u64).unwrap()
    }

    fn pick(&mut self, pool: &[Value]) -> Value {
        pool[self.below(pool.len())]
    }

    /// Up to `count` operations over `pool`, each result joining it.
    fn compute(&mut self, b: &mut Builder, pool: &mut Vec<Value>, count: usize, calls: bool, at: OriginId) {
        for _ in 0..=self.below(count) {
            let (x, y) = (self.pick(pool), self.pick(pool));
            let v = match self.below(5) {
                // A call of the function itself, of three parameters.
                0 if calls => {
                    let args = (0..3).map(|_| self.pick(pool)).collect();
                    let call =
                        InstKind::Call { callee: Callee::Func(FuncId::new(0)), args, kind: CallKind::Plain };
                    b.push(call, Some(Scalar::I64), at).unwrap()
                }
                1 => b.compare(Cond::ULt, x, y, at),
                2 => b.binary(BinOp::Mul, x, y, at),
                3 => b.binary(BinOp::Shl, x, y, at),
                _ => b.binary(BinOp::Add, x, y, at),
            };
            let v = if b.ty(v) == Scalar::I64 { v } else { b.cast(CastOp::ZExt, v, Scalar::I64, at) };
            pool.push(v);
        }
    }

    /// A function of three parameters, calling itself when `calls`.
    #[must_use]
    pub fn function(&mut self, calls: bool, o: OriginId) -> Function {
        let mut b = Builder::new(Linkage::Internal, signature(3), o);
        let mut pool = b.params(b.current()).to_vec();
        // A constant taken as it is, and one held.
        pool.extend([b.constant(Scalar::I64, 7, o), b.constant(Scalar::I64, 1 << 40, o)]);
        self.compute(&mut b, &mut pool, 4, calls, o);
        let head = b.block(&[Scalar::I64, Scalar::I64], o);
        let [body, left, right, exit] = [0; 4].map(|_| b.block(&[], o));
        let join = b.block(&[Scalar::I64], o);
        let state = vec![self.pick(&pool), self.pick(&pool)];
        b.terminate(Terminator::Jump(Target { block: head, args: state }));
        b.switch_to(head);
        pool.extend_from_slice(b.params(head));
        // What the head dominates, which the exit may read.
        let outer = pool.clone();
        let more = b.compare(Cond::ULt, self.pick(&pool), self.pick(&pool), o);
        let to = |block| Target { block, args: Vec::new() };
        b.terminate(Terminator::Branch { cond: more, then: to(body), otherwise: to(exit) });
        b.switch_to(body);
        self.compute(&mut b, &mut pool, 6, calls, o);
        let side = b.compare(Cond::ULt, self.pick(&pool), self.pick(&pool), o);
        b.terminate(Terminator::Branch { cond: side, then: to(left), otherwise: to(right) });
        for arm in [left, right] {
            b.switch_to(arm);
            let mut local = pool.clone();
            self.compute(&mut b, &mut local, 3, calls, o);
            let out = self.pick(&local[pool.len() - 1..]);
            b.terminate(Terminator::Jump(Target { block: join, args: vec![out] }));
        }
        b.switch_to(join);
        pool.extend_from_slice(b.params(join));
        let back = vec![self.pick(&pool), self.pick(&pool)];
        b.terminate(Terminator::Jump(Target { block: head, args: back }));
        b.switch_to(exit);
        let result = self.pick(&outer);
        b.terminate(Terminator::Return(Some(result)));
        b.finish()
    }
}
