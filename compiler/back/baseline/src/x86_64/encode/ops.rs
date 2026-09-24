//! Moves, arithmetic, conversions and memory operations.

use super::super::{Cc, Mem, Reg, Rm, Size};
use super::{Alu, Asm, Bytes, Field, LOCK, REX, Shift, Unary};

impl Asm {
    pub fn mov_rr(&mut self, dst: Reg, src: Reg) {
        if dst != src {
            self.modrm(Size::B64, &[0x89], Field::Reg(src), Rm::Reg(dst));
        }
    }

    /// `dst = imm`, the 64 bits given.
    pub fn mov_ri(&mut self, dst: Reg, imm: u64) {
        if let Ok(small) = u32::try_from(imm) {
            if dst.high() {
                self.bytes(&[REX | 1]);
            }
            self.bytes(&[0xb8 + dst.low()]);
            self.bytes(&small.to_le_bytes());
        } else if let Ok(signed) = i32::try_from(imm.cast_signed()) {
            self.modrm(Size::B64, &[0xc7], Field::Digit(0), Rm::Reg(dst));
            self.bytes(&signed.to_le_bytes());
        } else {
            self.bytes(&[REX | 8 | u8::from(dst.high()), 0xb8 + dst.low()]);
            self.bytes(&imm.to_le_bytes());
        }
    }

    /// Loads `size` bytes at `src` into `dst`, zero-extended.
    pub fn load(&mut self, size: Size, dst: Reg, src: Mem) {
        match size {
            Size::B8 => self.modrm(Size::B32, &[0x0f, 0xb6], Field::Reg(dst), Rm::Mem(src)),
            Size::B16 => self.modrm(Size::B32, &[0x0f, 0xb7], Field::Reg(dst), Rm::Mem(src)),
            Size::B32 | Size::B64 => self.modrm(size, &[0x8b], Field::Reg(dst), Rm::Mem(src)),
        }
    }

    pub fn store(&mut self, size: Size, dst: Mem, src: Reg) {
        self.modrm(size, &[Self::by_size(size, 0x88, 0x89)], Field::Reg(src), Rm::Mem(dst));
    }

    pub fn lea(&mut self, dst: Reg, src: Mem) {
        self.modrm(Size::B64, &[0x8d], Field::Reg(dst), Rm::Mem(src));
    }

    /// `dst op= src`, or `cmp dst, src`.
    pub fn alu(&mut self, op: Alu, size: Size, dst: Reg, src: Reg) {
        let opcode = (op as u8) << 3 | Self::by_size(size, 0, 1);
        self.modrm(size, &[opcode], Field::Reg(src), Rm::Reg(dst));
    }

    /// `dst op= imm`, or `cmp dst, imm`.
    pub fn alu_imm(&mut self, op: Alu, size: Size, dst: Rm, imm: i32) {
        let digit = Field::Digit(op as u8);
        match (size, i8::try_from(imm)) {
            (Size::B8, _) => {
                self.modrm(size, &[0x80], digit, dst);
                self.bytes(&imm.to_le_bytes()[..1]);
            }
            (_, Ok(small)) => {
                self.modrm(size, &[0x83], digit, dst);
                self.bytes(&small.to_le_bytes());
            }
            (Size::B16, Err(_)) => {
                self.modrm(size, &[0x81], digit, dst);
                self.bytes(&imm.to_le_bytes()[..2]);
            }
            (_, Err(_)) => {
                self.modrm(size, &[0x81], digit, dst);
                self.bytes(&imm.to_le_bytes());
            }
        }
    }

    /// `dst *= src`, at 32 or 64 bits.
    pub fn imul(&mut self, size: Size, dst: Reg, src: Reg) {
        self.modrm(size, &[0x0f, 0xaf], Field::Reg(dst), Rm::Reg(src));
    }

    pub fn unary(&mut self, op: Unary, size: Size, rm: Rm) {
        self.modrm(size, &[Self::by_size(size, 0xf6, 0xf7)], Field::Digit(op as u8), rm);
    }

    /// `dst` shifted by `cl`.
    pub fn shift(&mut self, op: Shift, size: Size, dst: Reg) {
        self.modrm(size, &[Self::by_size(size, 0xd2, 0xd3)], Field::Digit(op as u8), Rm::Reg(dst));
    }

