//! Data objects. Each is a `union` of a `struct` of its bytes, as runs of `u8` between the
//! addresses its relocations write, and of the integer giving its alignment. An address sits at
//! an offset the target aligns addresses to, so C places every field where the bytes say; the
//! size of every `struct` is checked. Objects are defined so that what one points to comes
//! first; objects pointing to each other in a cycle are declared before, as C99 allows for
//! exactly that (6.9.2).

use std::collections::BTreeSet;

use vun_index::IndexVec;
use vun_lir::{Address, DataId};

use super::Emitter;
use crate::ast::{Aggregate, Binary, Decl, Expr, Ident, Item, Storage, Ty};
use crate::lack::{Lack, Missing};

/// The items of the unit's data.
pub(super) struct Objects {
    pub(super) types: Vec<Item>,
    /// The declarations definitions need first: those cycles need, or, in a program spanning
    /// several files, each object's, for the files using it.
    pub(super) declared: Vec<Item>,
    /// Every definition, in order.
    pub(super) defined: Vec<Item>,
}

/// Where the walk ordering data objects is at one.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Visit {
    New,
    Open,
    Done,
}

impl Emitter<'_> {
    pub(super) fn objects(&mut self) -> Result<Objects, Lack> {
        let (order, cyclic) = self.order();
        let mut types = Vec::new();
        let mut declared = Vec::new();
        let mut defined = Vec::new();
        for id in order {
            let (aggregates, decl, init) = self.object(id)?;
            types.extend(aggregates);
            let data = &self.unit.data[id];
            let storage = self.storage(data.linkage);
            let object =
                |storage, init| Item::Object { storage, constant: !data.writable, decl: decl.clone(), init };
            if self.shared {
                declared.push(object(Storage::Extern, None));
            } else if cyclic.contains(&id) {
                declared.push(object(storage, None));
            }
            defined.push(object(storage, Some(init)));
        }
        Ok(Objects { types, declared, defined })
    }

    /// The data objects, each after those it points to, and those a cycle reaches before their
    /// definition.
    fn order(&self) -> (Vec<DataId>, BTreeSet<DataId>) {
        let data = &self.unit.data;
        let mut visit: IndexVec<DataId, Visit> = IndexVec::filled(Visit::New, data.len());
        let (mut order, mut cyclic) = (Vec::new(), BTreeSet::new());
        for (root, _) in data.iter() {
            let mut stack = vec![(root, 0)];
            while let Some(&(id, next)) = stack.last() {
                if next == 0 {
                    if visit[id] != Visit::New {
                        stack.pop();
                        continue;
                    }
                    visit[id] = Visit::Open;
                }
                let target = data[id].relocs.get(next).map(|r| r.target);
                let Some(target) = target else {
                    visit[id] = Visit::Done;
                    order.push(id);
                    stack.pop();
                    continue;
                };
                if let Some(top) = stack.last_mut() {
                    top.1 += 1;
                }
                if let Address::Data(to) = target {
                    match visit[to] {
                        Visit::New => stack.push((to, 0)),
                        Visit::Open => {
                            cyclic.insert(to);
                        }
                        Visit::Done => {}
                    }
                }
            }
        }
        (order, cyclic)
    }

    /// The types of object `id`, its declaration and its initializer.
    fn object(&mut self, id: DataId) -> Result<(Vec<Item>, Decl, Expr), Lack> {
        let unit = self.unit;
        let data = &unit.data[id];
        let at = data.origin;
        if data.section.is_some() {
            return Err(Missing::Section.at(at));
        }
        let (align, _) = self.aligned(data.align, at)?;
        let word = u64::from(unit.pointer_bits / u8::BITS);
        let pointer_align = u64::from(self.target.c.pointer_align);
        let mut relocs = data.relocs.clone();
        relocs.sort_by_key(|r| r.offset);
        let (mut fields, mut inits) = (Vec::new(), Vec::new());
        let mut field = |ty, len, init| {
            fields.push(Decl { ty, name: Ident::field(fields.len()), len });
            inits.push(init);
        };
        let run = |from: u64, to: u64| {
            let bytes = &data.bytes[usize::try_from(from).expect("fits")..usize::try_from(to).expect("fits")];
            Expr::Init(bytes.iter().map(|&b| Expr::Small(i64::from(b))).collect())
        };
        let mut offset = 0;
        let mut any = false;
        for reloc in &relocs {
            if reloc.offset % pointer_align != 0 {
                return Err(Missing::Unaligned(reloc.offset).at(at));
            }
            if reloc.offset > offset {
                field(Ty::BYTE, Some(reloc.offset - offset), run(offset, reloc.offset));
            }
            let (ty, init) = self.pointer(reloc.target, reloc.addend).map_err(|m| m.at(at))?;
            field(ty, None, init);
            any = true;
            offset = reloc.offset + word;
        }
        let end = data.bytes.len() as u64;
        if end > offset || !any {
            let init = if end > offset { run(offset, end) } else { Expr::Init(vec![Expr::Small(0)]) };
            field(Ty::BYTE, Some((end - offset).max(1)), init);
        }
        let (parts, whole) = (Ident::parts(id), Ident::data(id));
        let size = fields.iter().map(|f| f.len.unwrap_or(word)).sum::<u64>();
        self.check_size(
            Ty::Struct(parts),
            size.next_multiple_of(if relocs.is_empty() { 1 } else { pointer_align }),
        );
        let members = vec![Decl::new(Ty::Struct(parts), Ident::field(0)), Decl::new(align, Ident::field(1))];
        let types = vec![
            Item::Aggregate { kind: Aggregate::Struct, tag: parts, fields },
            Item::Aggregate { kind: Aggregate::Union, tag: whole, fields: members },
        ];
        let decl = Decl::new(Ty::Union(whole), self.data_name(id));
        Ok((types, decl, Expr::Init(vec![Expr::Init(inits)])))
    }

    /// The field holding an address in data, and its initializer: a function's as `void
    /// (*)(void)`, anything else's as `u8 *`.
    fn pointer(&self, target: Address, addend: i64) -> Result<(Ty, Expr), Missing> {
        let (reference, code) = self.reference(target);
        if code {
            if addend != 0 {
                return Err(Missing::FunctionOffset);
            }
            return Ok((Ty::code(), reference.cast(Ty::code())));
        }
        let at = reference.cast(Ty::address());
        let at = if addend == 0 { at } else { Expr::binary(Binary::Add, at, Expr::Small(addend)) };
        Ok((Ty::address(), at))
    }
}
