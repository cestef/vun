//! The forms of an instruction set whose instructions are words of fixed fields, as Arm's are: a
//! base word, the widths a form is defined at with the bits each sets, and the operand positions,
//! each a kind and the bit its field starts at. A kind says what it takes and the field an operand
//! encodes to ([`Field`]); whether an operand fits is whether it encodes, so a form never takes
//! what it cannot encode, and the encoder, the printer and form selection read one table.

/// What a form does with an operand, or with the flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
    ReadWrite,
    /// Nothing but compute its address.
    Address,
}

/// What an operand position of kind `Self` takes: the field an operand `O` encodes to in an
/// instruction of width `Z`, relative to the bit its position starts at, if it fits there.
pub trait Field<O, Z>: Copy {
    fn encode(self, operand: &O, size: Z) -> Option<u32>;
}

/// The register an operand names, whose encoding a form may require to differ from another's:
/// a register, or the base a memory operand writes back.
pub trait Named {
    fn named(&self) -> Option<u8>;
}

/// An operand position: what it takes, where its field goes, and what the form does with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arg<K> {
    pub kind: K,
    pub at: u8,
    /// Where the same field goes again, for an encoding naming one register in two fields
    /// (`cneg` is `csneg` of a register and itself; Thumb's `clz` names its source twice).
    pub again: Option<u8>,
    pub access: Access,
}

impl<K: Copy> Arg<K> {
    #[must_use]
    pub const fn new(kind: K, at: u8, access: Access) -> Self {
        Self { kind, at, again: None, access }
    }

    #[must_use]
    pub const fn again(self, at: u8) -> Self {
        Self { again: Some(at), ..self }
    }
}

/// A width a form is defined at, the bits it sets, the suffix it gives the mnemonic (`ldrb`), and
/// the qualifiers written after it, each after a `.` (`vcvt.s32.f32`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sized<Z> {
    pub size: Z,
    pub bits: u32,
    pub suffix: &'static str,
    pub qualifiers: &'static [&'static str],
}

impl<Z: Copy> Sized<Z> {
    #[must_use]
    pub const fn new(size: Z, bits: u32) -> Self {
        Self { size, bits, suffix: "", qualifiers: &[] }
    }

    #[must_use]
    pub const fn suffix(self, suffix: &'static str) -> Self {
        Self { suffix, ..self }
    }

    #[must_use]
    pub const fn qualifiers(self, qualifiers: &'static [&'static str]) -> Self {
        Self { qualifiers, ..self }
    }
}

/// One form of an instruction whose operand kinds are `K`, widths `Z` and general registers `R`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Form<K: 'static, Z: 'static, R: 'static> {
    pub base: u32,
    pub sizes: &'static [Sized<Z>],
    pub args: &'static [Arg<K>],
    /// The mnemonic, where it is not the instruction's (`ldur`).
    pub name: Option<&'static str>,
    /// The general registers it reads and writes without naming them (the link register of `bl`).
    pub reads: &'static [R],
    pub writes: &'static [R],
    /// What it does with the flags, if anything.
    pub flags: Option<Access>,
    /// Pairs of positions whose registers must differ: the manual calls the instruction
    /// unpredictable where they do not.
    pub apart: &'static [[u8; 2]],
}

impl<K: Copy, Z: Copy + PartialEq, R> Form<K, Z, R> {
    #[must_use]
    pub const fn new(base: u32, sizes: &'static [Sized<Z>], args: &'static [Arg<K>]) -> Self {
        Self { base, sizes, args, name: None, reads: &[], writes: &[], flags: None, apart: &[] }
    }

    #[must_use]
    pub const fn named(self, name: &'static str) -> Self {
        Self { name: Some(name), ..self }
    }

    #[must_use]
    pub const fn reads(self, reads: &'static [R]) -> Self {
        Self { reads, ..self }
    }

    #[must_use]
    pub const fn writes(self, writes: &'static [R]) -> Self {
        Self { writes, ..self }
    }

    #[must_use]
    pub const fn flags(self, flags: Access) -> Self {
        Self { flags: Some(flags), ..self }
    }

    #[must_use]
    pub const fn apart(self, apart: &'static [[u8; 2]]) -> Self {
        Self { apart, ..self }
    }

    /// Its definition at `size`.
    #[must_use]
    pub fn at(&self, size: Z) -> Option<&'static Sized<Z>> {
        self.sizes.iter().find(|s| s.size == size)
    }

    /// The kinds of its operands, in order.
    pub fn kinds(&self) -> impl Iterator<Item = K> {
        self.args.iter().map(|a| a.kind)
    }

    /// The word encoding `operands` at `size`, if this form takes them.
    #[must_use]
    pub fn encode<O>(&self, size: Z, operands: &[O]) -> Option<u32>
    where
        K: Field<O, Z>,
        O: Named,
    {
        let sized = self.at(size)?;
        if self.args.len() != operands.len() {
            return None;
        }
        let named = |at: u8| operands.get(usize::from(at)).and_then(Named::named);
        if self.apart.iter().any(|&[a, b]| named(a).is_some() && named(a) == named(b)) {
            return None;
        }
        self.args.iter().zip(operands).try_fold(self.base | sized.bits, |word, (arg, operand)| {
            let field = arg.kind.encode(operand, size)?;
            Some(word | field << arg.at | arg.again.map_or(0, |at| field << at))
        })
    }

    /// Whether this form takes `operands` at `size`.
    #[must_use]
    pub fn takes<O>(&self, size: Z, operands: &[O]) -> bool
    where
        K: Field<O, Z>,
        O: Named,
    {
        self.encode(size, operands).is_some()
    }

    /// The first of `forms` taking `operands` at `size`, and the word it encodes them to.
    #[must_use]
    pub fn select<'f, O>(forms: &'f [Self], size: Z, operands: &[O]) -> Option<(&'f Self, u32)>
    where
        K: Field<O, Z>,
        O: Named,
    {
        forms.iter().find_map(|f| f.encode(size, operands).map(|word| (f, word)))
    }
}
