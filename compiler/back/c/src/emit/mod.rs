//! LIR to C: the integer types the target's C model gives, the C types of signatures and
//! records, the data, the functions, and the file holding them in an order C accepts (spec 13,
//! section 8.2).

mod data;
mod func;
mod import;
mod ints;
mod names;
mod signature;

use std::collections::BTreeSet;

use vun_backend::{Emits, Lines, Part};
use vun_hash::Hash;
use vun_intern::Symbol;
use vun_lir::{Address, Record, Unit};
use vun_target::TargetSpec;

use crate::ast::{
    Aggregate, Binary, Builtin, Decl, Expr, File, Helper, Ident, Item, Names, Own, Proto, Stmt, Storage, Ty,
    Unary,
};
use vun_backend::{Lack, Missing};

/// What emitting a unit gathers as it goes.
pub(crate) struct Emitter<'u> {
    unit: &'u Unit,
    target: &'u TargetSpec,
    lines: &'u dyn Lines,
    /// Whether the program spans several files.
    shared: bool,
    /// The records the C convention passes by value, each a `struct` of its fields' types.
    records: Vec<(Record, Vec<Ty>)>,
    /// The sizes copied as one `struct` of bytes.
    copies: BTreeSet<u64>,
    /// The float conversions used.
    helpers: BTreeSet<Helper>,
    /// The compiler's builtins used.
    builtins: BTreeSet<Builtin>,
    checks: Vec<Item>,
    /// How names are spelled, as a block of inline assembly writes them too.
    names: Names,
    /// What the blocks of inline assembly name: kept though no C reads them.
    named: BTreeSet<Address>,
}

/// A file the backend writes: the part of the program it holds, and its C. The shared part of a
/// program spanning several files is a header, which each of the others includes.
pub(crate) struct Source {
    pub(crate) part: Part,
    pub(crate) file: File,
}

impl Source {
    pub(crate) fn kind(&self) -> Emits {
        if self.part == Part::Shared { Emits::Header } else { Emits::Source }
    }
}

impl<'u> Emitter<'u> {
    /// The C of `unit` for `target`: one file, or, past `shard` functions, a header, the data and
    /// shards of functions, each function in the shard its content hashes to.
    pub(crate) fn sources(
        unit: &'u Unit,
        target: &'u TargetSpec,
        lines: &'u dyn Lines,
        shard: usize,
    ) -> Result<Vec<Source>, Lack> {
        let shards = Self::shards(unit, shard);
        let mut e = Self {
            unit,
            target,
            lines,
            shared: shards > 1,
            records: Vec::new(),
            copies: BTreeSet::new(),
            helpers: BTreeSet::new(),
            builtins: BTreeSet::new(),
            checks: Vec::new(),
            names: Self::names(unit, shard),
            named: Self::named(unit),
        };
        e.writable()?;
        if let Some(at) = unit.functions.iter().map(|(_, f)| f.origin).next() {
            e.int(unit.pointer_bits, false, at)?;
        }
        let ints = e.ints();
        let imports = unit.imports.iter().map(|(_, i)| e.import(i)).collect::<Result<Vec<_>, _>>()?;
        let mut protos = Vec::new();
        let mut bodies = Vec::new();
        for (id, func) in unit.functions.iter() {
            let storage = e.storage(func.linkage);
            let proto = e.proto(storage, e.func(id), &func.signature, func.origin)?;
            protos.push(Item::Declare(proto, e.func_attributes(id)));
            #[expect(clippy::cast_possible_truncation, reason = "the remainder is below the shard count")]
            let at = (Hash::of_value(func).short() % shards as u64) as usize;
            bodies.push((at, e.define(id, storage)?));
            vun_trace::tracing::debug!(f = %func.linkage, shard = at, "emitted");
        }
        let data = e.objects()?;
        // A link symbol spelling a builtin the file uses would hide it.
        let builtin = |s: &Symbol| e.builtins.iter().any(|b| b.spelling() == s.as_str());
        if let Some((clash, at)) = Self::linked(unit).find(|(s, _)| builtin(s)) {
            return Err(Missing::Symbol(clash).at(at));
        }
        let mut common = ints;
        common.extend(e.types());
        common.extend(data.types);
        common.append(&mut e.checks);
        common.extend(e.helpers());
        common.extend(imports);
        common.extend(protos);
        common.extend(data.declared);
        let file = |items| File { names: e.names.clone(), items };
        if !e.shared {
            common.extend(data.defined);
            common.extend(bodies.into_iter().map(|(_, body)| body));
            return Ok(vec![Source { part: Part::Whole, file: file(common) }]);
        }
        let include = || Item::Include(Part::Shared);
        let mut sources = vec![
            Source { part: Part::Shared, file: file(common) },
            Source { part: Part::Data, file: file([include()].into_iter().chain(data.defined).collect()) },
        ];
        let mut shard_items: Vec<Vec<Item>> = (0..shards).map(|_| vec![include()]).collect();
        for (at, body) in bodies {
            shard_items[at].push(body);
        }
        for (i, items) in (0..).zip(shard_items) {
            sources.push(Source { part: Part::Shard(i), file: file(items) });
        }
        Ok(sources)
    }

