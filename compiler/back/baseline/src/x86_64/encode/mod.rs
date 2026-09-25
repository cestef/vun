//! The x86-64 encoder: each method appends one instruction's bytes to the code of a function.
//! Jumps to labels not bound yet are patched when the function ends; addresses the linker
//! places are relocations. Moves, arithmetic and memory operations are in `ops`.

mod ops;
mod sse;

pub use sse::Arith;

use vun_lir::Address;

use super::{Base, Cc, Mem, Reg, Rm, Size, Via, Xmm};
use crate::elf::RelocKind;

/// A place in a function's code that jumps name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Label(usize);

/// An address the linker writes into the code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodeReloc {
    pub offset: u64,
    pub target: Address,
    pub kind: RelocKind,
    pub addend: i64,
}

/// What `ModRM`'s `reg` field holds: a register, or an opcode extension.
#[derive(Clone, Copy)]
pub(super) enum Field {
    Reg(Reg),
    Xmm(Xmm),
    Digit(u8),
}

/// Which operands of an instruction are byte registers.
#[derive(Clone, Copy)]
pub(super) struct Bytes {
    pub(super) field: bool,
    pub(super) rm: bool,
}

/// An arithmetic or logic operation of the `add` family, by its opcode extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alu {
    Add = 0,
    Or = 1,
    And = 4,
    Sub = 5,
    Xor = 6,
    Cmp = 7,
}

/// A one-operand operation of the `F7` group, by its opcode extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unary {
    Neg = 3,
    Div = 6,
    Idiv = 7,
}

/// A shift by `cl`, by its opcode extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shift {
    Shl = 4,
    Shr = 5,
    Sar = 7,
}

#[derive(Default)]
pub struct Asm {
    pub code: Vec<u8>,
    pub relocs: Vec<CodeReloc>,
    labels: Vec<Option<u64>>,
    /// The rel32 fields written before their labels were bound.
    fixups: Vec<(u64, Label)>,
}

pub(super) const REX: u8 = 0x40;
const OPERAND_16: u8 = 0x66;
pub(super) const LOCK: u8 = 0xf0;

impl Asm {
    #[must_use]
    pub fn label(&mut self) -> Label {
        self.labels.push(None);
        Label(self.labels.len() - 1)
    }

    /// Binds `label` to where the next instruction goes.
    pub fn bind(&mut self, label: Label) {
        self.labels[label.0] = Some(self.here());
    }

    #[must_use]
    pub fn here(&self) -> u64 {
        self.code.len() as u64
    }

    /// Writes each jump's distance to its label.
    ///
    /// # Panics
    /// Panics if a jump's label was never bound, or is further than a rel32 reaches.
    pub fn finish(&mut self) {
        for &(at, label) in &self.fixups {
            let target = self.labels[label.0].expect("every label jumped to is bound");
            let from = at + 4;
            let rel =
                i32::try_from(i128::from(target) - i128::from(from)).expect("a function spans under 2 GiB");
            let at = usize::try_from(at).expect("code fits memory");
            self.code[at..at + 4].copy_from_slice(&rel.to_le_bytes());
        }
    }

