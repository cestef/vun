//! Which wasm instruction computes each LIR operation over each value type: tables, looked up.

use vun_lir::{BinOp, CastOp, Cond};

use crate::module::ValType::{self, F32, F64, I32, I64};
use crate::module::op::Num;

const BINARY: [(BinOp, ValType, Num); 34] = [
    (BinOp::Add, I32, Num::I32Add),
    (BinOp::Sub, I32, Num::I32Sub),
    (BinOp::Mul, I32, Num::I32Mul),
    (BinOp::SDiv, I32, Num::I32DivS),
    (BinOp::UDiv, I32, Num::I32DivU),
    (BinOp::SRem, I32, Num::I32RemS),
    (BinOp::URem, I32, Num::I32RemU),
    (BinOp::And, I32, Num::I32And),
    (BinOp::Or, I32, Num::I32Or),
    (BinOp::Xor, I32, Num::I32Xor),
    (BinOp::Shl, I32, Num::I32Shl),
    (BinOp::LShr, I32, Num::I32ShrU),
    (BinOp::AShr, I32, Num::I32ShrS),
    (BinOp::Add, I64, Num::I64Add),
    (BinOp::Sub, I64, Num::I64Sub),
    (BinOp::Mul, I64, Num::I64Mul),
    (BinOp::SDiv, I64, Num::I64DivS),
    (BinOp::UDiv, I64, Num::I64DivU),
    (BinOp::SRem, I64, Num::I64RemS),
    (BinOp::URem, I64, Num::I64RemU),
    (BinOp::And, I64, Num::I64And),
    (BinOp::Or, I64, Num::I64Or),
    (BinOp::Xor, I64, Num::I64Xor),
    (BinOp::Shl, I64, Num::I64Shl),
    (BinOp::LShr, I64, Num::I64ShrU),
    (BinOp::AShr, I64, Num::I64ShrS),
    (BinOp::FAdd, F32, Num::F32Add),
    (BinOp::FSub, F32, Num::F32Sub),
    (BinOp::FMul, F32, Num::F32Mul),
    (BinOp::FDiv, F32, Num::F32Div),
    (BinOp::FAdd, F64, Num::F64Add),
    (BinOp::FSub, F64, Num::F64Sub),
    (BinOp::FMul, F64, Num::F64Mul),
    (BinOp::FDiv, F64, Num::F64Div),
];

const COMPARE: [(Cond, ValType, Num); 30] = [
    (Cond::Eq, I32, Num::I32Eq),
    (Cond::Ne, I32, Num::I32Ne),
    (Cond::SLt, I32, Num::I32LtS),
    (Cond::SLe, I32, Num::I32LeS),
    (Cond::SGt, I32, Num::I32GtS),
    (Cond::SGe, I32, Num::I32GeS),
    (Cond::ULt, I32, Num::I32LtU),
    (Cond::ULe, I32, Num::I32LeU),
    (Cond::UGt, I32, Num::I32GtU),
    (Cond::UGe, I32, Num::I32GeU),
    (Cond::Eq, I64, Num::I64Eq),
    (Cond::Ne, I64, Num::I64Ne),
    (Cond::SLt, I64, Num::I64LtS),
    (Cond::SLe, I64, Num::I64LeS),
    (Cond::SGt, I64, Num::I64GtS),
    (Cond::SGe, I64, Num::I64GeS),
    (Cond::ULt, I64, Num::I64LtU),
    (Cond::ULe, I64, Num::I64LeU),
    (Cond::UGt, I64, Num::I64GtU),
    (Cond::UGe, I64, Num::I64GeU),
    (Cond::FEq, F32, Num::F32Eq),
    (Cond::FLt, F32, Num::F32Lt),
    (Cond::FLe, F32, Num::F32Le),
    (Cond::FGt, F32, Num::F32Gt),
    (Cond::FGe, F32, Num::F32Ge),
    (Cond::FEq, F64, Num::F64Eq),
    (Cond::FLt, F64, Num::F64Lt),
    (Cond::FLe, F64, Num::F64Le),
    (Cond::FGt, F64, Num::F64Gt),
    (Cond::FGe, F64, Num::F64Ge),
];

const CAST: [(CastOp, ValType, ValType, Num); 25] = [
    (CastOp::ZExt, I32, I64, Num::I64ExtendI32U),
    (CastOp::SExt, I32, I64, Num::I64ExtendI32S),
    (CastOp::Trunc, I64, I32, Num::I32WrapI64),
    (CastOp::FConvert, F32, F64, Num::F64PromoteF32),
    (CastOp::FConvert, F64, F32, Num::F32DemoteF64),
    (CastOp::SToF, I32, F32, Num::F32ConvertI32S),
    (CastOp::SToF, I32, F64, Num::F64ConvertI32S),
    (CastOp::SToF, I64, F32, Num::F32ConvertI64S),
    (CastOp::SToF, I64, F64, Num::F64ConvertI64S),
    (CastOp::UToF, I32, F32, Num::F32ConvertI32U),
    (CastOp::UToF, I32, F64, Num::F64ConvertI32U),
    (CastOp::UToF, I64, F32, Num::F32ConvertI64U),
    (CastOp::UToF, I64, F64, Num::F64ConvertI64U),
    (CastOp::FToS, F32, I32, Num::I32TruncSatF32S),
    (CastOp::FToS, F64, I32, Num::I32TruncSatF64S),
    (CastOp::FToS, F32, I64, Num::I64TruncSatF32S),
    (CastOp::FToS, F64, I64, Num::I64TruncSatF64S),
    (CastOp::FToU, F32, I32, Num::I32TruncSatF32U),
    (CastOp::FToU, F64, I32, Num::I32TruncSatF64U),
    (CastOp::FToU, F32, I64, Num::I64TruncSatF32U),
    (CastOp::FToU, F64, I64, Num::I64TruncSatF64U),
    (CastOp::Bitcast, F32, I32, Num::I32ReinterpretF32),
    (CastOp::Bitcast, F64, I64, Num::I64ReinterpretF64),
    (CastOp::Bitcast, I32, F32, Num::F32ReinterpretI32),
    (CastOp::Bitcast, I64, F64, Num::F64ReinterpretI64),
];

/// The instruction computing `op` over values of type `ty`, which verified LIR has.
pub(super) fn binary(op: BinOp, ty: ValType) -> Num {
    BINARY.iter().find(|r| r.0 == op && r.1 == ty).map(|r| r.2).expect("verified LIR computes it")
}

/// The instruction comparing values of type `ty` by `cond`, but `FNe`, which no one does.
pub(super) fn compare(cond: Cond, ty: ValType) -> Num {
    COMPARE.iter().find(|r| r.0 == cond && r.1 == ty).map(|r| r.2).expect("verified LIR compares so")
}

/// The instruction converting a value of type `from` to one of type `to` by `op`.
pub(super) fn cast(op: CastOp, from: ValType, to: ValType) -> Num {
    CAST.iter()
        .find(|r| r.0 == op && r.1 == from && r.2 == to)
        .map(|r| r.3)
        .expect("verified LIR converts so")
}
