//! Instructions as a typed set: each declares its encoding and, for the validator, the operand
//! types it pops and the one it pushes, once.

use super::{SymbolId, TypeId, ValType};

/// Numeric instructions: an opcode, the operands popped (the last on top) and the result pushed.
macro_rules! numeric {
    ($($name:ident = $code:literal $(/ $prefix:literal)?: [$($pop:ident),*] -> $push:ident;)*) => {
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
    I32Eqz = 0x45: [I32] -> I32;
    I32Eq = 0x46: [I32, I32] -> I32;
    I32Ne = 0x47: [I32, I32] -> I32;
    I32LtS = 0x48: [I32, I32] -> I32;
    I32LtU = 0x49: [I32, I32] -> I32;
    I32GtS = 0x4a: [I32, I32] -> I32;
    I32GtU = 0x4b: [I32, I32] -> I32;
    I32LeS = 0x4c: [I32, I32] -> I32;
    I32LeU = 0x4d: [I32, I32] -> I32;
    I32GeS = 0x4e: [I32, I32] -> I32;
    I32GeU = 0x4f: [I32, I32] -> I32;
    I64Eq = 0x51: [I64, I64] -> I32;
    I64Ne = 0x52: [I64, I64] -> I32;
    I64LtS = 0x53: [I64, I64] -> I32;
    I64LtU = 0x54: [I64, I64] -> I32;
    I64GtS = 0x55: [I64, I64] -> I32;
    I64GtU = 0x56: [I64, I64] -> I32;
    I64LeS = 0x57: [I64, I64] -> I32;
    I64LeU = 0x58: [I64, I64] -> I32;
    I64GeS = 0x59: [I64, I64] -> I32;
    I64GeU = 0x5a: [I64, I64] -> I32;
    F32Eq = 0x5b: [F32, F32] -> I32;
    F32Ne = 0x5c: [F32, F32] -> I32;
    F32Lt = 0x5d: [F32, F32] -> I32;
    F32Gt = 0x5e: [F32, F32] -> I32;
    F32Le = 0x5f: [F32, F32] -> I32;
    F32Ge = 0x60: [F32, F32] -> I32;
    F64Eq = 0x61: [F64, F64] -> I32;
    F64Ne = 0x62: [F64, F64] -> I32;
    F64Lt = 0x63: [F64, F64] -> I32;
    F64Gt = 0x64: [F64, F64] -> I32;
    F64Le = 0x65: [F64, F64] -> I32;
    F64Ge = 0x66: [F64, F64] -> I32;
    I32Add = 0x6a: [I32, I32] -> I32;
    I32Sub = 0x6b: [I32, I32] -> I32;
    I32Mul = 0x6c: [I32, I32] -> I32;
    I32DivS = 0x6d: [I32, I32] -> I32;
    I32DivU = 0x6e: [I32, I32] -> I32;
    I32RemS = 0x6f: [I32, I32] -> I32;
    I32RemU = 0x70: [I32, I32] -> I32;
    I32And = 0x71: [I32, I32] -> I32;
    I32Or = 0x72: [I32, I32] -> I32;
    I32Xor = 0x73: [I32, I32] -> I32;
    I32Shl = 0x74: [I32, I32] -> I32;
    I32ShrS = 0x75: [I32, I32] -> I32;
    I32ShrU = 0x76: [I32, I32] -> I32;
    I64Add = 0x7c: [I64, I64] -> I64;
    I64Sub = 0x7d: [I64, I64] -> I64;
    I64Mul = 0x7e: [I64, I64] -> I64;
    I64DivS = 0x7f: [I64, I64] -> I64;
    I64DivU = 0x80: [I64, I64] -> I64;
    I64RemS = 0x81: [I64, I64] -> I64;
    I64RemU = 0x82: [I64, I64] -> I64;
    I64And = 0x83: [I64, I64] -> I64;
    I64Or = 0x84: [I64, I64] -> I64;
    I64Xor = 0x85: [I64, I64] -> I64;
    I64Shl = 0x86: [I64, I64] -> I64;
    I64ShrS = 0x87: [I64, I64] -> I64;
    I64ShrU = 0x88: [I64, I64] -> I64;
    F32Add = 0x92: [F32, F32] -> F32;
    F32Sub = 0x93: [F32, F32] -> F32;
    F32Mul = 0x94: [F32, F32] -> F32;
    F32Div = 0x95: [F32, F32] -> F32;
    F64Add = 0xa0: [F64, F64] -> F64;
    F64Sub = 0xa1: [F64, F64] -> F64;
    F64Mul = 0xa2: [F64, F64] -> F64;
    F64Div = 0xa3: [F64, F64] -> F64;
    I32WrapI64 = 0xa7: [I64] -> I32;
    I64ExtendI32S = 0xac: [I32] -> I64;
    I64ExtendI32U = 0xad: [I32] -> I64;
    F32ConvertI32S = 0xb2: [I32] -> F32;
    F32ConvertI32U = 0xb3: [I32] -> F32;
    F32ConvertI64S = 0xb4: [I64] -> F32;
    F32ConvertI64U = 0xb5: [I64] -> F32;
    F32DemoteF64 = 0xb6: [F64] -> F32;
    F64ConvertI32S = 0xb7: [I32] -> F64;
    F64ConvertI32U = 0xb8: [I32] -> F64;
    F64ConvertI64S = 0xb9: [I64] -> F64;
    F64ConvertI64U = 0xba: [I64] -> F64;
    F64PromoteF32 = 0xbb: [F32] -> F64;
    I32ReinterpretF32 = 0xbc: [F32] -> I32;
    I64ReinterpretF64 = 0xbd: [F64] -> I64;
    F32ReinterpretI32 = 0xbe: [I32] -> F32;
    F64ReinterpretI64 = 0xbf: [I64] -> F64;
    I32Extend8S = 0xc0: [I32] -> I32;
    I32Extend16S = 0xc1: [I32] -> I32;
    I32TruncSatF32S = 0x00 / 0xfc: [F32] -> I32;
    I32TruncSatF32U = 0x01 / 0xfc: [F32] -> I32;
    I32TruncSatF64S = 0x02 / 0xfc: [F64] -> I32;
    I32TruncSatF64U = 0x03 / 0xfc: [F64] -> I32;
    I64TruncSatF32S = 0x04 / 0xfc: [F32] -> I64;
    I64TruncSatF32U = 0x05 / 0xfc: [F32] -> I64;
    I64TruncSatF64S = 0x06 / 0xfc: [F64] -> I64;
    I64TruncSatF64U = 0x07 / 0xfc: [F64] -> I64;
}

/// A load or store: the type on the stack and the bytes in memory, narrower ones read with
/// zeros above.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Mem {
    pub ty: ValType,
    pub bytes: u8,
}

/// Loads and stores by their [`Mem`]: the load's code, the store's.
const MEMS: [(Mem, u8, u8); 6] = [
    (Mem { ty: ValType::I32, bytes: 4 }, 0x28, 0x36),
    (Mem { ty: ValType::I64, bytes: 8 }, 0x29, 0x37),
    (Mem { ty: ValType::F32, bytes: 4 }, 0x2a, 0x38),
    (Mem { ty: ValType::F64, bytes: 8 }, 0x2b, 0x39),
    (Mem { ty: ValType::I32, bytes: 1 }, 0x2d, 0x3a),
    (Mem { ty: ValType::I32, bytes: 2 }, 0x2f, 0x3b),
];

impl Mem {
    /// The codes of its load and its store, if the backend accesses memory this way.
    #[must_use]
    pub fn codes(self) -> Option<(u8, u8)> {
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
