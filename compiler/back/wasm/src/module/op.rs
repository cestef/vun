//! Instructions as a typed set: each declares its encoding and, for the validator, the operand
//! types it pops and the one it pushes, once.

use super::{SymbolId, TypeId, ValType};

/// Numeric instructions: an opcode, the operands popped (the last on top) and the result pushed.
macro_rules! numeric {
    ($($name:ident $mnemonic:literal = $code:literal $(/ $prefix:literal)?: [$($pop:ident),*] -> $push:ident;)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Num {
            $($name,)*
        }

        impl Num {
            /// The prefix byte of an instruction that has one, and its code.
            #[must_use]
            pub const fn code(self) -> (Option<u8>, u32) {
                match self {
                    $(Self::$name => (numeric!(@prefix $($prefix)?), $code),)*
                }
            }

            /// The operand types popped, the one on top last.
            #[must_use]
            pub const fn pops(self) -> &'static [ValType] {
                match self {
                    $(Self::$name => &[$(ValType::$pop),*],)*
                }
            }

            /// Its name in the text format.
            #[must_use]
            pub const fn mnemonic(self) -> &'static str {
                match self {
                    $(Self::$name => $mnemonic,)*
                }
            }

            #[must_use]
            pub const fn pushes(self) -> ValType {
                match self {
                    $(Self::$name => ValType::$push,)*
                }
            }
        }
    };
    (@prefix $prefix:literal) => { Some($prefix) };
    (@prefix) => { None };
}

