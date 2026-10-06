//! Encoding function bodies: each instruction's bytes, and the relocations of the immediates a
//! linker rewrites, padded to a fixed width.

use vun_bytes::{Out, PADDED};
use vun_isa_wasm32::{Imm, Kind as Position};
use vun_wasm::format::op::{END, I32_CONST};

use super::link::{Reloc, RelocKind};
use super::op::{Asm, Num, Op, Operand};
use super::{Kind, Linker, Module, SymbolId};

/// The block type of blocks taking and giving nothing.
const EMPTY: u8 = 0x40;

/// A function body being encoded, with the relocations of its immediates.
pub(super) struct Body<'m> {
    pub out: Out,
    pub relocs: Vec<Reloc>,
    pub module: &'m Module,
    pub functions: &'m [SymbolId],
    pub placed: &'m [u64],
}

impl Body<'_> {
    /// A padded immediate the linker rewrites, relocated by `kind` to `index`.
    fn relocated(&mut self, kind: RelocKind, index: u32, addend: i32, value: u32) {
        let offset = u32::try_from(self.out.here()).expect("bodies fit 4 GiB");
        self.relocs.push(Reloc { kind, offset, index, addend });
        match kind {
            RelocKind::TableIndexSleb | RelocKind::MemoryAddrSleb => {
                self.out.sleb_padded(i32::from_ne_bytes(value.to_ne_bytes()));
            }
            _ => self.out.uleb_padded(value),
        }
        debug_assert_eq!(u64::from(offset) + PADDED as u64, self.out.here());
    }

    fn function_index(&self, sym: SymbolId) -> u32 {
        let at = self.functions.iter().position(|&f| f == sym).expect("a called symbol is a function");
        u32::try_from(at).expect("functions fit u32")
    }

    pub(super) fn op(&mut self, op: &Op) {
        let out = &mut self.out;
        match op {
            Op::Unreachable => out.u8(0x00),
            Op::Block => out.bytes(&[0x02, EMPTY]),
            Op::Loop => out.bytes(&[0x03, EMPTY]),
            Op::If => out.bytes(&[0x04, EMPTY]),
            Op::Else => out.u8(0x05),
            Op::End => out.u8(END),
            Op::Br(l) | Op::BrIf(l) => {
                out.u8(if matches!(op, Op::Br(_)) { 0x0c } else { 0x0d });
                out.uleb(u64::from(*l));
            }
            Op::BrTable { cases, default } => {
                out.u8(0x0e);
                out.uleb(cases.len() as u64);
                for &l in cases.iter().chain([default]) {
                    out.uleb(u64::from(l));
                }
            }
            Op::Return => out.u8(0x0f),
            Op::Call(sym) => {
                out.u8(0x10);
                let index = self.function_index(*sym);
                self.relocated(RelocKind::FunctionIndexLeb, sym.number(), 0, index);
            }
            Op::CallIndirect(ty) => {
                out.u8(0x11);
                self.relocated(RelocKind::TypeIndexLeb, ty.number(), 0, ty.number());
                let table = self.module.linker(Linker::Table).expect("an indirect call names the table");
                self.relocated(RelocKind::TableNumberLeb, table.number(), 0, 0);
            }
            Op::Drop => out.u8(0x1a),
            Op::Select => out.u8(0x1b),
            Op::LocalGet(l) | Op::LocalSet(l) | Op::LocalTee(l) => {
                out.u8(match op {
                    Op::LocalGet(_) => 0x20,
                    Op::LocalSet(_) => 0x21,
                    _ => 0x22,
                });
                out.uleb(u64::from(*l));
            }
            Op::GlobalGet(sym) | Op::GlobalSet(sym) => {
                out.u8(if matches!(op, Op::GlobalGet(_)) { 0x23 } else { 0x24 });
                self.relocated(RelocKind::GlobalIndexLeb, sym.number(), 0, 0);
            }
            Op::Load(mem) | Op::Store(mem) => {
                let (load, store) = mem.accesses().expect("verified accesses have codes");
                out.u8(if matches!(op, Op::Load(_)) { load.code } else { store.code });
                out.uleb(u64::from(mem.bytes.trailing_zeros()));
                out.uleb(0);
            }
            Op::MemoryCopy => {
                out.u8(0xfc);
                out.uleb(10);
                out.bytes(&[0, 0]);
            }
            Op::I32Const(v) => {
                out.u8(I32_CONST);
                out.sleb(i64::from(*v));
            }
            Op::I64Const(v) => {
                out.u8(0x42);
                out.sleb(*v);
            }
            Op::F32Const(bits) => {
                out.u8(0x43);
                out.u32(*bits);
            }
            Op::F64Const(bits) => {
                out.u8(0x44);
                out.u64(*bits);
            }
            Op::Address(sym, addend) => {
                out.u8(I32_CONST);
                let (kind, value) = match self.module.symbols[*sym].kind {
                    Kind::Func { .. } => (RelocKind::TableIndexSleb, 0),
                    Kind::Data(Some(at)) => {
                        (RelocKind::MemoryAddrSleb, Self::address(self.placed[at], *addend))
                    }
                    _ => (RelocKind::MemoryAddrSleb, 0),
                };
                self.relocated(kind, sym.number(), *addend, value);
            }
            Op::Num(n) => self.num(*n),
            Op::Asm(asm) => self.asm(asm),
        }
    }

    fn address(at: u64, addend: i32) -> u32 {
        let at = i64::try_from(at).expect("memory fits") + i64::from(addend);
        u32::try_from(at).expect("addresses of wasm32 fit u32")
    }

    /// An instruction of a block: its opcode, then each operand as its kind says, then the bytes
    /// its form ends with.
    fn asm(&mut self, asm: &Asm) {
        let form = vun_isa_wasm32::Op::entry(asm.op, asm.form).1;
        match form.prefix() {
            Some(prefix) => {
                self.out.u8(prefix);
                self.out.uleb(u64::from(form.code()));
            }
            None => self.out.u8(u8::try_from(form.code()).expect("unprefixed codes are bytes")),
        }
        for (arg, operand) in form.args.iter().zip(&asm.operands) {
            match (arg.kind, operand) {
                (Position::Imm(Imm::Signed(_)), Operand::Imm(v)) => {
                    self.out.sleb(*v);
                }
                (Position::Imm(Imm::Bits(bits)), Operand::Imm(v)) => {
                    self.out.bytes(&v.to_le_bytes()[..usize::from(bits / 8)]);
                }
                (_, Operand::Imm(v)) => self.out.uleb(v.cast_unsigned()),
                (_, Operand::Local(l)) => self.out.uleb(u64::from(*l)),
            }
        }
        self.out.bytes(form.tail);
    }

    fn num(&mut self, n: Num) {
        match n.code() {
            (Some(prefix), code) => {
                self.out.u8(prefix);
                self.out.uleb(u64::from(code));
            }
            (None, code) => self.out.u8(u8::try_from(code).expect("unprefixed codes are bytes")),
        }
    }
}
