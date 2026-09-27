//! LIR to C: the integer types the target's C model gives, the C types of signatures and
//! records, the data, the functions, and the file holding them in an order C accepts (spec 13,
//! section 8.2).

mod data;
mod func;
mod ints;
mod signature;

use std::collections::BTreeSet;
use std::path::PathBuf;

use vun_backend::Lines;
use vun_hash::Hash;
use vun_intern::Symbol;
use vun_lir::{Address, DataId, FuncId, ImportKind, Linkage, Record, Unit};
use vun_origin::OriginId;
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
}

/// A file the backend writes: its name, whether other files include it, and its C.
pub(crate) struct Source {
    pub(crate) name: PathBuf,
    pub(crate) header: bool,
    pub(crate) file: File,
}

/// The header of a program spanning several files, which each of them includes.
pub(crate) const HEADER: &str = "program.h";
/// The file of a program in one file.
const PROGRAM: &str = "program.c";
/// The file defining the data of a program spanning several.
const DATA: &str = "data.c";

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
            protos.push(Item::Declare(e.proto(storage, e.func(id), &func.signature, func.origin)?));
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
        let names = Self::names(unit, shard);
        let file = |items| File { names: names.clone(), items };
        if !e.shared {
            common.extend(data.defined);
            common.extend(bodies.into_iter().map(|(_, body)| body));
            return Ok(vec![Source { name: PathBuf::from(PROGRAM), header: false, file: file(common) }]);
        }
        let include = || Item::Include(HEADER);
        let mut sources = vec![
            Source { name: PathBuf::from(HEADER), header: true, file: file(common) },
            Source {
                name: PathBuf::from(DATA),
                header: false,
                file: file([include()].into_iter().chain(data.defined).collect()),
            },
        ];
        let mut shard_items: Vec<Vec<Item>> = (0..shards).map(|_| vec![include()]).collect();
        for (at, body) in bodies {
            shard_items[at].push(body);
        }
        for (i, items) in shard_items.into_iter().enumerate() {
            sources.push(Source {
                name: PathBuf::from(format!("shard{i}.c")),
                header: false,
                file: file(items),
            });
        }
        Ok(sources)
    }

    /// How many shards the functions of `unit` fill, `shard` in each.
    fn shards(unit: &Unit, shard: usize) -> usize {
        unit.functions.len().div_ceil(shard.max(1)).max(1)
    }

    /// The spellings of the names of `unit`'s C, tagged when it spans several files.
    pub(crate) fn names(unit: &Unit, shard: usize) -> Names {
        #[expect(clippy::cast_possible_truncation, reason = "a tag keeps the hash's low bits")]
        let tag = (Self::shards(unit, shard) > 1).then(|| Hash::of_value(unit).short() as u32);
        Names::new(Self::linked(unit).map(|(s, _)| s), tag)
    }

    /// The link symbols of `unit`, exported or imported, with where each is written.
    fn linked(unit: &Unit) -> impl Iterator<Item = (Symbol, OriginId)> {
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
    fn storage(&self, linkage: Linkage) -> Storage {
        match linkage {
            Linkage::Export(_) => Storage::Public,
            Linkage::Internal if self.shared => Storage::Public,
            Linkage::Internal => Storage::Static,
        }
    }

    /// Refuses a link symbol C cannot write, where it is declared.
    fn writable(&self) -> Result<(), Lack> {
        Self::linked(self.unit)
            .try_for_each(|(s, at)| if Names::writable(s) { Ok(()) } else { Err(Missing::Symbol(s).at(at)) })
    }

    fn import(&mut self, import: &vun_lir::Import) -> Result<Item, Lack> {
        let name = Ident::Symbol(import.symbol);
        Ok(match &import.kind {
            ImportKind::Func(signature) => {
                Item::Declare(self.proto(Storage::Extern, name, signature, import.origin)?)
            }
            ImportKind::Data => Item::Object {
                storage: Storage::Extern,
                constant: false,
                decl: Decl::new(Ty::BYTE, name),
                attributes: Vec::new(),
                init: None,
            },
        })
    }

    fn func(&self, id: FuncId) -> Ident {
        Self::func_ident(self.unit, id)
    }

    fn data_name(&self, id: DataId) -> Ident {
        match self.unit.data[id].linkage {
            Linkage::Export(s) => Ident::Symbol(s),
            Linkage::Internal => Ident::data(id),
        }
    }

    /// `&name` of something in the program, and whether it is code.
    fn reference(&self, address: Address) -> (Expr, bool) {
        let (name, code) = match address {
            Address::Func(f) => (self.func(f), true),
            Address::Data(d) => (self.data_name(d), false),
            Address::Import(i) => {
                let import = &self.unit.imports[i];
                (Ident::Symbol(import.symbol), matches!(import.kind, ImportKind::Func(_)))
            }
        };
        (Expr::unary(Unary::Address, Expr::Name(name)), code)
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