numeric! {
    I32Eqz "i32.eqz" = 0x45: [I32] -> I32;
    I32Eq "i32.eq" = 0x46: [I32, I32] -> I32;
    I32Ne "i32.ne" = 0x47: [I32, I32] -> I32;
    I32LtS "i32.lt_s" = 0x48: [I32, I32] -> I32;
    I32LtU "i32.lt_u" = 0x49: [I32, I32] -> I32;
    I32GtS "i32.gt_s" = 0x4a: [I32, I32] -> I32;
    I32GtU "i32.gt_u" = 0x4b: [I32, I32] -> I32;
    I32LeS "i32.le_s" = 0x4c: [I32, I32] -> I32;
    I32LeU "i32.le_u" = 0x4d: [I32, I32] -> I32;
    I32GeS "i32.ge_s" = 0x4e: [I32, I32] -> I32;
    I32GeU "i32.ge_u" = 0x4f: [I32, I32] -> I32;
    I64Eq "i64.eq" = 0x51: [I64, I64] -> I32;
    I64Ne "i64.ne" = 0x52: [I64, I64] -> I32;
    I64LtS "i64.lt_s" = 0x53: [I64, I64] -> I32;
    I64LtU "i64.lt_u" = 0x54: [I64, I64] -> I32;
    I64GtS "i64.gt_s" = 0x55: [I64, I64] -> I32;
    I64GtU "i64.gt_u" = 0x56: [I64, I64] -> I32;
    I64LeS "i64.le_s" = 0x57: [I64, I64] -> I32;
    I64LeU "i64.le_u" = 0x58: [I64, I64] -> I32;
    I64GeS "i64.ge_s" = 0x59: [I64, I64] -> I32;
    I64GeU "i64.ge_u" = 0x5a: [I64, I64] -> I32;
    F32Eq "f32.eq" = 0x5b: [F32, F32] -> I32;
    F32Ne "f32.ne" = 0x5c: [F32, F32] -> I32;
    F32Lt "f32.lt" = 0x5d: [F32, F32] -> I32;
    F32Gt "f32.gt" = 0x5e: [F32, F32] -> I32;
    F32Le "f32.le" = 0x5f: [F32, F32] -> I32;
    F32Ge "f32.ge" = 0x60: [F32, F32] -> I32;
    F64Eq "f64.eq" = 0x61: [F64, F64] -> I32;
    F64Ne "f64.ne" = 0x62: [F64, F64] -> I32;
    F64Lt "f64.lt" = 0x63: [F64, F64] -> I32;
    F64Gt "f64.gt" = 0x64: [F64, F64] -> I32;
    F64Le "f64.le" = 0x65: [F64, F64] -> I32;
    F64Ge "f64.ge" = 0x66: [F64, F64] -> I32;
    I32Add "i32.add" = 0x6a: [I32, I32] -> I32;
    I32Sub "i32.sub" = 0x6b: [I32, I32] -> I32;
    I32Mul "i32.mul" = 0x6c: [I32, I32] -> I32;
    I32DivS "i32.div_s" = 0x6d: [I32, I32] -> I32;
    I32DivU "i32.div_u" = 0x6e: [I32, I32] -> I32;
    I32RemS "i32.rem_s" = 0x6f: [I32, I32] -> I32;
    I32RemU "i32.rem_u" = 0x70: [I32, I32] -> I32;
    I32And "i32.and" = 0x71: [I32, I32] -> I32;
    I32Or "i32.or" = 0x72: [I32, I32] -> I32;
    I32Xor "i32.xor" = 0x73: [I32, I32] -> I32;
    I32Shl "i32.shl" = 0x74: [I32, I32] -> I32;
    I32ShrS "i32.shr_s" = 0x75: [I32, I32] -> I32;
    I32ShrU "i32.shr_u" = 0x76: [I32, I32] -> I32;
    I64Add "i64.add" = 0x7c: [I64, I64] -> I64;
    I64Sub "i64.sub" = 0x7d: [I64, I64] -> I64;
    I64Mul "i64.mul" = 0x7e: [I64, I64] -> I64;
    I64DivS "i64.div_s" = 0x7f: [I64, I64] -> I64;
    I64DivU "i64.div_u" = 0x80: [I64, I64] -> I64;
    I64RemS "i64.rem_s" = 0x81: [I64, I64] -> I64;
    I64RemU "i64.rem_u" = 0x82: [I64, I64] -> I64;
    I64And "i64.and" = 0x83: [I64, I64] -> I64;
    I64Or "i64.or" = 0x84: [I64, I64] -> I64;
    I64Xor "i64.xor" = 0x85: [I64, I64] -> I64;
    I64Shl "i64.shl" = 0x86: [I64, I64] -> I64;
    I64ShrS "i64.shr_s" = 0x87: [I64, I64] -> I64;
    I64ShrU "i64.shr_u" = 0x88: [I64, I64] -> I64;
    F32Add "f32.add" = 0x92: [F32, F32] -> F32;
    F32Sub "f32.sub" = 0x93: [F32, F32] -> F32;
    F32Mul "f32.mul" = 0x94: [F32, F32] -> F32;
    F32Div "f32.div" = 0x95: [F32, F32] -> F32;
    F64Add "f64.add" = 0xa0: [F64, F64] -> F64;
    F64Sub "f64.sub" = 0xa1: [F64, F64] -> F64;
    F64Mul "f64.mul" = 0xa2: [F64, F64] -> F64;
    F64Div "f64.div" = 0xa3: [F64, F64] -> F64;
    I32WrapI64 "i32.wrap_i64" = 0xa7: [I64] -> I32;
    I64ExtendI32S "i64.extend_i32_s" = 0xac: [I32] -> I64;
    I64ExtendI32U "i64.extend_i32_u" = 0xad: [I32] -> I64;
    F32ConvertI32S "f32.convert_i32_s" = 0xb2: [I32] -> F32;
    F32ConvertI32U "f32.convert_i32_u" = 0xb3: [I32] -> F32;
    F32ConvertI64S "f32.convert_i64_s" = 0xb4: [I64] -> F32;
    F32ConvertI64U "f32.convert_i64_u" = 0xb5: [I64] -> F32;
    F32DemoteF64 "f32.demote_f64" = 0xb6: [F64] -> F32;
    F64ConvertI32S "f64.convert_i32_s" = 0xb7: [I32] -> F64;
    F64ConvertI32U "f64.convert_i32_u" = 0xb8: [I32] -> F64;
    F64ConvertI64S "f64.convert_i64_s" = 0xb9: [I64] -> F64;
    F64ConvertI64U "f64.convert_i64_u" = 0xba: [I64] -> F64;
    F64PromoteF32 "f64.promote_f32" = 0xbb: [F32] -> F64;
    I32ReinterpretF32 "i32.reinterpret_f32" = 0xbc: [F32] -> I32;
    I64ReinterpretF64 "i64.reinterpret_f64" = 0xbd: [F64] -> I64;
    F32ReinterpretI32 "f32.reinterpret_i32" = 0xbe: [I32] -> F32;
    F64ReinterpretI64 "f64.reinterpret_i64" = 0xbf: [I64] -> F64;
    I32Extend8S "i32.extend8_s" = 0xc0: [I32] -> I32;
    I32Extend16S "i32.extend16_s" = 0xc1: [I32] -> I32;
    I32TruncSatF32S "i32.trunc_sat_f32_s" = 0x00 / 0xfc: [F32] -> I32;
    I32TruncSatF32U "i32.trunc_sat_f32_u" = 0x01 / 0xfc: [F32] -> I32;
    I32TruncSatF64S "i32.trunc_sat_f64_s" = 0x02 / 0xfc: [F64] -> I32;
    I32TruncSatF64U "i32.trunc_sat_f64_u" = 0x03 / 0xfc: [F64] -> I32;
    I64TruncSatF32S "i64.trunc_sat_f32_s" = 0x04 / 0xfc: [F32] -> I64;
    I64TruncSatF32U "i64.trunc_sat_f32_u" = 0x05 / 0xfc: [F32] -> I64;
    I64TruncSatF64S "i64.trunc_sat_f64_s" = 0x06 / 0xfc: [F64] -> I64;
    I64TruncSatF64U "i64.trunc_sat_f64_u" = 0x07 / 0xfc: [F64] -> I64;
}