    /// `dst` shifted by `bits`.
    pub fn shift_imm(&mut self, op: Shift, size: Size, dst: Reg, bits: i32) {
        self.modrm(size, &[Self::by_size(size, 0xc0, 0xc1)], Field::Digit(op as u8), Rm::Reg(dst));
        self.bytes(&bits.to_le_bytes()[..1]);
    }

    /// Sign-extends `rax` into `rdx` for a division of `size` (32 or 64 bits).
    pub fn sign_extend_rax(&mut self, size: Size) {
        if size == Size::B64 {
            self.bytes(&[REX | 8]);
        }
        self.bytes(&[0x99]);
    }

    /// `dst` = 1 where `cc` holds, else 0, in its low byte.
    pub fn setcc(&mut self, cc: Cc, dst: Reg) {
        self.modrm(Size::B8, &[0x0f, 0x90 + cc as u8], Field::Digit(0), Rm::Reg(dst));
    }

    /// `dst` = the low `from` bits of `src`, zero-extended.
    pub fn zero_extend(&mut self, from: Size, dst: Reg, src: Reg) {
        let byte = Bytes { field: false, rm: true };
        match from {
            Size::B8 => self.encode(Size::B32, byte, &[0x0f, 0xb6], Field::Reg(dst), Rm::Reg(src)),
            Size::B16 => self.modrm(Size::B32, &[0x0f, 0xb7], Field::Reg(dst), Rm::Reg(src)),
            Size::B32 => self.modrm(Size::B32, &[0x89], Field::Reg(src), Rm::Reg(dst)),
            Size::B64 => self.mov_rr(dst, src),
        }
    }

    /// `dst` = the low `from` bits of `src`, sign-extended to 64 bits.
    pub fn sign_extend(&mut self, from: Size, dst: Reg, src: Reg) {
        let byte = Bytes { field: false, rm: true };
        match from {
            Size::B8 => self.encode(Size::B64, byte, &[0x0f, 0xbe], Field::Reg(dst), Rm::Reg(src)),
            Size::B16 => self.modrm(Size::B64, &[0x0f, 0xbf], Field::Reg(dst), Rm::Reg(src)),
            Size::B32 => self.modrm(Size::B64, &[0x63], Field::Reg(dst), Rm::Reg(src)),
            Size::B64 => self.mov_rr(dst, src),
        }
    }

    /// Sets the flags from `a & b`.
    pub fn test(&mut self, size: Size, a: Reg, b: Reg) {
        self.modrm(size, &[Self::by_size(size, 0x84, 0x85)], Field::Reg(b), Rm::Reg(a));
    }

    /// `dst = src` where `cc` holds.
    pub fn cmov(&mut self, cc: Cc, dst: Reg, src: Reg) {
        self.modrm(Size::B64, &[0x0f, 0x40 + cc as u8], Field::Reg(dst), Rm::Reg(src));
    }

    /// Exchanges `reg` with memory, atomically.
    pub fn xchg(&mut self, size: Size, mem: Mem, reg: Reg) {
        self.modrm(size, &[Self::by_size(size, 0x86, 0x87)], Field::Reg(reg), Rm::Mem(mem));
    }

    /// Adds `reg` to memory atomically, `reg` receiving what memory held.
    pub fn lock_xadd(&mut self, size: Size, mem: Mem, reg: Reg) {
        self.bytes(&[LOCK]);
        self.modrm(size, &[0x0f, Self::by_size(size, 0xc0, 0xc1)], Field::Reg(reg), Rm::Mem(mem));
    }

    /// Writes `reg` to memory where it holds `rax`, atomically; `rax` receives what it held.
    pub fn lock_cmpxchg(&mut self, size: Size, mem: Mem, reg: Reg) {
        self.bytes(&[LOCK]);
        self.modrm(size, &[0x0f, Self::by_size(size, 0xb0, 0xb1)], Field::Reg(reg), Rm::Mem(mem));
    }

    /// Copies `rcx` bytes from `rsi` to `rdi`.
    pub fn rep_movsb(&mut self) {
        self.bytes(&[0xf3, 0xa4]);
    }
}
