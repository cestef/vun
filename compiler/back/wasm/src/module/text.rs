//! Instructions in the text format, as listings show them: symbols by name, blocks indented.

use std::fmt;

use super::Module;
use super::op::{Op, Operand};

/// An instruction of `module`, shown in the text format.
pub struct Text<'m> {
    pub module: &'m Module,
    pub op: &'m Op,
}

impl fmt::Display for Text<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = |sym: super::SymbolId| self.module.symbols[sym].name;
        match self.op {
            Op::Unreachable => f.write_str("unreachable"),
            Op::Block => f.write_str("block"),
            Op::Loop => f.write_str("loop"),
            Op::If => f.write_str("if"),
            Op::Else => f.write_str("else"),
            Op::End => f.write_str("end"),
            Op::Br(l) => write!(f, "br {l}"),
            Op::BrIf(l) => write!(f, "br_if {l}"),
            Op::BrTable { cases, default } => {
                f.write_str("br_table")?;
                for l in cases.iter().chain([default]) {
                    write!(f, " {l}")?;
                }
                Ok(())
            }
            Op::Return => f.write_str("return"),
            Op::Call(sym) => write!(f, "call ${}", name(*sym)),
            Op::CallIndirect(ty) => write!(f, "call_indirect (type {ty:?})"),
            Op::Drop => f.write_str("drop"),
            Op::Select => f.write_str("select"),
            Op::LocalGet(l) => write!(f, "local.get {l}"),
            Op::LocalSet(l) => write!(f, "local.set {l}"),
            Op::LocalTee(l) => write!(f, "local.tee {l}"),
            Op::GlobalGet(sym) => write!(f, "global.get ${}", name(*sym)),
            Op::GlobalSet(sym) => write!(f, "global.set ${}", name(*sym)),
            Op::Load(mem) | Op::Store(mem) => {
                let (load, store) = mem.accesses().ok_or(fmt::Error)?;
                f.write_str(if matches!(self.op, Op::Load(_)) { load.mnemonic } else { store.mnemonic })
            }
            Op::MemoryCopy => f.write_str("memory.copy"),
            Op::I32Const(v) => write!(f, "i32.const {v}"),
            Op::I64Const(v) => write!(f, "i64.const {v}"),
            Op::F32Const(bits) => write!(f, "f32.const {}", f32::from_bits(*bits)),
            Op::F64Const(bits) => write!(f, "f64.const {}", f64::from_bits(*bits)),
            Op::Address(sym, 0) => write!(f, "i32.const &${}", name(*sym)),
            Op::Address(sym, addend) => write!(f, "i32.const &${}{addend:+}", name(*sym)),
            Op::Num(n) => f.write_str(n.mnemonic()),
            Op::Asm(asm) => {
                // The text format spells the first underscore of a table's name as a dot.
                f.write_str(&vun_isa_wasm32::Op::entry(asm.op, asm.form).0.name().replacen('_', ".", 1))?;
                for operand in &asm.operands {
                    match operand {
                        Operand::Local(l) => write!(f, " {l}")?,
                        Operand::Imm(v) => write!(f, " {v}")?,
                    }
                }
                Ok(())
            }
        }
    }
}
