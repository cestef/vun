//! What the program keeps: what its exports, its entry and the symbols objects keep reach through
//! relocations, as `wasm-ld --gc-sections` does; a function or segment nothing reaches is left
//! out, and so is an import nothing calls.

use vun_hash::Set;
use vun_wasm::{Object, Reloc, RelocKind};

use crate::provided::Provided;
use crate::resolve::{Func, Import, Resolved, Seg, Symbols};

#[derive(Default)]
pub(crate) struct Live<'o> {
    pub(crate) funcs: Set<Func>,
    pub(crate) segs: Set<Seg>,
    pub(crate) imports: Set<Import<'o>>,
    /// The types of the undefined weak functions called, as their objects' types.
    pub(crate) missing: Set<(usize, u32)>,
    /// Whether the function calling the initialization functions is.
    pub(crate) ctors: bool,
}

impl<'o> Live<'o> {
    /// What `roots` reach in `symbols`.
    pub(crate) fn of(symbols: &Symbols<'o>, roots: impl IntoIterator<Item = Resolved<'o>>) -> Self {
        let mut live = Self::default();
        let mut work: Vec<Resolved<'o>> = roots.into_iter().collect();
        while let Some(reached) = work.pop() {
            match reached {
                Resolved::Func(func) if live.funcs.insert(func) => {
                    let object = &symbols.objects[func.object];
                    let body = object.code.bodies[func.index as usize].clone();
                    work.extend(
                        Self::within(&object.code.relocs, body)
                            .map(|r| symbols.resolved[func.object][r.index as usize]),
                    );
                }
                Resolved::Data(seg, _) if live.segs.insert(seg) => {
                    let object = &symbols.objects[seg.object];
                    let bytes = object.data.segments[seg.index as usize].bytes.clone();
                    work.extend(
                        Self::within(&object.data.relocs, bytes)
                            .map(|r| symbols.resolved[seg.object][r.index as usize]),
                    );
                }
                Resolved::Import(import) => {
                    live.imports.insert(import);
                }
                Resolved::Missing { object, ty } => {
                    live.missing.insert((object, ty));
                }
                Resolved::Provided(Provided::CallCtors) if !live.ctors => {
                    live.ctors = true;
                    work.extend(Self::inits(symbols).map(|(_, resolved)| resolved));
                }
                _ => {}
            }
        }
        live
    }

    /// The relocations of symbols in `range` of a payload, whose relocations are by offset.
    fn within(relocs: &[Reloc], range: std::ops::Range<usize>) -> impl Iterator<Item = &Reloc> {
        let from = relocs.partition_point(|r| r.offset < range.start);
        let to = relocs.partition_point(|r| r.offset < range.end);
        relocs[from..to].iter().filter(|r| r.kind != RelocKind::TypeIndexLeb)
    }

    /// Every initialization function, with its priority, in the order they are called: by
    /// priority, then as the objects list them.
    pub(crate) fn inits(symbols: &Symbols<'o>) -> impl Iterator<Item = (u32, Resolved<'o>)> {
        let mut inits: Vec<(u32, Resolved<'o>)> = symbols
            .objects
            .iter()
            .enumerate()
            .flat_map(|(o, object): (usize, &Object)| {
                object
                    .inits
                    .iter()
                    .map(move |init| (init.priority, symbols.resolved[o][init.symbol as usize]))
            })
            .filter(|(_, resolved)| matches!(resolved, Resolved::Func(_)))
            .collect();
        inits.sort_by_key(|&(priority, _)| priority);
        inits.dedup();
        inits.into_iter()
    }
}
