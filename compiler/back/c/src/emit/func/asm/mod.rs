//! Inline assembly as GNU C writes it (spec 07, section 6): each binding a register variable in
//! the register it names or its position takes, else in the first argument register of the
//! target's C convention the block leaves free; the text is the assembler's own listing of each
//! instruction over those registers, its lines labelled uniquely to the statement by `%=`. The
//! registers the block names or its forms use unnamed are clobbered, with the flags and memory.
//! Symbols are refused: their names in assembly are the object format's, not C's. Where the
//! program checks its blocks (spec 13, section 8.7), the statement also reads the stack pointer
//! into two more registers, first and last, and a block that returns with it moved ends the
//! program through `check.stack`; the addresses each instruction writes are the native backend's
//! to check, which encodes them.

mod gnu;

use vun_asm::AsmOperand;
use vun_asm::Blocks;
use vun_asm_arm64::Arm64;
use vun_asm_x86_64::X86_64;
use vun_backend::{Lack, Missing};
use vun_inline::{Bound, Line};
use vun_intern::Symbol;
use vun_isa_thumb::Thumb;
use vun_lir::{Address, AsmCheck, Scalar, Value};
use vun_origin::OriginId;

use super::FnEmit;
use crate::ast::{Asm, Binary, Binding, Decl, Expr, Flow, Ident, Stmt};
use gnu::{Block, Given, Gnu, Labelled, Unlinked};

/// The registers a block writes beyond those it names: the flags, and memory.
const CLOBBERED: [&str; 2] = ["cc", "memory"];

impl FnEmit<'_, '_> {
    /// `asm` as a GNU C statement, if the target's code is in the block's instruction set.
    pub(super) fn asm(
        &mut self,
        asm: &vun_lir::Asm,
        check: Option<&AsmCheck>,
        at: OriginId,
    ) -> Result<(), Lack> {
        if Symbol::intern(asm.arch.name()) != self.e.target.isa {
            return Err(Missing::Asm.at(at));
        }
        let block = asm.clone().rebind(&mut Unlinked).map_err(|o| Missing::AsmOperand(o).at(at))?;
        match asm.arch {
            X86_64::ARCH => self.inline::<X86_64>(&block, check),
            Arm64::ARCH => self.inline::<Arm64>(&block, check),
            Thumb::ARCH => self.inline::<Thumb>(&block, check),
            _ => Err(Missing::Asm.at(at)),
        }
    }

