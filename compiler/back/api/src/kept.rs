//! Machine code kept by what it is compiled from (spec 13, section 2; decision 669). A function
//! is compiled in its own numbering of what it names, so that its code is the same whatever else
//! the unit holds, and kept under the hash of that function and of what compiling it reads of
//! the unit: the signature each name is called with and the width of pointers.

use vun_asm::Reloc;
use vun_hash::Map;
use vun_index::Idx;
use vun_lir::unit::{ImportKind, Unit};
use vun_lir::{Address, DataId, FuncId, Function, ImportId};
use vun_query::Fingerprint;

use crate::Compiled;

/// Where machine code is kept between builds, its relocations in each function's own numbering.
pub trait Kept: Sync {
    fn get(&self, key: Fingerprint) -> Option<Compiled<()>>;

    fn put(&self, key: Fingerprint, code: Compiled<()>);
}

/// The addresses of the unit a function names, numbered in the order first met.
struct Names {
    unit: Vec<Address>,
    local: Map<Address, Address>,
}

impl Names {
    /// The names of `f`, and `f` naming them by their own numbers.
    fn of(f: &Function) -> (Self, Function) {
        let mut names = Self { unit: Vec::new(), local: Map::default() };
        let mut local = f.clone();
        local.map_addresses(|address| names.number(address));
        (names, local)
    }

    fn number(&mut self, address: Address) -> Address {
        let n = self.unit.len();
        *self.local.entry(address).or_insert_with(|| {
            self.unit.push(address);
            match address {
                Address::Func(_) => Address::Func(FuncId::new(n)),
                Address::Data(_) => Address::Data(DataId::new(n)),
                Address::Import(_) => Address::Import(ImportId::new(n)),
            }
        })
    }

    /// The address of the unit numbered `local`.
    fn unit(&self, local: Address) -> Address {
        let n = match local {
            Address::Func(id) => id.index(),
            Address::Data(id) => id.index(),
            Address::Import(id) => id.index(),
        };
        self.unit[n]
    }

    fn relocs(relocs: &[Reloc<Address>], to: impl Fn(Address) -> Address) -> Vec<Reloc<Address>> {
        relocs.iter().map(|&r| Reloc { target: to(r.target), ..r }).collect()
    }
}

impl dyn Kept + '_ {
    /// The code of `f`, a function of `unit`, as kept under `context` and the content `f` is
    /// compiled from, or as `compile` makes it, then kept. `context` names what compiling reads
    /// besides the unit: the target and its processor.
    ///
    /// # Errors
    /// Returns what `compile` returns; what fails is not kept.
    pub fn code<T, E>(
        &self,
        context: Fingerprint,
        unit: &Unit,
        f: &Function,
        compile: impl FnOnce() -> Result<Compiled<T>, E>,
    ) -> Result<Compiled<T>, E> {
        let (names, local) = Names::of(f);
        let read: Vec<_> = (names.unit.iter())
            .map(|&address| match address {
                Address::Func(g) => Some(&unit.functions[g].signature),
                Address::Import(i) => match &unit.imports[i].kind {
                    ImportKind::Func(signature) => Some(signature),
                    ImportKind::Data => None,
                },
                Address::Data(_) => None,
            })
            .collect();
        let key = Fingerprint::of(&(context, unit.pointer_bits, &local, read));
        if let Some(code) = self.get(key) {
            let relocs = Names::relocs(&code.code.relocs, |a| names.unit(a));
            let code = Compiled { code: vun_asm::Code { relocs, ..code.code }, ..code };
            return Ok(code.unlisted());
        }
        let compiled = compile()?;
        let code = vun_asm::Code {
            bytes: compiled.code.bytes.clone(),
            relocs: Names::relocs(&compiled.code.relocs, |a| names.local[&a]),
            listing: Vec::new(),
        };
        let unwind = compiled.unwind.clone();
        self.put(key, Compiled { code, frame: compiled.frame, marks: compiled.marks.clone(), unwind });
        Ok(compiled)
    }
}
