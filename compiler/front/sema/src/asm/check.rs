//! Checking a block against its architecture's table: each line's operands read, its mnemonic's
//! forms tried in order, the first that takes the operands chosen, and where none does, why the
//! nearest one does not: an immediate too wide, a register of another class, or no form at all.

use std::collections::BTreeMap;

use vun_diag::{ChildKind, DiagCtxt, DiagId, Diagnostic, Msg, errors};
use vun_hir::tree::{Asm, AsmEntry, ExprId, Inst, Writeback};
use vun_inline::{ClassId, Line, Mem, SizeId};
use vun_intern::Symbol;
use vun_origin::OriginId;

use super::given::Given;
use super::table::{Fixed, Flow, Table};
use super::{Arch, Block, x86_64};
use crate::ty::Value;

/// A form taking a line's operands: the instruction's width, and the operands as it takes them.
pub(super) struct Fit {
    pub(super) size: SizeId,
    pub(super) operands: Vec<vun_inline::Operand<ExprId, ExprId, ExprId>>,
}

/// Why a form does not take a line's operands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Misfit {
    /// It takes another number of operands.
    Count,
    /// No operand gives the instruction's width, and the form has several.
    Unsized,
    /// It is not defined at the width its operands give.
    Width,
    /// The operand at `at` does not fit its position.
    Operand { at: usize, why: Why },
}

/// Why an operand does not fit a position, the nearest miss first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Why {
    /// An immediate the position's bits do not hold.
    Wide,
    /// A register of another class than the position's.
    Class,
    /// An operand of another kind, or width.
    Kind,
}

/// A term of an address: a register, one times a scale, bytes, or a declaration's address.
pub(super) enum Term {
    Reg(Given),
    Scaled(Given, i64),
    Bytes(i64),
    Symbol(ExprId),
}

impl Arch {
    fn fit(self, table: &Table, form: &Value, fixed: Fixed, givens: &[Given]) -> Result<Fit, Misfit> {
        match self {
            Self::X86_64 => x86_64::fit(table, form, fixed, givens),
        }
    }

    /// Memory from the parts between its brackets, if the architecture writes it so.
    pub(super) fn memory(
        self,
        parts: Vec<Vec<Term>>,
        writeback: Writeback,
    ) -> Option<Mem<ExprId, ExprId, ExprId>> {
        match self {
            Self::X86_64 => match (<[_; 1]>::try_from(parts), writeback) {
                (Ok([terms]), Writeback::No) => x86_64::memory(terms),
                _ => None,
            },
        }
    }

    /// The class and width of register a value of `bytes` bytes is held in, a float or not.
    pub(crate) fn register(
        self,
        table: &Table,
        bytes: Option<u64>,
        float: bool,
    ) -> Option<(ClassId, Option<SizeId>)> {
        match self {
            Self::X86_64 => Some((x86_64::class(table, float)?, bytes.and_then(|b| x86_64::width(table, b)))),
        }
    }
}

/// Checks a block, reporting what does not fit its architecture's table.
pub(crate) struct Checker<'c> {
    pub(crate) arch: Arch,
    pub(crate) table: &'c Table,
    /// The class and width of register the value of each binding needs.
    pub(crate) bound: &'c BTreeMap<ExprId, (ClassId, Option<SizeId>)>,
    pub(crate) diag: &'c mut DiagCtxt,
}

