//! What an instruction set tells about blocks of inline assembly (spec 07, section 6): the entries
//! of its table a block names, by their index in declaration order, and its operands as the
//! assembler takes them. Every backend emitting blocks, native code or GNU C, reads them here.

use vun_inline::{Base, Block, Bound, ClassId, FormId, Inst, Line, Mem, OpId, Operand, RegRef};

use crate::word::Access;
use crate::{Fit, Fixed, Given, Label, Misfit, Via, Width};

/// Operands of inline assembly a backend may have no encoding or no register for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AsmOperand {
    /// Memory with an index register.
    Indexed,
    /// Memory at a symbol's address past a register.
    SymbolOffRegister,
    /// Memory at a fixed address, with no register or symbol.
    Absolute,
    /// Memory at the address of something another module defines, which only its entry in the
    /// global offset table holds.
    Imported,
    /// A value bound in a register of a class no value is allocated to.
    Unallocated,
    /// Operands the instruction's form does not encode, such as an immediate none of its
    /// encodings holds.
    Unencodable,
    /// A symbol, in text a C compiler assembles, where its assembly name is the object format's.
    Symbol,
    /// More bindings than registers the block leaves them.
    Crowded,
}

/// An instruction set's facts about blocks of inline assembly.
pub trait Blocks {
    /// The namespace of the instruction set, `lang.arch.<isa>`, which a block names.
    const ARCH: vun_items::Item;

    /// The class of general registers, which values and addresses are held in.
    const GENERAL: ClassId;

    /// A general register, of the class values are held in.
    type Reg: Copy + Eq + 'static;
    /// The width of an operation or an operand.
    type Size: Width;
    /// What an operand position takes.
    type Kind: Copy;
    /// An operand as the assembler encodes it, `S` naming what the linker places.
    type Operand<S: Copy>;

    /// The kinds of the positions of the form `inst` was checked against, in order.
    fn kinds<V, P, S>(inst: &Inst<V, P, S>) -> impl Iterator<Item = Self::Kind>;

    /// What the form of `inst` does with each of its positions, in order.
    fn accesses<V, P, S>(inst: &Inst<V, P, S>) -> impl Iterator<Item = Access>;

    /// The general registers that form reads or writes without naming them.
    fn implicit<V, P, S>(inst: &Inst<V, P, S>) -> impl Iterator<Item = Self::Reg>;

    /// The memory operand the form of `inst` writes, by its index among the operands, and the
    /// bytes it writes there.
    fn written<V, P, S>(inst: &Inst<V, P, S>) -> Option<(usize, u32)>;

    /// The general register `r` is, if it is of that class.
    fn general(r: RegRef) -> Option<Self::Reg>;

    /// A general register, as a block names it.
    fn reference(r: Self::Reg) -> RegRef;

    /// The class of register a value bound at a position of `kind` is held in, and the register
    /// the position takes, if one; `None` is an address's register, a general one.
    ///
    /// # Errors
    /// The position takes no register a value is held in.
    fn binding(kind: Option<Self::Kind>) -> Result<(ClassId, Option<Self::Reg>), AsmOperand>;

    /// Whether memory a block names at a position of `kind` is one the assembler addresses.
    ///
    /// # Errors
    /// What it does not address.
    fn addressable<V, P, S>(mem: &Mem<V, P, S>, kind: Self::Kind) -> Result<(), AsmOperand>;

    /// The width a line's operands give the form `form` of `op`, and each as the form takes it,
    /// `fixed` what its mnemonic's spelling supplies. A form checked so is one the assembler
    /// encodes, by the same predicates; an immediate must also be a value the operation's bits
    /// hold.
    ///
    /// # Errors
    /// Why the form does not take them.
    fn fit<V: Clone, P: Clone, S: Copy>(
        op: OpId,
        form: FormId,
        fixed: Fixed,
        givens: &[Given<V, P, S>],
    ) -> Result<Fit<V, P, S>, Misfit>;

    /// A label, as a branch's target.
    fn label<S: Copy>(label: Label) -> Self::Operand<S>;

    /// Something the linker places, as a branch's target.
    fn target<S: Copy>(target: S) -> Self::Operand<S>;

    /// A block's operand at a position of `kind` as the assembler takes it, placed where `place`
    /// says.
    ///
    /// # Panics
    /// Panics on memory the assembler does not address ([`Blocks::addressable`]).
    /// The bindings of `block`, in the order `Block::bindings` gives them, each with the kind of
    /// the position it is at.
    #[must_use]
    fn uses<V, P, S>(block: &Block<V, P, S>) -> impl Iterator<Item = Use<'_, Self, V, P, S>>
    where
        Self: Sized,
    {
        block.insts.iter().flat_map(|inst| {
            Self::kinds(inst).zip(&inst.operands).flat_map(move |(kind, operand)| {
                let (kind, bounds) = match operand {
                    Operand::Bound(b) => (Some(kind), [Some(b), None]),
                    Operand::Mem(mem) => {
                        let index = mem.index.as_ref().map(|(index, _)| index);
                        (
                            None,
                            [mem.base.as_ref(), index].map(|r| match r? {
                                Base::Bound(b) => Some(b),
                                Base::Reg(_) => None,
                            }),
                        )
                    }
                    _ => (None, [None, None]),
                };
                bounds.into_iter().flatten().map(move |bound| Use { inst, kind, bound })
            })
        })
    }

