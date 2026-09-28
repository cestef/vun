//! What the instruction compilers of load and store architectures share: how an instruction is
//! compiled, decided once as a [`Selection`], and what each instruction, terminator and the entry
//! needs of the plan, read from that decision (`needs`), so that what the plan provides and what
//! is emitted agree.

mod needs;

use vun_abi::{Carrier, Convention, Passing};
use vun_backend::Lack;
use vun_codegen::snippet::{Emits, Snippet};
use vun_codegen::{Cx, Needs};
use vun_lir::{Callee, Cond, Inst, RmwOp, Value};

use crate::inline::Inline;
use needs::Looked;

/// An instruction as it is compiled: a snippet over its operands at a size, or a sequence the
/// architecture writes itself.
pub enum Selection<'f, A: Emits> {
    Snippet {
        snippet: Snippet<A>,
        ins: Vec<Value>,
        size: A::Size,
        cond: Option<A::Cond>,
    },
    Call {
        callee: &'f Callee,
        args: &'f [Value],
    },
    /// An atomic read-modify-write or compare-exchange, as a loop of exclusive accesses.
    Rmw {
        op: RmwOp,
        addr: Value,
        value: Value,
        size: A::Size,
    },
    CompareExchange {
        addr: Value,
        expected: Value,
        new: Value,
        size: A::Size,
    },
    Copy {
        to: Value,
        from: Value,
        bytes: u64,
    },
    Asm(&'f vun_lir::Asm, Option<&'f vun_lir::AsmCheck>),
    /// An operation reading or giving a value of the float bank, which lives in its home
    /// (`Arch::bank`), as the architecture writes it: its other operands in registers.
    Homed {
        ins: Vec<Value>,
    },
}

/// A register a value travels in under `A`'s conventions.
pub type Carried<A> = Carrier<<A as vun_codegen::Arch>::Reg, <A as Selects>::Float>;

/// A load and store architecture's instruction compilers, as the needs they share read them.
pub trait Selects: Inline + Emits {
    /// A floating-point register, which conventions pass floats in.
    type Float: Copy + 'static;

    /// Copies up to this many bytes move through a register, larger ones in a loop over three
    /// scratch registers.
    const UNROLLED: u64;

    /// How `inst` is compiled.
    ///
    /// # Errors
    /// Returns what the backend lacks for it.
    fn selection<'a>(cx: &Cx<'a, Self>, inst: Inst) -> Result<Selection<'a, Self>, Lack>;

    /// The flags a branch fusing the comparison `cond` of `a` reads, as a snippet, and its size.
    ///
    /// # Errors
    /// Returns what the backend lacks for it.
    fn flags(cx: &Cx<'_, Self>, cond: Cond, a: Value)
    -> Result<(Snippet<Self>, <Self as Emits>::Size), Lack>;

    /// The size `v` is held at in a general register.
    ///
    /// # Errors
    /// Returns what the backend lacks for it.
    fn held(cx: &Cx<'_, Self>, v: Value) -> Result<<Self as Emits>::Size, Lack>;

    /// Whether an operation of `size` reads narrow operands, which it extends first.
    fn narrow(size: <Self as Emits>::Size) -> bool;

    /// The variant of the call standard the function compiled follows, and where its own
    /// signature passes each value.
    fn convention(cx: &Cx<'_, Self>) -> &'static Convention<Self::Reg, Self::Float>;
    fn passing<'c>(cx: &'c Cx<'_, Self>) -> &'c Passing<Carried<Self>>;

    /// What the backend lacks to compile `inst`.
    ///
    /// # Errors
    /// Returns it.
    fn lacking(cx: &Cx<'_, Self>, inst: Inst) -> Result<(), Lack> {
        Self::selection(cx, inst).map(drop)
    }

    /// What `inst` needs; nothing for one the backend cannot compile, which compiling reports.
    fn needs_of(cx: &Cx<'_, Self>, inst: Inst, needs: &mut Needs) {
        Looked(cx).needs(inst, needs);
    }

    /// What the terminator of `block` reads itself.
    fn term_needs_of(cx: &Cx<'_, Self>, block: vun_lir::Block, needs: &mut Needs) {
        Looked(cx).term_needs(block, needs);
    }

    /// Where the entry receives each parameter.
    fn entry_needs_of(cx: &Cx<'_, Self>, needs: &mut Needs) {
        Looked(cx).entry_needs(needs);
    }
}
