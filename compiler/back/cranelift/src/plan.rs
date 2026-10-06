//! Where a call of a signature places the values Cranelift does not place as the target's
//! convention says: C records passed or returned by value.
//!
//! `vun-abi` classifies a signature as the convention does; Cranelift places scalars by its own
//! rules, and a record is lowered as scalars (its parts in registers), a struct argument or a
//! copy passed by address. The two must agree: [`Placed::of`] walks the classifier's placements
//! the way Cranelift counts registers and the stack, and refuses a signature where they differ
//! (AAPCS64 closes the registers a record did not fit, which Cranelift does not).

use cranelift_codegen::ir::{Type, types};
use vun_abi::{Carrier, Convention, Floats, Loc, Part, Passing, Registers, Ret, Slots, Stack};
use vun_backend::Missing;
use vun_lir::{AbiType, Record, Scalar, Signature};

/// The bytes a stack argument takes at the least, and the alignment of one.
const SLOT: u64 = 8;

/// The size and alignment of a record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Layout {
    pub(crate) size: u64,
    pub(crate) align: u64,
}

impl From<&Record> for Layout {
    fn from(record: &Record) -> Self {
        Self { size: record.size, align: record.align }
    }
}

/// The bytes of a record travelling in one register: where they lie in it, and the scalar the
/// register holds them as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Lane {
    pub(crate) offset: u64,
    pub(crate) ty: Type,
}

/// A record travelling in registers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Parts {
    pub(crate) layout: Layout,
    pub(crate) lanes: Vec<Lane>,
}

/// How one parameter of a signature is passed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Pass {
    /// As many LIR values, each as the value Cranelift passes.
    Values(usize),
    /// A record in registers, loaded from and stored to its memory.
    Parts(Parts),
    /// A record on the stack, as Cranelift's struct argument.
    Struct(Layout),
    /// A record the caller copies, passed by the address of the copy.
    Copy(Layout),
}

/// How a result comes back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Returned {
    /// As the value Cranelift returns, if there is one.
    Direct,
    /// Written through the address passed after the parameters (Vun's convention).
    Through,
    /// Written through the address Cranelift's struct return passes where the convention does.
    Sret,
    /// A record in registers, which the caller stores where its result goes.
    Parts(Parts),
}

/// How a signature's values are passed and its result comes back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Placed {
    pub(crate) params: Vec<Pass>,
    pub(crate) returned: Returned,
}

/// Registers handed out in order, and the stack arguments follow, as Cranelift places them.
struct Cursor<'a, I, F> {
    ints: &'a [I],
    floats: &'a [F],
    used: (usize, usize),
    stack: u64,
}

impl<'a, I: Copy + PartialEq, F: Copy + PartialEq> Cursor<'a, I, F> {
    const fn new(ints: &'a [I], floats: &'a [F]) -> Self {
        Self { ints, floats, used: (0, 0), stack: 0 }
    }

    /// Takes the next register of the class of `carrier`, which must be the one named.
    fn reg(&mut self, carrier: Carrier<I, F>) -> Result<(), Missing> {
        let next = match carrier {
            Carrier::Int(r) => self.ints.get(self.used.0) == Some(&r),
            Carrier::Float(r) => self.floats.get(self.used.1) == Some(&r),
        };
        if !next {
            return Err(Missing::Placement);
        }
        match carrier {
            Carrier::Int(_) => self.used.0 += 1,
            Carrier::Float(_) => self.used.1 += 1,
        }
        Ok(())
    }

    /// A scalar of `bytes` placed at `offset` of the stack arguments, which Cranelift does only
    /// once every register of its class is taken.
    fn spill(&mut self, float: bool, offset: u64, bytes: u64) -> Result<(), Missing> {
        let taken = if float { self.used.1 >= self.floats.len() } else { self.used.0 >= self.ints.len() };
        let stride = bytes.max(SLOT);
        let at = self.stack.next_multiple_of(stride);
        if !taken || offset != at {
            return Err(Missing::Placement);
        }
        self.stack = at + stride;
        Ok(())
    }

    /// A scalar placed by `loc`.
    fn scalar(&mut self, scalar: Scalar, loc: &Loc<Carrier<I, F>>) -> Result<(), Missing> {
        match loc {
            Loc::Reg(carrier) => self.reg(*carrier),
            &Loc::Stack { offset, bytes } => self.spill(scalar.is_float(), offset, bytes),
            _ => Err(Missing::Placement),
        }
    }