    /// How many shards the functions of `unit` fill, `shard` in each.
    fn shards(unit: &Unit, shard: usize) -> usize {
        unit.functions.len().div_ceil(shard.max(1)).max(1)
    }

    /// The address of something in the program, as a `u8 *`; a function's goes through the
    /// integer as wide as an address, since C converts function pointers only to integers.
    fn address(&self, address: Address) -> Expr {
        match self.reference(address) {
            (at, true) => at.cast(self.word()).cast(Ty::address()),
            (at, false) => at.cast(Ty::address()),
        }
    }

    /// Adds a check C makes while compiling: that `holds` is not 0.
    fn check(&mut self, holds: Expr) {
        let name = Ident::Own(Own::Check(u32::try_from(self.checks.len()).expect("checks fit u32")));
        self.checks.push(Item::Check { name, holds });
    }

    /// Checks that `sizeof(ty)` is `bytes`.
    fn check_size(&mut self, ty: Ty, bytes: u64) {
        self.check(Expr::binary(Binary::Eq, Expr::Sizeof(ty), Expr::small(bytes)));
    }

    /// The `struct`s of records and of copied bytes. A record's fields are placed as C places
    /// them; its size checks that C placed them as the record says.
    fn types(&mut self) -> Vec<Item> {
        let mut items = Vec::new();
        for (i, (record, types)) in self.records.clone().into_iter().enumerate() {
            let fields = types.into_iter().enumerate().map(|(j, ty)| Decl::new(ty, Ident::field(j)));
            let tag = Ident::record(i);
            items.push(Item::Aggregate { kind: Aggregate::Struct, tag, fields: fields.collect() });
            self.check_size(Ty::Struct(tag), record.size);
        }
        for &size in &self.copies {
            let bytes = Decl { ty: Ty::BYTE, name: Ident::field(0), len: Some(size) };
            let tag = Ident::Own(Own::Copy(size));
            items.push(Item::Aggregate { kind: Aggregate::Struct, tag, fields: vec![bytes] });
        }
        items
    }

    /// A call of the float conversion `helper`, defined once it is used.
    pub(super) fn helper(&mut self, helper: Helper, arg: Expr) -> Expr {
        self.helpers.insert(helper);
        Expr::Call(Box::new(Expr::Name(Ident::Own(Own::Helper(helper)))), vec![arg])
    }

    /// The float conversions used: each reads its parameter's bits as the other type, which
    /// `-fno-strict-aliasing` defines.
    fn helpers(&self) -> Vec<Item> {
        self.helpers
            .iter()
            .map(|&helper| {
                let (width, float_of) = match helper {
                    Helper::FloatOf(width) => (width, true),
                    Helper::BitsOf(width) => (width, false),
                };
                let (int, float) = (Ty::uint(width), if width == 32 { Ty::Float } else { Ty::Double });
                let (from, to) = if float_of { (int, float) } else { (float, int) };
                let param = Ident::param(0);
                let read = Expr::unary(Unary::Address, Expr::Name(param)).at(to.clone());
                let proto = Proto {
                    storage: Storage::Static,
                    result: to,
                    name: Ident::Own(Own::Helper(helper)),
                    params: vec![Decl::new(from, param)],
                };
                Item::Define(proto, vec![Stmt::Return(Some(read))])
            })
            .collect()
    }
}
