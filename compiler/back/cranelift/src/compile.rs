//! Compiling every function of a unit, in parallel, into code with its relocations.

use cranelift_codegen::control::ControlPlane;
use cranelift_codegen::ir::{ExternalName, LibCall};
use cranelift_codegen::{CodegenError, Context, FinalizedRelocTarget};
use vun_asm::{Code, Reloc};
use vun_backend::{Assisted, Compiled, Lack};
use vun_index::{Idx, IndexVec};
use vun_lir::analysis::Reached;
use vun_lir::{Address, FuncId, ImportId, Unit};
use vun_origin::OriginId;
use vun_par::Parallel;

use crate::failed::Failed;
use crate::func::{Cx, Lowered};
use crate::isa::Isa;
use crate::names::Addressed;
use crate::relocs::Placed;

/// What compiled code calls that the unit does not name: what Cranelift itself needs of the
/// runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Support {
    /// The address of a thread's storage, which the general dynamic model asks the loader for.
    TlsGetAddr,
}

impl Support {
    /// The libcalls Cranelift may name, and what each is.
    const CALLS: &[(LibCall, Self)] = &[(LibCall::ElfTlsGetAddr, Self::TlsGetAddr)];

    /// The link symbol of the function.
    pub(crate) const fn symbol(self) -> &'static str {
        match self {
            Self::TlsGetAddr => "__tls_get_addr",
        }
    }

    fn of(call: LibCall) -> Option<Self> {
        Self::CALLS.iter().find(|(c, _)| *c == call).map(|&(_, support)| support)
    }
}

/// What compiles the functions given by their ids, which Cranelift cannot express: another
/// backend's code for them.
pub(crate) type Assist<'a> = &'a dyn Fn(&[FuncId]) -> Assisted;

/// What a relocation of compiled code names.
#[derive(Clone, Copy)]
enum Named {
    Address(Address),
    Support(Support),
}

/// A function compiled: its code, and its text if it was asked for.
struct Function {
    code: Code<(), Named>,
    /// The bytes of its frame below the frame pointer.
    frame: u64,
    marks: Vec<(u64, OriginId)>,
    text: Option<Listed>,
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
        let cx = Cx::new(unit, isa, &reached);
        let functions: Vec<_> = unit.functions.iter().collect();
        let outcomes = functions.par_map(|&(id, f)| {
            if !reached.contains(id) {
                return Ok::<_, Failed>(Outcome::Unreached);
            }
            let _span =
                vun_trace::tracing::debug_span!("cranelift", id = id.index(), f = %f.linkage).entered();
            let lowered = match cx.lower(id, f) {
                Ok(lowered) => lowered,
                Err(lack) => return Ok(Outcome::Lacking(lack)),
            };
            let compiled = Self::function(isa, lowered, f.origin, listed)?;
            vun_trace::tracing::debug!(bytes = compiled.code.bytes.len(), "compiled");
            Ok(Outcome::Compiled(compiled))
        });
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
                    Named::Support(support) => done.import(unit, support),
                },
                kind: r.kind,
                addend: r.addend,
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

    /// One function's code.
    ///
    /// # Panics
    /// If Cranelift finds the function invalid, which it is only if lowering made it so.
    fn function(isa: &Isa, lowered: Lowered, at: OriginId, listed: bool) -> Result<Function, Failed> {
        let Lowered { func, origins } = lowered;
        let mut context = Context::for_function(func);
        context.set_disasm(listed);
        let clif = listed.then(|| context.func.display().to_string());
        if let Err(failure) = context.compile(&*isa.target, &mut ControlPlane::default()) {
            return match failure.inner {
                CodegenError::Verifier(errors) => {
                    panic!(
                        "the cranelift backend made an invalid function: {errors}\n{}",
                        failure.func.display()
                    )
                }
                error => Err(Failed::Refused { at, error }),
            };
        }
        let compiled = context.compiled_code().expect("a function compiled has code");
        let names = context.func.params.user_named_funcs();
        let relocs = compiled.buffer.relocs().iter().map(|r| {
            let target = match &r.target {
                FinalizedRelocTarget::ExternalName(ExternalName::User(name)) => {
                    Named::Address(names[*name].address())
                }
                FinalizedRelocTarget::ExternalName(ExternalName::LibCall(call)) => {
                    Named::Support(Support::of(*call).expect("the support Cranelift calls is listed"))
                }
                _ => panic!("the backend lowers only calls and addresses of the unit's, and thread storage"),
            };
            let kind = r.kind.kind().expect("the relocations Cranelift writes for these machines are listed");
            Reloc { offset: u64::from(r.offset), target, kind, addend: r.addend }
        });
        let relocs = relocs.collect();
        let marks = compiled
            .buffer
            .get_srclocs_sorted()
            .iter()
            .filter(|s| !s.loc.is_default())
            .map(|s| (u64::from(s.start), origins[s.loc.bits() as usize]))
            .collect();
        let text = clif.map(|clif| Listed {
            text: format!("{clif}\n{}", compiled.vcode.as_deref().unwrap_or_default()),
            origins: origins.clone(),
        });
        let frame = compiled.buffer.frame_layout().map_or(0, |layout| u64::from(layout.frame_to_fp_offset));
        Ok(Function {
            code: Code { bytes: compiled.code_buffer().to_vec(), relocs, listing: Vec::new() },
            frame,
            marks,
            text,
        })
    }
}