    fn inline<I: Gnu>(&mut self, block: &Block, check: Option<&AsmCheck>) -> Result<(), Lack> {
        for inst in &block.insts {
            for (kind, operand) in I::kinds(inst).zip(&inst.operands) {
                if let vun_inline::Operand::Mem(mem) = operand {
                    I::addressable(mem, kind).map_err(|o| Missing::AsmOperand(o).at(inst.origin))?;
                }
            }
        }
        let uses: Vec<_> = I::uses(block).collect();
        let mut fixed = Vec::with_capacity(uses.len());
        for used in &uses {
            fixed.push(used.fixed().map_err(|o| Missing::AsmOperand(o).at(used.inst.origin))?);
        }
        let named: Vec<I::Reg> = I::named(block).collect();
        let taken: Vec<I::Reg> = named.iter().copied().chain(fixed.iter().flatten().copied()).collect();
        let mut free = I::free(self.e.target.convention).iter().copied().filter(|r| !taken.contains(r));
        let mut regs = Vec::with_capacity(uses.len());
        for (used, fixed) in uses.iter().zip(&fixed) {
            let reg = fixed.or_else(|| free.next());
            regs.push(reg.ok_or_else(|| Missing::AsmOperand(AsmOperand::Crowded).at(used.inst.origin))?);
        }
        let origin = block.insts[0].origin;
        let crowded = || Missing::AsmOperand(AsmOperand::Crowded).at(origin);
        let stack = match check {
            Some(_) => Some([free.next().ok_or_else(crowded)?, free.next().ok_or_else(crowded)?]),
            None => None,
        };
        // A register holds an address: the bindings' variables are that wide.
        let bits = self.e.unit.pointer_bits;
        let word = self.e.scalar(Scalar::int(bits).expect("addresses of an integer's width"), origin)?;
        let (mut bindings, mut after) = (Vec::new(), Vec::new());
        for (k, (used, &reg)) in uses.iter().zip(&regs).enumerate() {
            let name = Ident::bound(k);
            let flow = match used.bound {
                Bound::In { value, .. } => Flow::In(self.operand(*value).cast(word.clone())),
                Bound::Out { place, .. } | Bound::InOut { place, .. } => {
                    let ty = self.e.scalar(place.ty, used.inst.origin)?;
                    let at = self.named(place.addr).at(ty.clone());
                    if block.returns {
                        after.push(Stmt::Assign(at.clone(), Expr::Name(name).cast(ty)));
                    }
                    match used.bound {
                        Bound::InOut { .. } => Flow::InOut(at.cast(word.clone())),
                        _ => Flow::Out,
                    }
                }
            };
            bindings.push(Binding { decl: Decl::new(word.clone(), name), reg: I::name(reg), flow });
        }
        let mut clobbered: Vec<I::Reg> = Vec::new();
        for r in named {
            if !regs.contains(&r) && !clobbered.contains(&r) {
                clobbered.push(r);
            }
        }
        let clobbers = clobbered.into_iter().map(I::name).chain(CLOBBERED.map(str::to_owned)).collect();
        if let (Some(check), Some([first, last])) = (check, stack) {
            let (b, a) = (Ident::bound(bindings.len()), Ident::bound(bindings.len() + 1));
            for (name, reg) in [(b, first), (a, last)] {
                bindings.push(Binding {
                    decl: Decl::new(word.clone(), name),
                    reg: I::name(reg),
                    flow: Flow::Out,
                });
            }
            if block.returns {
                let site = Expr::small(u64::from(check.site)).cast(self.e.scalar(Scalar::I32, origin)?);
                let call =
                    Expr::Call(Box::new(self.callee(check.stack)), vec![site, Expr::Name(b), Expr::Name(a)]);
                let moved = Expr::binary(Binary::Ne, Expr::Name(b), Expr::Name(a));
                after.push(Stmt::If(moved, vec![Stmt::Expr(call)], Vec::new()));
            }
        }
        let lines = Self::lines::<I>(block, regs, stack);
        self.body.push(Stmt::Asm(Box::new(Asm { bindings, lines, clobbers, after })));
        Ok(())
    }

    /// The block's text: its instructions' over `regs`, each line some instruction jumps to
    /// labelled, between the lines setting the assembler's syntax.
    fn lines<I: Gnu>(block: &Block, regs: Vec<I::Reg>, stack: Option<[I::Reg; 2]>) -> Vec<String> {
        let jumped = |line: usize| {
            let mut targets = block.insts.iter().flat_map(|inst| &inst.operands);
            targets.any(|o| matches!(o, vun_inline::Operand::Label(l) if usize::from(l.0) == line))
        };
        let label = |line: usize| {
            let line = Line(u16::try_from(line).expect("blocks of at most 65536 lines"));
            format!("{}:", Labelled(line))
        };
        let mut given = Given::<I> { regs: regs.into_iter() };
        let mut lines: Vec<String> = I::ENTER.iter().map(|&l| l.to_owned()).collect();
        lines.extend(stack.map(|[before, _]| I::read_stack(before)));
        for (k, inst) in block.insts.iter().enumerate() {
            if jumped(k) {
                lines.push(label(k));
            }
            let operands: Vec<_> =
                I::kinds(inst).zip(&inst.operands).map(|(kind, o)| I::operand(o, kind, &mut given)).collect();
            lines.push(I::text(inst, &operands));
        }
        if jumped(block.insts.len()) {
            lines.push(label(block.insts.len()));
        }
        lines.extend(stack.map(|[_, after]| I::read_stack(after)));
        lines.extend(I::LEAVE.iter().map(|&l| l.to_owned()));
        lines
    }

    /// The function `address` names, to call.
    fn callee(&self, address: Address) -> Expr {
        match address {
            Address::Func(f) => Expr::Name(self.e.func(f)),
            Address::Import(i) => Expr::Name(Ident::Symbol(self.e.unit.imports[i].symbol)),
            Address::Data(_) => unreachable!("the verifier checks a check's helpers are functions"),
        }
    }

    /// The expression naming `v` wherever it is read: its local, its inlined expression assigned
    /// there first.
    fn named(&mut self, v: Value) -> Expr {
        if let Some(expr) = self.exprs[v].take() {
            self.assign(v, expr);
        }
        Expr::Name(Ident::value(v))
    }
}
