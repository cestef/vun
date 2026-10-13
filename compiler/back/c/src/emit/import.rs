//! Declarations of what the linker provides: functions and data another object defines.

use vun_backend::Lack;
use vun_lir::{Import, ImportKind};
use vun_target::Name;

use super::Emitter;
use crate::ast::{Attribute, Decl, Duration, Ident, Item, Storage, Ty};

/// The object format whose imports name the module they come from.
pub(super) const WASM: Name = Name("wasm");

impl Emitter<'_> {
    pub(super) fn import(&mut self, import: &Import) -> Result<Item, Lack> {
        let name = Ident::Symbol(import.symbol);
        Ok(match &import.kind {
            ImportKind::Func(signature) => {
                // A wasm object imports it from its library's module; elsewhere the linker is
                // given the library.
                let module = import.library.filter(|_| WASM.is(self.target.object_format));
                let from = module.map(|module| Attribute::Import { module, name: import.symbol });
                let attributes = from.into_iter().collect();
                Item::Declare(self.proto(Storage::Extern, name, signature, import.origin)?, attributes)
            }
            ImportKind::Data => Item::Object {
                storage: Storage::Extern,
                duration: Duration::Static,
                constant: false,
                decl: Decl::new(Ty::BYTE, name),
                attributes: Vec::new(),
                init: None,
            },
        })
    }
}
