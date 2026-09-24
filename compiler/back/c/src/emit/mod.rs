//! LIR to C: the integer types the target's C model gives, the C types of signatures and
//! records, the data, the functions, and the file holding them in an order C accepts (spec 13,
//! section 8.2).

mod data;
mod func;
mod ints;
mod signature;

use std::collections::BTreeSet;

use vun_backend::Lines;
use vun_intern::Symbol;
use vun_lir::{Address, DataId, FuncId, ImportKind, Linkage, Record, Unit};
use vun_target::TargetSpec;

use crate::ast::{
    Aggregate, Binary, Builtin, Decl, Expr, File, Ident, Item, Names, Own, Proto, Stmt, Storage, Ty, Unary,
};
use crate::lack::{Lack, Missing};

/// What emitting a unit gathers as it goes.
pub(crate) struct Emitter<'u> {
    unit: &'u Unit,
    target: &'u TargetSpec,
    lines: &'u dyn Lines,
    /// The records the C convention passes by value, each a `struct` of its fields' types.
    records: Vec<(Record, Vec<Ty>)>,
    /// The sizes copied as one `struct` of bytes.
    copies: BTreeSet<u64>,
    /// The float conversions used.
    helpers: BTreeSet<Own>,
    /// The compiler's builtins used.
    builtins: BTreeSet<Builtin>,
    checks: Vec<Item>,
}

impl<'u> Emitter<'u> {
    /// The C file of `unit` for `target`.
    pub(crate) fn file(unit: &'u Unit, target: &'u TargetSpec, lines: &'u dyn Lines) -> Result<File, Lack> {
        let mut e = Self {
            unit,
            target,
            lines,
            records: Vec::new(),
            copies: BTreeSet::new(),
            helpers: BTreeSet::new(),
            builtins: BTreeSet::new(),
            checks: Vec::new(),
        };
        let symbols = e.symbols()?;
        let ints = e.ints();
        let imports = unit.imports.iter().map(|(_, i)| e.import(i)).collect::<Result<Vec<_>, _>>()?;
        let mut protos = Vec::new();
        let mut bodies = Vec::new();
        for (id, func) in unit.functions.iter() {
            let storage = match func.linkage {
                Linkage::Export(_) => Storage::Public,
                Linkage::Internal => Storage::Static,
            };
            protos.push(Item::Declare(e.proto(storage, e.func(id), &func.signature, func.origin)?));
            bodies.push(e.define(id, storage)?);
        }
        let data = e.objects()?;
        // A link symbol spelling a builtin the file uses would hide it.
        if let Some(&clash) = symbols.iter().find(|s| e.builtins.iter().any(|b| b.spelling() == s.as_str())) {
            let at =
                unit.functions.iter().map(|(_, f)| f.origin).next().expect("builtins are used by functions");
            return Err(Missing::Symbol(clash).at(at));
        }
        let mut items = ints;
        items.extend(e.types());
        items.extend(data.types);
        items.append(&mut e.checks);
        items.extend(e.helpers());
        items.extend(imports);
        items.extend(protos);
        items.extend(data.objects);
        items.extend(bodies);
        Ok(File { names: Names::new(symbols), items })
    }

    /// The unit's link symbols, each checked writable in C.
    fn symbols(&self) -> Result<Vec<Symbol>, Lack> {
        let unit = self.unit;
        let linked = |linkage: Linkage, at| match linkage {
            Linkage::Export(s) => Some((s, at)),
            Linkage::Internal => None,
        };
        let functions = unit.functions.iter().filter_map(|(_, f)| linked(f.linkage, f.origin));
        let data = unit.data.iter().filter_map(|(_, d)| linked(d.linkage, d.origin));
        let imports = unit.imports.iter().map(|(_, i)| (i.symbol, i.origin));
        functions
            .chain(data)
            .chain(imports)
            .map(|(s, at)| if Names::writable(s) { Ok(s) } else { Err(Missing::Symbol(s).at(at)) })
            .collect()
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
                init: None,
            },
        })
    }

    fn func(&self, id: FuncId) -> Ident {
        match self.unit.functions[id].linkage {
            Linkage::Export(s) => Ident::Symbol(s),
            Linkage::Internal => Ident::func(id),
        }
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

    /// The float conversions used: each reads its parameter's bits as the other type, which
    /// `-fno-strict-aliasing` defines.
    fn helpers(&self) -> Vec<Item> {
        self.helpers
            .iter()
            .map(|&helper| {
                let (width, float_of) = match helper {
                    Own::FloatOf(width) => (width, true),
                    Own::BitsOf(width) => (width, false),
                    _ => unreachable!("only float conversions are helpers"),
                };
                let (int, float) = (Ty::uint(width), if width == 32 { Ty::Float } else { Ty::Double });
                let (from, to) = if float_of { (int, float) } else { (float, int) };
                let param = Ident::param(0);
                let read = Expr::unary(Unary::Address, Expr::Name(param)).at(to.clone());
                let proto = Proto {
                    storage: Storage::Static,
                    result: to,
                    name: Ident::Own(helper),
                    params: vec![Decl::new(from, param)],
                };
                Item::Define(proto, vec![Stmt::Return(Some(read))])
            })
            .collect()
    }
}
