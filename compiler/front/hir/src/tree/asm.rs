//! Blocks of an architecture's instructions (spec 07, section 6), as written: sema reads the
//! words and numbers of each line against the architecture's table.

use num_bigint::BigInt;
use vun_grammar::{Flow, TokenKind};
use vun_intern::Symbol;
use vun_origin::OriginId;

use super::ExprId;

/// `arch.asm { entries }`.
pub struct Asm {
    /// The architecture's namespace: `lang.arch.x86_64`.
    pub arch: ExprId,
    pub entries: Vec<AsmEntry>,
}

pub enum AsmEntry {
    /// `'name:`, marking the instruction after it.
    Label {
        name: Symbol,
        origin: OriginId,
    },
    Inst(Inst),
}

/// One line: `mnemonic.suffix operands`.
pub struct Inst {
    pub mnemonic: Symbol,
    /// The condition after a `.`: `b.eq`.
    pub suffix: Option<Symbol>,
    pub operands: Vec<Operand>,
    pub origin: OriginId,
}

pub struct Operand {
    pub kind: OperandKind,
    pub origin: OriginId,
}

/// Whether a memory access writes the address it reached back to its base.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Writeback {
    No,
    /// `[x1, 8]!`.
    Yes,
}

pub enum OperandKind {
    Error,
    /// An expression no architecture takes as an operand, `f(x)`, which sema reports.
    Unfit(ExprId),
    /// A word of the architecture: a register, a condition, a size.
    Word(Symbol),
    Int(BigInt),
    /// `'name`: the instruction a label marks.
    Label(Symbol),
    Prefix {
        op: TokenKind,
        operand: Box<Operand>,
    },
    Infix {
        op: TokenKind,
        left: Box<Operand>,
        right: Box<Operand>,
    },
    /// `[parts]`.
    Memory {
        parts: Vec<Operand>,
        writeback: Writeback,
    },
    /// `{flow(register) value}`: a Vun value bound to the operand. Without a flow, `{name}` is the
    /// address of a declaration.
    Bound {
        flow: Option<Flow>,
        register: Option<Symbol>,
        value: ExprId,
    },
}

impl Operand {
    /// Calls `f` on each Vun expression in this operand, a bound value or an unfit one, in source
    /// order.
    pub fn each_expr(&self, f: &mut dyn FnMut(ExprId)) {
        match &self.kind {
            OperandKind::Bound { value: e, .. } | OperandKind::Unfit(e) => f(*e),
            OperandKind::Prefix { operand, .. } => operand.each_expr(f),
            OperandKind::Infix { left, right, .. } => {
                left.each_expr(f);
                right.each_expr(f);
            }
            OperandKind::Memory { parts, .. } => parts.iter().for_each(|p| p.each_expr(f)),
            OperandKind::Error | OperandKind::Word(_) | OperandKind::Int(_) | OperandKind::Label(_) => {}
        }
    }
}

impl Asm {
    /// Calls `f` on each Vun expression among the block's operands, in source order.
    pub fn each_expr(&self, f: &mut dyn FnMut(ExprId)) {
        for entry in &self.entries {
            if let AsmEntry::Inst(inst) = entry {
                inst.operands.iter().for_each(|o| o.each_expr(f));
            }
        }
    }
}
