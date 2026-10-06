//! Checking the block typing found against the tables of the architectures (spec 07, section 6):
//! the architecture whose table holds its instructions, the class of register each value it binds
//! needs, and the block against the table.

use std::collections::BTreeMap;
use std::sync::Arc;

use vun_diag::{DiagCtxt, Diagnostic, Msg, errors};
use vun_hir::tree::ExprId;
use vun_sema::asm::{Assembled, Assembler, Request};
use vun_sema::env::Env;
use vun_sema::layout::Shapes;
use vun_sema::target::TargetOf;
use vun_sema_ty::item::Site;

use crate::{Arch, ArchTable, Checker, Held, Table};

/// The assembler the driver installs.
pub struct Assembly;

impl Assembler for Assembly {
    fn assemble(&self, env: &Env<'_, '_>, request: &Request, diag: &mut DiagCtxt) -> Assembled {
        let Some((arch, table)) = Self::arch_of(env, &request.instructions, diag).filter(|_| request.placed)
        else {
            return Assembled::Skipped;
        };
        // A value's width is known only where the target's code is the block's: elsewhere the
        // block is checked for its class of register alone.
        let targeted = env.cx.get::<TargetOf>(&()).is_some_and(|t| t.isa == arch.namespace().symbol());
        let mut bound = BTreeMap::new();
        for (&value, &ty) in &request.bound {
            let bytes = env.cx.get::<Shapes>(&ty).map(|s| s.layout.size).filter(|_| targeted);
            bound.insert(value, arch.register(&table, bytes, Held::of(ty)));
        }
        let checked =
            Checker { env, arch, table: &table, forms: &request.forms, bound: &bound, targeted, diag }
                .block(request.block);
        checked.map_or(Assembled::Rejected, Assembled::Checked)
    }
}

impl Assembly {
    /// The architecture whose table holds the instructions `callees` name, and the table; `None`
    /// when they are of none, or of several, which is reported.
    fn arch_of(env: &Env<'_, '_>, callees: &[ExprId], diag: &mut DiagCtxt) -> Option<(Arch, Arc<Table>)> {
        let package = env.file.package;
        let tables: Vec<_> = Arch::ALL
            .iter()
            .filter_map(|&arch| Some((arch, env.cx.get::<ArchTable>(&(package, arch)).ok()?)))
            .collect();
        let mut found: Option<usize> = None;
        for &callee in callees {
            let Ok(Site::Decl(decl)) = env.resolve(callee) else { continue };
            let Some(at) = tables.iter().position(|(_, t)| t.callables.contains_key(&decl)) else { continue };
            if found.is_some_and(|f| f != at) {
                let msg =
                    Msg::new().text("this instruction is of another architecture than the block's first");
                let origin = env.parsed.hir.exprs[callee].origin;
                diag.error(Diagnostic::error(errors::ASM_UNKNOWN_ARCH, msg).at(origin, None));
                return None;
            }
            found = Some(at);
        }
        found.map(|at| tables[at].clone())
    }
}