impl Checker<'_> {
    /// The block checked, or `None` when a line does not fit, which is reported.
    pub(crate) fn block(&mut self, asm: &Asm) -> Option<Block> {
        let mut labels = BTreeMap::new();
        let mut line = 0_u16;
        for entry in &asm.entries {
            match entry {
                AsmEntry::Label { name, .. } => {
                    labels.insert(*name, Line(line));
                }
                AsmEntry::Inst(_) => line += 1,
            }
        }
        let mut insts = Vec::new();
        let mut fits = true;
        for entry in &asm.entries {
            let AsmEntry::Inst(inst) = entry else { continue };
            match self.inst(inst, &labels) {
                Some(inst) => insts.push(inst),
                None => fits = false,
            }
        }
        let returns = self.returns(&insts);
        fits.then(|| Block { arch: self.arch.namespace(), insts, returns })
    }

    fn inst(
        &mut self,
        inst: &Inst,
        labels: &BTreeMap<Symbol, Line>,
    ) -> Option<vun_inline::Inst<ExprId, ExprId, ExprId>> {
        let givens: Vec<_> = inst.operands.iter().map(|o| self.given(o, labels)).collect();
        let givens = givens.into_iter().collect::<Option<Vec<_>>>()?;
        let Some(spellings) = self.table.spellings.get(&(inst.mnemonic, inst.suffix)) else {
            let known = self.table.spellings.keys().filter(|(_, s)| s.is_none()).map(|(m, _)| m.as_str());
            let near = vun_suggest::closest(inst.mnemonic.as_str(), known);
            let msg = Msg::new().text("the architecture has no instruction ").code(inst.mnemonic.as_str());
            let mut d = Diagnostic::error(errors::ASM_UNKNOWN_MNEMONIC, msg).at(inst.origin, None);
            if let Some(near) = near {
                d = d.child(ChildKind::Help, Msg::did_you_mean(near));
            }
            self.diag.error(d);
            return None;
        };
        let mut nearest: Option<Misfit> = None;
        for &spelling in spellings {
            match self.arch.fit(self.table, self.table.form(spelling), spelling.fixed, &givens) {
                Ok(fit) => {
                    let (op, form, size, operands) = (spelling.op, spelling.form, fit.size, fit.operands);
                    return Some(vun_inline::Inst { op, form, size, operands, origin: inst.origin });
                }
                Err(misfit) => nearest = Some(nearest.map_or(misfit, |n| n.nearer(misfit))),
            }
        }
        self.misfit(inst, nearest.unwrap_or(Misfit::Count));
        None
    }

    /// Reports why the nearest form does not take the operands of `inst`.
    fn misfit(&mut self, inst: &Inst, misfit: Misfit) {
        let mnemonic = inst.mnemonic.as_str();
        let (id, origin, msg): (DiagId, OriginId, Msg) = match misfit {
            Misfit::Operand { at, why: Why::Wide } => {
                let msg = Msg::new().text("the integer does not fit any immediate of ").code(mnemonic);
                (errors::ASM_IMMEDIATE_TOO_WIDE, inst.operands[at].origin, msg)
            }
            Misfit::Operand { at, why: Why::Class } => {
                let msg = Msg::new()
                    .text("the register is of another class than ")
                    .code(mnemonic)
                    .text(" takes here");
                (errors::ASM_REGISTER_CLASS, inst.operands[at].origin, msg)
            }
            Misfit::Unsized => {
                let msg = Msg::new().text("no operand gives the width of ").code(mnemonic);
                (errors::ASM_NO_FORM, inst.origin, msg)
            }
            Misfit::Count | Misfit::Width | Misfit::Operand { .. } => {
                let msg = Msg::new().text("no form of ").code(mnemonic).text(" takes these operands");
                (errors::ASM_NO_FORM, inst.origin, msg)
            }
        };
        self.diag.error(Diagnostic::error(id, msg).at(origin, None));
    }

    /// Whether the end of the block is reached: past a line that goes on, or by a jump to a label
    /// at the end; not past one that ends or only jumps.
    fn returns(&self, insts: &[vun_inline::Inst<ExprId, ExprId, ExprId>]) -> bool {
        let end = insts.len();
        let mut reached = vec![false; end + 1];
        let mut stack = vec![0];
        while let Some(at) = stack.pop() {
            if std::mem::replace(&mut reached[at], true) {
                continue;
            }
            let Some(inst) = insts.get(at) else { continue };
            let form = &self.table.ops[usize::from(inst.op.0)].forms[usize::from(inst.form.0)];
            let flow = Table::flow(self.arch, form);
            for operand in &inst.operands {
                if let vun_inline::Operand::Label(Line(line)) = operand {
                    stack.push(usize::from(*line));
                }
            }
            if flow == Flow::Next {
                stack.push(at + 1);
            }
        }
        reached[end]
    }
}

impl Misfit {
    /// The nearer miss of two: one at an operand, the nearest reason first.
    fn nearer(self, other: Self) -> Self {
        match (self, other) {
            (Self::Operand { why: a, .. }, Self::Operand { why: b, .. }) if b < a => other,
            (Self::Operand { .. }, _) => self,
            (_, Self::Operand { why: Why::Wide | Why::Class, .. }) | (Self::Count, Self::Unsized) => other,
            (Self::Unsized, _) => self,
            (_, Self::Operand { .. }) => other,
            _ => self,
        }
    }
}
