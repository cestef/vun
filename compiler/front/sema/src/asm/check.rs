//! Checking a block against its architecture's table: each line's operands read, its mnemonic's
//! forms tried in order, the first that takes the operands chosen, and where none does, why the
//! nearest one does not: an immediate too wide, a register of another class, or no form at all.

use std::collections::BTreeMap;

use vun_diag::{ChildKind, DiagCtxt, DiagId, Diagnostic, Msg, errors};
use vun_hir::tree::{Asm, AsmEntry, ExprId, Inst, Writeback};
use vun_inline::{ClassId, Disp, FormId, Line, Mem, OpId, SizeId};
use vun_intern::Symbol;
use vun_isa::{Blocks, Fit, Fixed, Misfit, Why};
use vun_origin::OriginId;

use super::given::Given;
use super::spell::Mnemonic;
use super::table::{Flow, Table};
use super::{Arch, Block, arm64, bracket, thumb, x86_64};

/// A term of an address: a register, one times a scale, bytes, or a declaration's address.
pub(super) enum Term {
    Reg(Given),
    Scaled(Given, i64),
    Bytes(i64),
    Symbol(ExprId),
}

impl Arch {
    /// How the form `form` of `op` takes `givens`, as the architecture's assembler would.
    fn fit(
        self,
        op: OpId,
        form: FormId,
        fixed: Fixed,
        givens: &[Given],
    ) -> Result<Fit<ExprId, ExprId, ExprId>, Misfit> {
        match self {
            Self::X86_64 => <vun_isa_x86_64::X86_64 as Blocks>::fit(op, form, fixed, givens),
            Self::Arm64 => <vun_isa_arm64::Arm64 as Blocks>::fit(op, form, fixed, givens),
            Self::Thumb => <vun_isa_thumb::Thumb as Blocks>::fit(op, form, fixed, givens),
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
            Self::Arm64 | Self::Thumb => bracket::memory(parts),
        }
    }

    /// The class and width of register a value of `bytes` bytes is held in, a float or not.
    pub(crate) fn register(
        self,
        table: &Table,
        bytes: Option<u64>,
        float: bool,
    ) -> Option<(ClassId, Option<SizeId>)> {
        let width = bytes.and_then(|b| table.width(b));
        match self {
            Self::X86_64 => Some((x86_64::class(table, float)?, width)),
            Self::Arm64 => Some((arm64::class(table, float)?, width)),
            Self::Thumb => Some((thumb::class(table, float)?, width)),
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
        let mnemonic = Mnemonic { word: inst.mnemonic, parts: inst.suffixes.clone() };
        let Some(spellings) = self.table.spellings.get(&mnemonic) else {
            let known = self.table.spellings.keys().filter(|m| m.parts.is_empty()).map(|m| m.word.as_str());
            let near = vun_suggest::closest(inst.mnemonic.as_str(), known);
            let written = mnemonic.to_string();
            let msg = Msg::new().text("the architecture has no instruction ").code(written);
            let mut d = Diagnostic::error(errors::ASM_UNKNOWN_MNEMONIC, msg).at(inst.origin, None);
            if let Some(near) = near {
                d = d.child(ChildKind::Help, Msg::did_you_mean(near));
            }
            self.diag.error(d);
            return None;
        };
        // A post-index access is written as its memory, then the bytes it moves by: `[x1], 8`.
        let (folded, written) = Self::post_indexed(&givens);
        let mut tries: Vec<(&[Given], Option<&[usize]>)> = vec![(&givens, None)];
        if !folded.is_empty() {
            tries.push((&folded, Some(&written)));
        }
        let mut nearest: Option<Misfit> = None;
        for &spelling in spellings {
            for &(givens, written) in &tries {
                let misfit = match self.arch.fit(spelling.op, spelling.form, spelling.fixed, givens) {
                    Ok(fit) => {
                        let (op, form, size, operands) = (spelling.op, spelling.form, fit.size, fit.operands);
                        vun_trace::tracing::trace!(%mnemonic, op = op.0, form = form.0, size = size.0, "fitted");
                        return Some(vun_inline::Inst { op, form, size, operands, origin: inst.origin });
                    }
                    // Named by its place among the operands written.
                    Err(Misfit::Operand { at, why }) => {
                        Misfit::Operand { at: written.map_or(at, |w| w[at]), why }
                    }
                    Err(misfit) => misfit,
                };
                let (op, form, fixed) = (spelling.op.0, spelling.form.0, spelling.fixed);
                vun_trace::tracing::trace!(%mnemonic, op, form, ?fixed, ?misfit, "missed");
                nearest = Some(nearest.map_or(misfit, |n| n.nearer(misfit)));
            }
        }
        self.misfit(inst, nearest.unwrap_or(Misfit::Count));
        None
    }

    /// `givens` with each memory operand followed by bytes taken as a post-index access, and the
    /// place among those written of each; empty when none is.
    fn post_indexed(givens: &[Given]) -> (Vec<Given>, Vec<usize>) {
        let (mut folded, mut written) = (Vec::new(), Vec::new());
        let mut at = 0;
        while at < givens.len() {
            match (&givens[at], givens.get(at + 1)) {
                (
                    Given::Mem {
                        mem: Mem { base, index: None, disp: Disp::Bytes(0) },
                        writeback: vun_isa::Writeback::No,
                    },
                    Some(Given::Imm(Some(bytes))),
                ) => {
                    let mem = Mem { base: base.clone(), index: None, disp: Disp::Bytes(*bytes) };
                    folded.push(Given::Mem { mem, writeback: vun_isa::Writeback::Post });
                    written.push(at);
                    at += 2;
                }
                (given, _) => {
                    folded.push(given.clone());
                    written.push(at);
                    at += 1;
                }
            }
        }
        if folded.len() == givens.len() { (Vec::new(), Vec::new()) } else { (folded, written) }
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

/// The nearer of two misses.
trait Nearer {
    fn nearer(self, other: Self) -> Self;
}

impl Nearer for Misfit {
    /// One at an operand, of the form taking the most operands before it, then the nearest
    /// reason; then one whose width is unknown.
    fn nearer(self, other: Self) -> Self {
        match (self, other) {
            (Self::Operand { at: a, why: wa }, Self::Operand { at: b, why: wb }) => {
                if (b, std::cmp::Reverse(wb)) > (a, std::cmp::Reverse(wa)) { other } else { self }
            }
            (Self::Operand { .. } | Self::Unsized, _) => self,
            (_, Self::Operand { .. } | Self::Unsized) => other,
            _ => self,
        }
    }
}
