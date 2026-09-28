//! What the C of a unit calls things: the spellings of names, who sees each definition, the
//! symbols the linker sees, and what GNU C is told of what only inline assembly names.

use std::collections::BTreeSet;

use vun_backend::{Lack, Missing};
use vun_hash::Hash;
use vun_inline::Read;
use vun_intern::Symbol;
use vun_lir::{Address, Conv, DataId, FuncId, ImportKind, InstKind, Linkage, Unit};
use vun_origin::OriginId;

use super::Emitter;
use crate::ast::{Attribute, Expr, Ident, Names, Storage, Unary};

impl Emitter<'_> {
    /// The spellings of the names of `unit`'s C, tagged when it spans several files.
    pub(crate) fn names(unit: &Unit, shard: usize) -> Names {
        #[expect(clippy::cast_possible_truncation, reason = "a tag keeps the hash's low bits")]
        let tag = (Self::shards(unit, shard) > 1).then(|| Hash::of_value(unit).short() as u32);
        Names::new(Self::linked(unit).map(|(s, _)| s), tag)
    }

    /// The link symbols of `unit`, exported or imported, with where each is written.
    pub(super) fn linked(unit: &Unit) -> impl Iterator<Item = (Symbol, OriginId)> {
        let exported = |linkage: Linkage, at| match linkage {
            Linkage::Export(s) => Some((s, at)),
            Linkage::Internal => None,
        };
        let functions = unit.functions.iter().filter_map(move |(_, f)| exported(f.linkage, f.origin));
        let data = unit.data.iter().filter_map(move |(_, d)| exported(d.linkage, d.origin));
        functions.chain(data).chain(unit.imports.iter().map(|(_, i)| (i.symbol, i.origin)))
    }

    /// The name of function `id` of `unit`.
    pub(crate) fn func_ident(unit: &Unit, id: FuncId) -> Ident {
        match unit.functions[id].linkage {
            Linkage::Export(s) => Ident::Symbol(s),
            Linkage::Internal => Ident::func(id),
        }
    }

    /// Who sees a definition of the unit: exports the linker; the rest this file, or, in a
    /// program spanning several, each of them, under tagged names.
    pub(super) fn storage(&self, linkage: Linkage) -> Storage {
        match linkage {
            Linkage::Export(_) => Storage::Public,
            Linkage::Internal if self.shared => Storage::Public,
            Linkage::Internal => Storage::Static,
        }
    }

    /// Refuses a link symbol C cannot write, where it is declared.
    pub(super) fn writable(&self) -> Result<(), Lack> {
        Self::linked(self.unit)
            .try_for_each(|(s, at)| if Names::writable(s) { Ok(()) } else { Err(Missing::Symbol(s).at(at)) })
    }

    pub(super) fn func(&self, id: FuncId) -> Ident {
        Self::func_ident(self.unit, id)
    }

    pub(super) fn data_name(&self, id: DataId) -> Ident {
        match self.unit.data[id].linkage {
            Linkage::Export(s) => Ident::Symbol(s),
            Linkage::Internal => Ident::data(id),
        }
    }

    /// The name of something in the program.
    pub(super) fn ident(&self, address: Address) -> Ident {
        match address {
            Address::Func(f) => self.func(f),
            Address::Data(d) => self.data_name(d),
            Address::Import(i) => Ident::Symbol(self.unit.imports[i].symbol),
        }
    }

    /// `&name` of something in the program, and whether it is code.
    pub(super) fn reference(&self, address: Address) -> (Expr, bool) {
        let code = match address {
            Address::Func(_) => true,
            Address::Data(_) => false,
            Address::Import(i) => matches!(self.unit.imports[i].kind, ImportKind::Func(_)),
        };
        (Expr::unary(Unary::Address, Expr::Name(self.ident(address))), code)
    }

    /// What the unit's blocks of inline assembly name.
    pub(super) fn named(unit: &Unit) -> BTreeSet<Address> {
        let kinds = unit.functions.iter().flat_map(|(_, f)| f.insts.iter().map(|(_, i)| &i.kind));
        let blocks = kinds.filter_map(|kind| match kind {
            InstKind::Asm(asm, _) => Some(asm),
            _ => None,
        });
        let symbols = blocks.flat_map(|asm| {
            asm.reads().filter_map(|read| match read {
                Read::Symbol(&address) => Some(address),
                Read::Value(_) => None,
            })
        });
        symbols.collect()
    }

    /// What GNU C says of function `id` beyond C99: that it is naked, and kept where a block of
    /// assembly names it.
    pub(super) fn func_attributes(&self, id: FuncId) -> Vec<Attribute> {
        let naked = self.unit.functions[id].signature.conv == Conv::Naked;
        let used = self.named.contains(&Address::Func(id));
        [(naked, Attribute::Naked), (used, Attribute::Used)]
            .into_iter()
            .filter_map(|(on, a)| on.then_some(a))
            .collect()
    }
}
