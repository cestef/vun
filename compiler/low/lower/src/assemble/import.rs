//! The imports of a program: the symbols the linker provides, made once per symbol.

use vun_lir::{Conv, Import, ImportId, ImportKind};
use vun_origin::OriginId;
use vun_primitives::abi::Abi;
use vun_sema::callable::Callable;
use vun_sema::link::Linked;
use vun_sema_ty::item::DeclRef;

use super::Assembly;
use crate::unlowered::{Form, Unlowered};

impl Assembly<'_, '_, '_> {
    pub(super) fn import(
        &mut self,
        decl: &DeclRef,
        linked: Linked,
        at: OriginId,
    ) -> Result<ImportId, Unlowered> {
        let Linked { symbol, abi, library } = linked;
        if let Some(&id) = self.imports.get(&symbol) {
            return Ok(id);
        }
        let ty = self
            .l
            .types
            .cx
            .get::<vun_sema::decl::DeclType>(decl)
            .map_err(|_| Unlowered::unsupported(at, Form::Callee))?
            .ty;
        let kind = match ty.signature() {
            Some(signature) => ImportKind::Func(self.l.signature_of(&signature, abi, at)?),
            None => self.generic_import(decl, abi, at)?,
        };
        // The largest bound the program's declarations of the symbol state, the handler of
        // traps included, which a program declares itself (decision 164).
        let stack_max = self.l.env.linked_stack(symbol);
        let id =
            self.unit.imports.push(Import { symbol, library, kind, origin: decl.decl.origin, stack_max });
        self.imports.insert(symbol, id);
        Ok(id)
    }

    /// What a declaration with no signature of its own is imported as: the C function a link symbol
    /// with `$` parameters ending in a pack declares, its fixed parameters and then variadic ones,
    /// each call passing its own through the symbol's address (decision 686); data otherwise.
    fn generic_import(
        &self,
        decl: &DeclRef,
        abi: Option<Abi>,
        at: OriginId,
    ) -> Result<ImportKind, Unlowered> {
        let env = &self.l.env;
        let callable = Callable::Linked(decl.clone());
        if callable.generic(env).is_none() {
            return Ok(ImportKind::Data);
        }
        let unsupported = || Unlowered::unsupported(at, Form::Import);
        let fixed = callable.fixed(env).ok_or_else(unsupported)?;
        let mut signature = self.l.signature(&fixed.params, fixed.result, abi, at)?;
        if signature.conv != Conv::C {
            return Err(unsupported());
        }
        signature.variadic = Some(signature.params.len());
        Ok(ImportKind::Func(signature))
    }
}
