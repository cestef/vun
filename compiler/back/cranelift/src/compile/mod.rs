//! Compiling every function of a unit, in parallel, into code with its relocations.

mod function;
mod lowering;

use cranelift_codegen::ir::LibCall;
use vun_asm::{Code, Reloc};
use vun_backend::{Assisted, Compiled, Lack};
use vun_index::{Idx, IndexVec};
use vun_lir::analysis::Reached;
use vun_lir::{Address, FuncId, ImportId, Unit};
use vun_origin::OriginId;
use vun_par::Parallel;
use vun_trace::Activity;

use crate::failed::Failed;
use crate::isa::Isa;

/// What compiled code calls that the unit does not name: what Cranelift itself needs of the
/// runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Support {
    /// The address of a thread's storage, which the general dynamic model asks the loader for.
    TlsGetAddr,
    /// The copy of a struct argument into the caller's stack, which x86-64 makes with a call.
    Memcpy,
}

impl Support {
    /// The libcalls Cranelift may name, and what each is.
    const CALLS: &[(LibCall, Self)] =
        &[(LibCall::ElfTlsGetAddr, Self::TlsGetAddr), (LibCall::Memcpy, Self::Memcpy)];

    /// The link symbol of the function.
    pub(crate) const fn symbol(self) -> &'static str {
        match self {
            Self::TlsGetAddr => "__tls_get_addr",
            Self::Memcpy => "memcpy",
        }
    }

    fn of(call: LibCall) -> Option<Self> {
        Self::CALLS.iter().find(|(c, _)| *c == call).map(|&(_, support)| support)
    }
}

const COMPILE: Activity = Activity::stage("cranelift compile functions");

/// What compiles the functions given by their ids, which Cranelift cannot express: another
/// backend's code for them.
pub(crate) type Assist<'a> = &'a dyn Fn(&[FuncId]) -> Assisted;

/// What a relocation of compiled code names.
#[derive(Clone, Copy)]
pub(crate) enum Named {
    Address(Address),
    /// The body of a function with an entry, which follows the entry in its code.
    Body(FuncId),
    Support(Support),
}

/// A function compiled: its code, and its text if it was asked for.
pub(crate) struct Function {
    pub(crate) code: Code<(), Named>,
    /// Where its body starts, if the code is an entry followed by it.
    pub(crate) body_at: Option<u64>,
    /// The bytes of its frame below the frame pointer.
    pub(crate) frame: u64,
    pub(crate) marks: Vec<(u64, OriginId)>,
    pub(crate) text: Option<Listed>,
}

/// A function's text for people: its Cranelift IR and machine code, and the origins the IR's
/// source locations number.
pub(crate) struct Listed {
    pub(crate) text: String,
    pub(crate) origins: Vec<OriginId>,
}

/// What became of a function.
enum Outcome {
    /// Nothing reaches it.
    Unreached,
    Compiled(Function),
    /// It holds what Cranelift cannot express, and goes to the assisting backend.
    Lacking(Lack),
}

impl Outcome {
    fn lack(&self) -> Option<Lack> {
        match self {
            Self::Lacking(lack) => Some(*lack),
            Self::Unreached | Self::Compiled(_) => None,
        }
    }
}

/// The code of a unit's functions, with what they call beyond it.
pub(crate) struct Compilation {
    pub(crate) code: IndexVec<FuncId, Compiled<()>>,
    /// The support functions the code calls, each the import after the unit's, in order.
    pub(crate) support: Vec<Support>,
    pub(crate) listing: Vec<(FuncId, Listed)>,
}