    /// Writes `value` over the four bytes at `at`, a field reserved earlier.
    ///
    /// # Panics
    /// Panics if `at` is not in the code.
    pub fn patch32(&mut self, at: u64, value: u32) {
        let at = usize::try_from(at).expect("code fits memory");
        self.code[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }

    pub(super) fn bytes(&mut self, bytes: &[u8]) {
        self.code.extend_from_slice(bytes);
    }

    /// One instruction over `ModRM` of the operation size `size`, its register operands bytes
    /// where it is 8 bits.
    pub(super) fn modrm(&mut self, size: Size, opcode: &[u8], field: Field, rm: Rm) {
        let byte = size == Size::B8;
        self.encode(size, Bytes { field: byte, rm: byte }, opcode, field, rm);
    }

    /// One instruction over `ModRM`: prefixes, REX, `opcode`, `ModRM` with SIB and displacement.
    /// A byte register past the first four needs REX, even with no bit of it set.
    pub(super) fn encode(&mut self, size: Size, bytes: Bytes, opcode: &[u8], field: Field, rm: Rm) {
        if size == Size::B16 {
            self.bytes(&[OPERAND_16]);
        }
        let (reg, reg_high, reg_byte) = match field {
            Field::Reg(r) => (r.low(), r.high(), bytes.field && r.byte_needs_rex()),
            Field::Xmm(x) => (x.low(), x.high(), false),
            Field::Digit(d) => (d, false, false),
        };
        let (base_high, rm_byte) = match rm {
            Rm::Reg(r) => (r.high(), bytes.rm && r.byte_needs_rex()),
            Rm::Xmm(x) => (x.high(), false),
            Rm::Mem(Mem { base: Base::Reg(r), .. }) => (r.high(), false),
            Rm::Mem(Mem { base: Base::Rip(..), .. }) => (false, false),
        };
        let rex = REX | u8::from(size == Size::B64) << 3 | u8::from(reg_high) << 2 | u8::from(base_high);
        if rex != REX || reg_byte || rm_byte {
            self.bytes(&[rex]);
        }
        self.bytes(opcode);
        match rm {
            Rm::Reg(r) => self.bytes(&[0xc0 | reg << 3 | r.low()]),
            Rm::Xmm(x) => self.bytes(&[0xc0 | reg << 3 | x.low()]),
            Rm::Mem(Mem { base: Base::Reg(b), disp }) => {
                let sib = b.low() == Reg::Rsp.low();
                let (mode, wide) = match disp {
                    0 if b.low() != Reg::Rbp.low() => (0b00, None),
                    d if i8::try_from(d).is_ok() => (0b01, Some(false)),
                    _ => (0b10, Some(true)),
                };
                self.bytes(&[mode << 6 | reg << 3 | b.low()]);
                if sib {
                    self.bytes(&[0x24]);
                }
                match wide {
                    Some(false) => self.bytes(&disp.to_le_bytes()[..1]),
                    Some(true) => self.bytes(&disp.to_le_bytes()),
                    None => {}
                }
            }
            Rm::Mem(Mem { base: Base::Rip(target, via), disp }) => {
                self.bytes(&[reg << 3 | 0b101]);
                let kind = match via {
                    Via::Direct => RelocKind::Pc32,
                    Via::Got => RelocKind::GotPc32,
                };
                // The displacement is from the end of the instruction, which ends here.
                self.relocs.push(CodeReloc {
                    offset: self.here(),
                    target,
                    kind,
                    addend: i64::from(disp) - 4,
                });
                self.bytes(&[0; 4]);
            }
        }
    }

    /// The opcode of the byte form where `size` is 8 bits, else the other.
    pub(super) const fn by_size(size: Size, byte: u8, other: u8) -> u8 {
        if matches!(size, Size::B8) { byte } else { other }
    }

    fn rel32(&mut self, label: Label) {
        self.fixups.push((self.here(), label));
        self.bytes(&[0; 4]);
    }

    pub fn jmp(&mut self, label: Label) {
        self.bytes(&[0xe9]);
        self.rel32(label);
    }

    pub fn jcc(&mut self, cc: Cc, label: Label) {
        self.bytes(&[0x0f, 0x80 + cc as u8]);
        self.rel32(label);
    }

    /// Calls a function the linker places.
    pub fn call(&mut self, target: Address) {
        self.bytes(&[0xe8]);
        self.relocs.push(CodeReloc { offset: self.here(), target, kind: RelocKind::Plt32, addend: -4 });
        self.bytes(&[0; 4]);
    }

    pub fn call_reg(&mut self, target: Reg) {
        self.modrm(Size::B32, &[0xff], Field::Digit(2), Rm::Reg(target));
    }

    /// `push rbp; mov rbp, rsp; sub rsp, imm32`: the offset of the frame size, patched once
    /// known.
    pub fn prologue(&mut self) -> u64 {
        self.bytes(&[0x55, REX | 8, 0x89, 0xe5, REX | 8, 0x81, 0xec]);
        let at = self.here();
        self.bytes(&[0; 4]);
        at
    }

    /// `leave; ret`.
    pub fn epilogue(&mut self) {
        self.bytes(&[0xc9, 0xc3]);
    }

    pub fn ud2(&mut self) {
        self.bytes(&[0x0f, 0x0b]);
    }
}
