//! One function's code: lowered to Cranelift IR and compiled, with its entry if it has one.

use cranelift_codegen::control::ControlPlane;
use cranelift_codegen::ir::ExternalName;
use cranelift_codegen::{CodegenError, Context, FinalizedRelocTarget};
use vun_asm::{Code, Reloc};
use vun_origin::OriginId;

use super::{Compilation, Function, Listed, Named, Support};
use crate::failed::Failed;
use crate::func::Lowered;
use crate::isa::Isa;
use crate::names::{Target, Targeted};
use crate::relocs::Placed;

impl Compilation {
    /// The code of a function lowered: its body, and its entry before it if it has one. `word` is
    /// the bytes of an address.
    pub(super) fn compiled(
        isa: &Isa,
        lowered: &Lowered,
        at: OriginId,
        listed: bool,
        word: u64,
    ) -> Result<Function, Failed> {
        let Lowered { func, entry, origins } = lowered.clone();
        let body = Self::function(isa, func, &origins, at, listed)?;
        Ok(match entry {
            Some(entry) => {
                let entry = Self::function(isa, entry, &origins, at, listed)?;
                body.entered(entry, isa.target.function_alignment().preferred, word)
            }
            None => body,
        })
    }

    /// One function's code.
    ///
    /// # Panics
    /// If Cranelift finds the function invalid, which it is only if lowering made it so.
    pub(super) fn function(
        isa: &Isa,
        func: cranelift_codegen::ir::Function,
        origins: &[OriginId],
        at: OriginId,
        listed: bool,
    ) -> Result<Function, Failed> {
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
                FinalizedRelocTarget::ExternalName(ExternalName::User(name)) => match names[*name].target() {
                    Target::Address(address) => Named::Address(address),
                    Target::Body(f) => Named::Body(f),
                },
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
            origins: origins.to_vec(),
        });
        let frame = compiled.buffer.frame_layout().map_or(0, |layout| u64::from(layout.frame_to_fp_offset));
        Ok(Function {
            body_at: None,
            code: Code { bytes: compiled.code_buffer().to_vec(), relocs, listing: Vec::new() },
            frame,
            marks,
            text,
        })
    }
}