    /// The general registers `block` names, as operands or registers of addresses, or its forms
    /// use unnamed.
    #[must_use]
    fn named<V, P, S>(block: &Block<V, P, S>) -> impl Iterator<Item = Self::Reg> {
        block.insts.iter().flat_map(|inst| {
            let named = inst.operands.iter().flat_map(|operand| match operand {
                Operand::Reg(r) | Operand::Mem(Mem { base: Some(Base::Reg(r)), .. }) => {
                    std::slice::from_ref(r)
                }
                Operand::List(regs) => regs.as_slice(),
                _ => &[],
            });
            let named = named.filter_map(|&r| Self::general(r));
            Self::implicit(inst).chain(named)
        })
    }

    /// The general registers `block` may overwrite: those it names but at a position it only
    /// reads, the registers of addresses (a form may write an address back), and those its forms
    /// use unnamed.
    #[must_use]
    fn overwritten<V, P, S>(block: &Block<V, P, S>) -> impl Iterator<Item = Self::Reg> {
        block.insts.iter().flat_map(|inst| {
            let named =
                inst.operands.iter().zip(Self::accesses(inst)).flat_map(|(operand, access)| match operand {
                    Operand::Reg(_) if access == Access::Read => &[],
                    Operand::Reg(r) | Operand::Mem(Mem { base: Some(Base::Reg(r)), .. }) => {
                        std::slice::from_ref(r)
                    }
                    Operand::List(regs) => regs.as_slice(),
                    _ => &[],
                });
            let named = named.filter_map(|&r| Self::general(r));
            Self::implicit(inst).chain(named)
        })
    }

    fn operand<V, P, S: Copy>(
        operand: &Operand<V, P, S>,
        kind: Self::Kind,
        place: &mut impl Place<Self, S>,
    ) -> Self::Operand<S>;

    /// The register of the next binding at a position of `kind`, as a block names it.
    ///
    /// # Panics
    /// For a position no value is held at, which checking the block refuses.
    fn bound<S: Copy>(kind: Option<Self::Kind>, place: &mut impl Place<Self, S>) -> RegRef
    where
        Self: Sized,
    {
        let (class, _) = Self::binding(kind).expect("a checked block binds values where they are held");
        place.bound(class)
    }

    /// The general register of the next binding, an address's.
    ///
    /// # Panics
    /// Where the architecture holds addresses in no general register.
    fn address<S: Copy>(place: &mut impl Place<Self, S>) -> Self::Reg
    where
        Self: Sized,
    {
        Self::general(place.bound(Self::GENERAL)).expect("a general register")
    }
}

/// Where a block's operands are, as the code it is emitted into has them.
pub trait Place<I: Blocks + ?Sized, S: Copy> {
    /// The register of the next binding, of `class`, in the order `Block::bindings` gives them.
    fn bound(&mut self, class: ClassId) -> RegRef;

    /// The operand naming the start of `line`.
    fn line(&self, line: Line) -> I::Operand<S>;

    /// How a page or an address reaches `symbol`.
    fn via(&self, symbol: S) -> Via;
}

/// A binding of a block, with its instruction and the kind of the position it is at: `None` for
/// an address's register.
pub struct Use<'a, I: Blocks, V, P, S> {
    pub inst: &'a Inst<V, P, S>,
    pub kind: Option<I::Kind>,
    pub bound: &'a Bound<V, P>,
}

impl<I: Blocks, V, P, S> Use<'_, I, V, P, S> {
    /// The class of register it is held in.
    ///
    /// # Errors
    /// Its position takes no register a value is held in.
    pub fn class(&self) -> Result<ClassId, AsmOperand> {
        I::binding(self.kind).map(|(class, _)| class)
    }

    /// The register it must be in, if one: the one it names, or the one its position takes.
    ///
    /// # Errors
    /// Its position takes no register a value is held in, or it names one of another class.
    pub fn fixed(&self) -> Result<Option<RegRef>, AsmOperand> {
        let (class, taken) = I::binding(self.kind)?;
        match self.bound.reg() {
            Some(named) if named.class != class => Err(AsmOperand::Unallocated),
            Some(named) => Ok(Some(named)),
            None => Ok(taken.map(I::reference)),
        }
    }
}