    /// The pieces of `record` travelling in `parts`, each in the register the cursor is at.
    fn lanes(&mut self, record: &Record, parts: &[Part<Carrier<I, F>>]) -> Result<Parts, Missing> {
        let lanes = parts.iter().map(|part| {
            self.reg(part.reg)?;
            let ty = match (part.reg, part.bytes) {
                (Carrier::Int(_), 1) => types::I8,
                (Carrier::Int(_), 2) => types::I16,
                (Carrier::Int(_), 4) => types::I32,
                (Carrier::Int(_), 8) => types::I64,
                (Carrier::Float(_), 4) => types::F32,
                (Carrier::Float(_), 8) => types::F64,
                _ => return Err(Missing::Record),
            };
            Ok(Lane { offset: part.offset, ty })
        });
        Ok(Parts { layout: record.into(), lanes: lanes.collect::<Result<_, _>>()? })
    }
}

impl Placed {
    /// Every parameter passed as Cranelift passes it, and the result returned as it does.
    #[must_use]
    pub(crate) fn plain(signature: &Signature) -> Self {
        let values = |ty: &AbiType| Pass::Values(ty.values().count());
        Self { params: signature.params.iter().map(values).collect(), returned: Returned::Direct }
    }

    /// The placement of `signature` under `convention`, a record lowered where Cranelift's own
    /// placement of what it is lowered to is the convention's. A struct argument is used only if
    /// `structs` says the processor has them.
    ///
    /// # Errors
    /// Returns [`Missing::Placement`] where the two differ, and [`Missing::Record`] for a record
    /// a register cannot hold as one scalar.
    pub(crate) fn of<I, F>(
        signature: &Signature,
        convention: &Convention<I, F>,
        structs: bool,
    ) -> Result<Self, Missing>
    where
        I: Copy + PartialEq,
        F: Copy + PartialEq,
    {
        let rules = convention.rules;
        if rules.floats != Floats::Next || rules.slots != Slots::Banks || rules.stack != Stack::Words {
            return Err(Missing::Placement);
        }
        let passing = Passing::of(signature, convention)?;
        let Registers { int_args, float_args, int_results, float_results, result_address, .. } =
            convention.registers;
        let (mut args, mut rets) =
            (Cursor::new(int_args, float_args), Cursor::new(int_results, float_results));
        let through = signature.through().then(|| passing.params.last()).flatten();
        let returned = match (&signature.result, &passing.ret) {
            (Some(AbiType::Record(r)), Ret::Split(parts)) => Returned::Parts(rets.lanes(r, parts)?),
            (Some(AbiType::Record(_)), Ret::Memory) => {
                // The address takes the first argument register, which Cranelift hands it too.
                if let (None, Some(Loc::Reg(carrier))) = (result_address, through) {
                    args.reg(*carrier)?;
                }
                Returned::Sret
            }
            _ => Returned::Direct,
        };
        let mut locs = passing.params.iter();
        let mut params = Vec::new();
        for ty in &signature.params {
            let AbiType::Record(r) = ty else {
                for scalar in ty.values() {
                    args.scalar(scalar, locs.next().expect("a placement per value"))?;
                }
                params.push(Pass::Values(ty.values().count()));
                continue;
            };
            params.push(match locs.next().expect("a placement per value") {
                Loc::Split { parts, rest: None } => Pass::Parts(args.lanes(r, parts)?),
                &Loc::Copied { offset, size } if structs && size % SLOT == 0 && offset == args.stack => {
                    args.stack += size;
                    Pass::Struct(r.into())
                }
                Loc::Indirect { at, .. } => {
                    args.scalar(Scalar::Ptr, at)?;
                    Pass::Copy(r.into())
                }
                _ => return Err(Missing::Placement),
            });
        }
        Ok(Self { params, returned })
    }

    /// The placement under the convention named `named` of those `conventions` declare, whose
    /// registers it reads.
    ///
    /// # Errors
    /// Returns [`Missing::Convention`] for one they do not declare, and what [`Placed::of`] does.
    pub(crate) fn by<I, F>(
        signature: &Signature,
        conventions: &[(vun_target::Convention, &Convention<I, F>)],
        named: vun_target::Convention,
        structs: bool,
    ) -> Result<Self, Missing>
    where
        I: Copy + PartialEq,
        F: Copy + PartialEq,
    {
        let (_, convention) = conventions.iter().find(|(n, _)| *n == named).ok_or(Missing::Convention)?;
        Self::of(signature, convention, structs)
    }
}
