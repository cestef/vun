//! Structured control from a LIR function's blocks (Ramsey, "Beyond Relooper", 2022): each block
//! is emitted where its immediate dominator places it. A block that more than one forward edge
//! reaches is the code after a `block` wrapping its dominator's, a loop head is a `loop`, and a
//! jump is a `br` to the label of its target or, to a block only it reaches, the target's code in
//! place. A jump's arguments are pushed, then popped into the target's parameters in reverse:
//! a parallel copy with no temporary.
//!
//! A function entering a loop other than through its head (a coroutine's resumption) has no
//! such structure: it runs as one `loop` around a `br_table` on a local naming the next block,
//! each jump setting it and branching back.

use vun_backend::Lack;
use vun_index::IndexVec;
use vun_lir::analysis::{Cfg, Dominators, Loops};
use vun_lir::{Block, Target, Terminator};

use super::func::Lower;
use crate::module::op::Op;

/// What a label around the code being emitted leads to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Label {
    /// The end of a `block`, after which a block's code follows.
    Follow(Block),
    /// The head of a `loop`, a block's code.
    Head(Block),
    /// A label no jump names: an `if`, or where a switch lands.
    Other,
    /// The loop dispatching on the next block.
    Dispatch,
}

/// The shape of a function's control flow.
pub(super) struct Shape {
    /// Each block's position in reverse postorder.
    rpo: IndexVec<Block, usize>,
    /// The blocks each block immediately dominates, in reverse postorder.
    children: IndexVec<Block, Vec<Block>>,
    /// Whether more than one forward edge reaches a block.
    merge: IndexVec<Block, bool>,
    /// Whether a back edge reaches a block.
    head: IndexVec<Block, bool>,
    /// For a function whose control is irreducible, the dispatch it runs as.
    dispatch: Option<Dispatch>,
}

/// A function run as a loop dispatching on the next block: the local naming it, and the blocks
/// reached in reverse postorder, each named by its place there.
struct Dispatch {
    next: u32,
    blocks: Vec<Block>,
}

impl Shape {
    /// The shape of `lower`'s function.
    pub(super) fn of(lower: &mut Lower<'_>) -> Self {
        let f = lower.f;
        let cfg = Cfg::of(f);
        let loops = Loops::of(f, &cfg);
        let dominators = Dominators::of(f, &cfg);
        let count = f.blocks.len();
        let mut rpo = IndexVec::filled(usize::MAX, count);
        for (i, &b) in dominators.rpo().iter().enumerate() {
            rpo[b] = i;
        }
        let irreducible = dominators.rpo().iter().any(|&b| loops.is_header(b) && loops.is_irreducible(b));
        let dispatch = irreducible.then(|| Dispatch {
            next: lower.temp(crate::module::ValType::I32),
            blocks: dominators.rpo().to_vec(),
        });
        let mut merge = IndexVec::filled(false, count);
        let mut forward: IndexVec<Block, u32> = IndexVec::filled(0, count);
        let mut head = IndexVec::filled(false, count);
        for &b in dominators.rpo() {
            for t in f.blocks[b].term.targets() {
                if rpo[t.block] > rpo[b] {
                    forward[t.block] += 1;
                    merge[t.block] = forward[t.block] > 1;
                } else {
                    head[t.block] = true;
                }
            }
        }
        Self { rpo, children: dominators.children(), merge, head, dispatch }
    }
}

impl Shape {
    /// Whether the function runs as a loop dispatching on the next block.
    pub(super) fn dispatches(&self) -> bool {
        self.dispatch.is_some()
    }
}

/// Emits `lower`'s body in the shape `shape`.
pub(super) struct Emit<'s, 'l, 'a> {
    pub(super) lower: &'l mut Lower<'a>,
    pub(super) shape: &'s Shape,
    labels: Vec<Label>,
}

impl<'s, 'l, 'a> Emit<'s, 'l, 'a> {
    pub(super) fn new(lower: &'l mut Lower<'a>, shape: &'s Shape) -> Self {
        Self { lower, shape, labels: Vec::new() }
    }

    /// The function's body, from its entry `block`.
    pub(super) fn body(&mut self, entry: Block) -> Result<(), Lack> {
        let Some(dispatch) = &self.shape.dispatch else { return self.tree(entry) };
        let (next, blocks) = (dispatch.next, dispatch.blocks.clone());
        self.lower.op(Op::I32Const(0));
        self.lower.op(Op::LocalSet(next));
        self.open(Op::Loop, Label::Dispatch);
        for _ in &blocks {
            self.open(Op::Block, Label::Other);
        }
        self.lower.op(Op::LocalGet(next));
        let places = u32::try_from(blocks.len()).expect("blocks fit u32");
        self.lower.op(Op::BrTable { cases: (0..places).collect(), default: places - 1 });
        for block in blocks {
            self.close();
            for &inst in &self.lower.f.blocks[block].insts {
                self.lower.inst(inst)?;
            }
            self.terminator(block)?;
        }
        self.close();
        Ok(())
    }

