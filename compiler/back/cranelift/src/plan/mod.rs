//! Where a call of a signature places the values Cranelift does not place as the target's
//! convention says: C records passed or returned by value.
//!
//! `vun-abi` classifies a signature as the convention does; Cranelift places scalars by its own
//! rules, and a record is lowered as scalars (its parts in registers), a struct argument or a
//! copy passed by address. The two must agree: [`Placed::of`] walks the classifier's placements
//! the way Cranelift counts registers and the stack, and refuses a signature where they differ
//! (AAPCS64 closes the registers a record did not fit, which Cranelift does not): a register the
//! convention leaves unused is taken by a parameter carrying nothing ([`Skip`]), so that what
//! follows is where the convention puts it.

mod cursor;

use cranelift_codegen::ir::Type;
use cursor::Cursor;
use vun_abi::{Convention, Floats, Loc, Passing, Registers, Ret, Slots, Stack};
use vun_backend::Missing;
use vun_lir::{AbiType, Record, Scalar, Signature};

/// The bytes a stack argument takes at the least, and the alignment of one.
pub(crate) const SLOT: u64 = 8;

/// The size and alignment of a record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Layout {
    pub(crate) size: u64,
    pub(crate) align: u64,
}

impl Layout {
    /// The bytes it takes on the stack, in words.
    pub(crate) const fn words(self) -> u64 {
        self.size.next_multiple_of(SLOT)
    }
}

impl From<&Record> for Layout {
    fn from(record: &Record) -> Self {
        Self { size: record.size, align: record.align }
    }
}

/// The bytes of a record travelling in one register: where they lie in it, how many there are,
/// and the scalar the register holds them as, wider than the bytes where they are not a power of
/// two.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Lane {
    pub(crate) offset: u64,
    pub(crate) bytes: u64,
    pub(crate) ty: Type,
}

/// A parameter the convention's placement needs and the function takes nothing from: a register
/// it leaves unused, or a word of the stack it skips.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Skip {
    Int,
    Float,
    Stack,
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
    /// A record on the stack, as Cranelift's struct argument, which takes whole words.
    Struct(Layout),
    /// A record on the stack, as the words of it each passed as an integer, once the registers
    /// are taken, where the processor has no struct arguments.
    Words(Layout),
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

/// A parameter of a signature: what comes before it, and how it is passed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Param {
    pub(crate) skips: Vec<Skip>,
    pub(crate) pass: Pass,
}

impl From<Pass> for Param {
    fn from(pass: Pass) -> Self {
        Self { skips: Vec::new(), pass }
    }
}

/// How a signature's values are passed and its result comes back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Placed {
    pub(crate) params: Vec<Param>,
    pub(crate) returned: Returned,
}

impl Placed {
    /// Every parameter passed as Cranelift passes it, and the result returned as it does.
    #[must_use]
    pub(crate) fn plain(signature: &Signature) -> Self {
        let values = |ty: &AbiType| Pass::Values(ty.values().count());
        let params = signature.params.iter().map(|ty| values(ty).into());
        Self { params: params.collect(), returned: Returned::Direct }
    }

    /// The placement of `signature` under `convention`, a record lowered where Cranelift's own
    /// placement of what it is lowered to is the convention's, with parameters carrying nothing
    /// where the convention leaves registers or words of the stack unused. A struct argument is
    /// used only if `structs` says the processor has them.
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
            (Some(AbiType::Record(r)), Ret::Split(parts)) => match rets.lanes(r, parts)? {
                (skips, parts) if skips.is_empty() => Returned::Parts(parts),
                _ => return Err(Missing::Placement),
            },
            (Some(AbiType::Record(_)), Ret::Memory) => {
                // The address takes the first argument register, which Cranelift hands it too.
                if let (None, Some(Loc::Reg(carrier))) = (result_address, through)
                    && !args.reg(*carrier)?.is_empty()
                {
                    return Err(Missing::Placement);
                }
                Returned::Sret
            }
            _ => Returned::Direct,
        };
        let mut locs = passing.params.iter();
        let mut params = Vec::new();
        for ty in &signature.params {
            let AbiType::Record(r) = ty else {
                let mut skips = Vec::new();
                for (at, scalar) in ty.values().enumerate() {
                    let skipped = args.scalar(scalar, locs.next().expect("a placement per value"))?;
                    if at == 0 {
                        skips = skipped;
                    } else if !skipped.is_empty() {
                        return Err(Missing::Placement);
                    }
                }
                params.push(Param { skips, pass: Pass::Values(ty.values().count()) });
                continue;
            };
            params.push(match locs.next().expect("a placement per value") {
                Loc::Split { parts, rest: None } => {
                    let (skips, parts) = args.lanes(r, parts)?;
                    Param { skips, pass: Pass::Parts(parts) }
                }
                &Loc::Copied { offset, .. } => args.stacked(r, offset, structs)?,
                Loc::Indirect { at, .. } => {
                    let skips = args.scalar(Scalar::Ptr, at)?;
                    Param { skips, pass: Pass::Copy(r.into()) }
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