/// A load or store: the type on the stack and the bytes in memory, narrower ones read with
/// zeros above.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Mem {
    pub ty: ValType,
    pub bytes: u8,
}

/// A load or a store of a [`Mem`]: its code and its name in the text format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Access {
    pub code: u8,
    pub mnemonic: &'static str,
}

/// Loads and stores by their [`Mem`].
const MEMS: [(Mem, Access, Access); 6] = [
    (Mem { ty: ValType::I32, bytes: 4 }, access(0x28, "i32.load"), access(0x36, "i32.store")),
    (Mem { ty: ValType::I64, bytes: 8 }, access(0x29, "i64.load"), access(0x37, "i64.store")),
    (Mem { ty: ValType::F32, bytes: 4 }, access(0x2a, "f32.load"), access(0x38, "f32.store")),
    (Mem { ty: ValType::F64, bytes: 8 }, access(0x2b, "f64.load"), access(0x39, "f64.store")),
    (Mem { ty: ValType::I32, bytes: 1 }, access(0x2d, "i32.load8_u"), access(0x3a, "i32.store8")),
    (Mem { ty: ValType::I32, bytes: 2 }, access(0x2f, "i32.load16_u"), access(0x3b, "i32.store16")),
];

const fn access(code: u8, mnemonic: &'static str) -> Access {
    Access { code, mnemonic }
}

impl Mem {
    /// Its load and its store, if the backend accesses memory this way.
    #[must_use]
    pub fn accesses(self) -> Option<(Access, Access)> {
        MEMS.iter().find(|(m, ..)| *m == self).map(|&(_, load, store)| (load, store))
    }
}

/// An instruction. Blocks, loops and `if`s take and give nothing; labels are depths.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Op {
    Unreachable,
    Block,
    Loop,
    If,
    Else,
    End,
    Br(u32),
    BrIf(u32),
    BrTable {
        cases: Vec<u32>,
        default: u32,
    },
    Return,
    /// A call of the function a symbol names.
    Call(SymbolId),
    /// A call through the indirect function table of a function of the type.
    CallIndirect(TypeId),
    Drop,
    Select,
    LocalGet(u32),
    LocalSet(u32),
    LocalTee(u32),
    /// The global a symbol names, `__stack_pointer`.
    GlobalGet(SymbolId),
    GlobalSet(SymbolId),
    Load(Mem),
    Store(Mem),
    /// `memory.copy`: the destination, source and length popped.
    MemoryCopy,
    I32Const(i32),
    I64Const(i64),
    F32Const(u32),
    F64Const(u64),
    /// The address of what a symbol names, plus an addend: data's in memory, a function's index
    /// in the indirect function table.
    Address(SymbolId, i32),
    Num(Num),
}