    /// The code of `block` and of what it dominates.
    fn tree(&mut self, block: Block) -> Result<(), Lack> {
        let mut merges: Vec<Block> =
            self.shape.children[block].iter().copied().filter(|&c| self.shape.merge[c]).collect();
        merges.sort_by_key(|&c| self.shape.rpo[c]);
        if self.shape.head[block] {
            self.open(Op::Loop, Label::Head(block));
            self.within(block, &merges)?;
            self.close();
        } else {
            self.within(block, &merges)?;
        }
        Ok(())
    }

    /// The code of `block`, then of `merges` in order, each after a `block` ending before it.
    fn within(&mut self, block: Block, merges: &[Block]) -> Result<(), Lack> {
        if let Some((&last, rest)) = merges.split_last() {
            self.open(Op::Block, Label::Follow(last));
            self.within(block, rest)?;
            self.close();
            return self.tree(last);
        }
        for &inst in &self.lower.f.blocks[block].insts {
            self.lower.inst(inst)?;
        }
        self.terminator(block)
    }

    fn terminator(&mut self, block: Block) -> Result<(), Lack> {
        let at = self.lower.f.blocks[block].origin;
        self.lower.mark(at);
        match &self.lower.f.blocks[block].term {
            Terminator::Jump(t) => self.jump(block, t),
            Terminator::Branch { cond, then, otherwise } => {
                self.lower.get(*cond);
                self.open(Op::If, Label::Other);
                self.jump(block, then)?;
                self.lower.op(Op::Else);
                self.jump(block, otherwise)?;
                self.close();
                Ok(())
            }
            Terminator::Switch { value, cases, default } => self.switch(block, *value, cases, default),
            Terminator::Return(result) => self.lower.ret(*result).map_err(|m| m.at(at)),
            Terminator::Unreachable => {
                self.lower.op(Op::Unreachable);
                Ok(())
            }
        }
    }

    /// A jump from `from` along `t`: its arguments into the target's parameters, then a branch to
    /// the target's label or its code.
    fn jump(&mut self, from: Block, t: &Target) -> Result<(), Lack> {
        let params = self.lower.f.blocks[t.block].params.clone();
        for &arg in &t.args {
            self.lower.get(arg);
        }
        for &p in params.iter().rev() {
            self.lower.set(p);
        }
        if let Some(dispatch) = &self.shape.dispatch {
            let place =
                dispatch.blocks.iter().position(|&b| b == t.block).expect("a jump's target is reached");
            self.lower.op(Op::I32Const(i32::try_from(place).expect("blocks fit i32")));
            self.lower.op(Op::LocalSet(dispatch.next));
            let depth = self.depth(Label::Dispatch);
            self.lower.op(Op::Br(depth));
            return Ok(());
        }
        let backward = self.shape.rpo[t.block] <= self.shape.rpo[from];
        if backward || self.shape.merge[t.block] {
            let label = if backward { Label::Head(t.block) } else { Label::Follow(t.block) };
            let depth = self.depth(label);
            self.lower.op(Op::Br(depth));
            Ok(())
        } else {
            self.tree(t.block)
        }
    }

    /// A switch: a `br_table` inside a `block` per distinct target, the jump to each target
    /// after its block's end.
    fn switch(
        &mut self,
        from: Block,
        value: vun_lir::Value,
        cases: &[(u128, Target)],
        default: &Target,
    ) -> Result<(), Lack> {
        let mut targets: Vec<&Target> = Vec::new();
        for t in cases.iter().map(|(_, t)| t).chain([default]) {
            if !targets.iter().any(|seen| seen.block == t.block) {
                targets.push(t);
            }
        }
        for _ in &targets {
            self.open(Op::Block, Label::Other);
        }
        let landing = |t: &Target| {
            let at = targets.iter().position(|seen| seen.block == t.block).expect("listed");
            u32::try_from(at).expect("targets fit u32")
        };
        let lands: Vec<(u128, u32)> = cases.iter().map(|(n, t)| (*n, landing(t))).collect();
        self.lower.dispatch(value, &lands, landing(default));
        for t in targets {
            self.close();
            self.jump(from, t)?;
        }
        Ok(())
    }

    fn open(&mut self, op: Op, label: Label) {
        self.lower.op(op);
        self.labels.push(label);
    }

    fn close(&mut self) {
        self.lower.op(Op::End);
        self.labels.pop();
    }

    /// The depth of the innermost label `label`, which is open.
    fn depth(&self, label: Label) -> u32 {
        let at = self.labels.iter().rev().position(|&l| l == label).expect("a jump's label is open");
        u32::try_from(at).expect("labels fit u32")
    }
}