impl Compilation {
    /// The code of `unit` for `isa`, with a text of each function if `listed`.
    ///
    /// # Errors
    /// Returns the first thing the backend cannot compile.
    pub(crate) fn of(
        unit: &Unit,
        isa: &Isa,
        assist: Option<Assist<'_>>,
        listed: bool,
    ) -> Result<Self, Failed> {
        let reached = Reached::of(unit);
        let functions: Vec<_> = unit.functions.iter().collect();
        let lowered = Self::lowered(unit, isa, &reached, &functions);
        let word = u64::from(unit.pointer_bits / u8::BITS);
        let compiling = COMPILE.start();
        let lowered: Vec<_> = lowered.iter().zip(&functions).collect();
        let outcomes = lowered.par_map(|&(lowered, &(id, f))| {
            let _span =
                vun_trace::tracing::debug_span!("cranelift", id = id.index(), f = %f.linkage).entered();
            let lowered = match lowered {
                None => return Ok::<_, Failed>(Outcome::Unreached),
                Some(Ok(lowered)) => lowered,
                Some(Err(lack)) => return Ok(Outcome::Lacking(*lack)),
            };
            let compiled = Self::compiled(isa, lowered, f.origin, listed, word)?;
            vun_trace::tracing::debug!(bytes = compiled.code.bytes.len(), "compiled");
            Ok(Outcome::Compiled(compiled))
        });
        drop(compiling);
        let outcomes = outcomes.into_iter().collect::<Result<Vec<_>, _>>()?;
        let lacking: Vec<(FuncId, Lack)> =
            outcomes.iter().zip(&functions).filter_map(|(o, &(id, _))| o.lack().map(|l| (id, l))).collect();
        let mut others = Vec::new();
        if let Some(&(_, first)) = lacking.first() {
            let Some(assist) = assist else { return Err(first.into()) };
            let ids: Vec<FuncId> = lacking.iter().map(|&(id, _)| id).collect();
            for &(id, lack) in &lacking {
                vun_trace::tracing::debug!(id = id.index(), missing = ?lack.missing, "handed to the assisting backend");
            }
            others = assist(&ids)?;
        }
        let mut others = others.into_iter().peekable();
        let body_at: IndexVec<FuncId, Option<u64>> = outcomes
            .iter()
            .map(|o| match o {
                Outcome::Compiled(compiled) => compiled.body_at,
                Outcome::Unreached | Outcome::Lacking(_) => None,
            })
            .collect();
        let mut done = Self { code: IndexVec::new(), support: Vec::new(), listing: Vec::new() };
        for ((id, _), outcome) in functions.iter().zip(outcomes) {
            let compiled = match outcome {
                Outcome::Compiled(compiled) => compiled,
                Outcome::Unreached => {
                    done.code.push(Compiled::unreached());
                    continue;
                }
                Outcome::Lacking(_) => {
                    let (_, code) = others.next_if(|(other, _)| other == id).expect("what was handed over");
                    done.code.push(code);
                    continue;
                }
            };
            let relocs = compiled.code.relocs.iter().map(|r| Reloc {
                offset: r.offset,
                target: match r.target {
                    Named::Address(address) => address,
                    Named::Body(f) => Address::Func(f),
                    Named::Support(support) => done.import(unit, support),
                },
                kind: r.kind,
                addend: r.addend
                    + match r.target {
                        Named::Body(f) => {
                            i64::try_from(body_at[f].expect("a body has an entry")).expect("code fits")
                        }
                        Named::Address(_) | Named::Support(_) => 0,
                    },
            });
            let relocs = relocs.collect();
            let code = Code { bytes: compiled.code.bytes, relocs, listing: Vec::new() };
            done.code.push(Compiled {
                code,
                frame: compiled.frame,
                marks: compiled.marks,
                unwind: Vec::new(),
            });
            done.listing.extend(compiled.text.map(|text| (*id, text)));
        }
        Ok(done)
    }

    /// The import standing for `support`, added the first time it is called.
    fn import(&mut self, unit: &Unit, support: Support) -> Address {
        let at = self.support.iter().position(|&s| s == support).unwrap_or_else(|| {
            self.support.push(support);
            self.support.len() - 1
        });
        Address::Import(ImportId::new(unit.imports.len() + at))
    }
}
