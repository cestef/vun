//! Checking a block against its architecture's table: where each label marks, then each
//! instruction called (`inst`), and whether the end of the block is reached.

use std::collections::BTreeMap;

use vun_diag::{DiagCtxt, Diagnostic, Msg, errors};
use vun_hir::tree::{BlockId, ExprId, PatKind, Stmt};
use vun_inline::{ClassId, Line, Mem, SizeId};
use vun_isa::{Blocks, Fit, Fixed, Misfit};
use vun_primitives::arch::Arch as Form;

use super::read::Labels;
use super::table::{Flow, Table};
use super::{Arch, Block, arm64, bracket, thumb, wasm32, x86_64};
use crate::env::Env;

/// A part of an address: a register, one times a scale, bytes, or a declaration's address.
pub(super) enum Term {
    Reg(super::given::Given),
    Scaled(super::given::Given, i64),
    Bytes(i64),
    Symbol(ExprId),
}

impl Arch {
    /// How the form `form` of `op` takes `givens`, as the architecture's assembler would.
    pub(super) fn fit(
        self,
        op: vun_inline::OpId,
        form: vun_inline::FormId,
        fixed: Fixed,
        givens: &[super::given::Given],
    ) -> Result<Fit<ExprId, ExprId, ExprId>, Misfit> {
        match self {
            Self::X86_64 => <vun_isa_x86_64::X86_64 as Blocks>::fit(op, form, fixed, givens),
            Self::Arm64 => <vun_isa_arm64::Arm64 as Blocks>::fit(op, form, fixed, givens),
            Self::Thumb => <vun_isa_thumb::Thumb as Blocks>::fit(op, form, fixed, givens),
            Self::Wasm32 => <vun_isa_wasm32::Wasm32 as Blocks>::fit(op, form, fixed, givens),
        }
    }

    /// Memory from its parts, if the architecture writes one so.
    pub(super) fn memory(self, parts: Vec<Term>) -> Option<Mem<ExprId, ExprId, ExprId>> {
        match self {
            Self::X86_64 => x86_64::memory(parts),
            Self::Arm64 | Self::Thumb => bracket::memory(parts),
            Self::Wasm32 => None,
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
            Self::Wasm32 => Some((wasm32::class(table, bytes, float)?, width)),
        }
    }
}

/// Checks a block, reporting what does not fit its architecture's table.
pub(crate) struct Checker<'c, 'a, 'db> {
    pub(crate) env: &'c Env<'a, 'db>,
    pub(crate) arch: Arch,
    pub(crate) table: &'c Table,
    /// The operand form of `lang.arch` each callee of the block calls, as it was typed.
    pub(crate) forms: &'c BTreeMap<ExprId, Form>,
    /// The class and width of register the value of each binding needs.
    pub(crate) bound: &'c BTreeMap<ExprId, (ClassId, Option<SizeId>)>,
    /// Whether the block is of the target's instruction set, so that the widths of values are
    /// known.
    pub(crate) targeted: bool,
    pub(crate) diag: &'c mut DiagCtxt,
}

/// A statement of a block: an instruction called, a label made or marked, or a spread.
pub(super) enum Entry {
    Inst(ExprId),
    Marks(vun_hir::tree::BinderId),
    Makes,
    Spread,
}

impl Checker<'_, '_, '_> {
    /// The block `block` checked, or `None` when an instruction does not fit, which is reported.
    pub(crate) fn block(&mut self, block: BlockId) -> Option<Block> {
        let entries = self.entries(block)?;
        let mut labels = Labels::new();
        let mut line = 0_u16;
        for entry in &entries {
            match entry {
                Entry::Inst(_) => line = line.checked_add(1)?,
                Entry::Marks(label) => {
                    labels.insert(*label, Line(line));
                }
                Entry::Makes | Entry::Spread => {}
            }
        }
        let (mut insts, mut stacks) = (Vec::new(), Vec::new());
        let mut fits = true;
        for entry in &entries {
            let Entry::Inst(call) = entry else { continue };
            match self.inst(*call, &labels) {
                Some((inst, stack)) => {
                    insts.push(inst);
                    stacks.push(stack);
                }
                None => fits = false,
            }
        }
        if fits {
            fits = self.stack(self.env.parsed.hir.blocks[block].origin, &insts, &stacks);
        }
        let returns = self.returns(&insts);
        let needs = self.table.needs(self.arch, &insts);
        fits.then(|| Block { arch: self.arch.namespace(), insts, returns, needs })
    }

    /// What each statement of `block` is, its value last; `None` where one is none of them,
    /// which is reported.
    fn entries(&mut self, block: BlockId) -> Option<Vec<Entry>> {
        let hir = &self.env.parsed.hir;
        let b = &hir.blocks[block];
        let written = b.stmts.iter().filter_map(|s| match s {
            vun_hir::tree::Entry::One(s) => Some(s),
            vun_hir::tree::Entry::Staged(_) => None,
        });
        let mut entries = Vec::new();
        let mut read = true;
        for stmt in written {
            let entry = match stmt {
                Stmt::Spread(_) => Some(Entry::Spread),
                Stmt::Expr(e) => self.entry(*e),
                Stmt::Decl(d) => {
                    let decl = &hir.decls[*d];
                    let made =
                        decl.value.and_then(|v| self.args(v)).and_then(|(callee, _)| self.form(callee));
                    match (made, &hir.pats[decl.pat].kind) {
                        (Some(Form::here), PatKind::Bind(label)) => Some(Entry::Marks(*label)),
                        (Some(Form::label), PatKind::Bind(_)) => Some(Entry::Makes),
                        _ => None,
                    }
                }
                _ => None,
            };
            if let Some(entry) = entry {
                entries.push(entry);
            } else {
                read = false;
                let origin = match stmt {
                    Stmt::Decl(d) => hir.decls[*d].origin,
                    Stmt::Expr(e) => hir.exprs[*e].origin,
                    Stmt::Assign { origin, .. } => *origin,
                    Stmt::Block(b) => hir.blocks[*b].origin,
                    Stmt::Spread(ns) => hir.namespaces[*ns].origin,
                };
                let msg = Msg::new().text("a block of instructions holds calls of instructions and labels");
                self.diag.error(Diagnostic::error(errors::ASM_UNKNOWN_MNEMONIC, msg).at(origin, None));
            }
        }
        if let Some(tail) = b.tail {
            entries.push(self.entry(tail)?);
        }
        read.then_some(entries)
    }

    /// The statement `expr`: a label marked (`place(end)`), or an instruction called.
    fn entry(&self, expr: ExprId) -> Option<Entry> {
        let (callee, args) = self.args(expr)?;
        match (self.form(callee), args.as_slice()) {
            (Some(Form::place), [label]) => match self.env.parsed.resolution.names[*label] {
                Some(vun_hir::resolve::Res::Binder(b)) => Some(Entry::Marks(b)),
                _ => None,
            },
            (Some(_), _) => None,
            (None, _) => Some(Entry::Inst(expr)),
        }
    }

    /// Whether the end of the block is reached: past an instruction that goes on, or by a jump to
    /// a label at the end; not past one that ends or only jumps.
    fn returns(&self, insts: &[vun_inline::Inst<ExprId, ExprId, ExprId>]) -> bool {
        let end = insts.len();
        let mut reached = vec![false; end + 1];
        let mut stack = vec![0];
        while let Some(at) = stack.pop() {
            if std::mem::replace(&mut reached[at], true) {
                continue;
            }
            let Some(inst) = insts.get(at) else { continue };
            let flow = self.table.form(inst.op, inst.form).map_or(Flow::Next, |f| Table::flow(self.arch, f));
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
