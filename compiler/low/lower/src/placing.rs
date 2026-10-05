//! Placing a function's ids in tables holding more than its own: the program's unit, when the
//! program is assembled (`assemble`), or another function's tables, when that function inlines it
//! (`inlined`). What each table does with a function, a link symbol, a data object or a trap site
//! is its own; which of the function's ids go where is found once, here.

use vun_hash::Map;
use vun_index::Idx;
use vun_lir::{Address, DataId, Function, InstKind, Site};
use vun_origin::OriginId;
use vun_sema_ty::item::DeclRef;

use crate::piece::{Bytes, Datum, FuncRef, Lir, Refs};
use crate::unlowered::{Form, Unlowered};

/// Tables a function's ids are placed in.
pub(crate) trait Places {
    /// Where the function `named` is, reported at `at` when it is nowhere.
    fn function(&mut self, named: &FuncRef, at: OriginId) -> Result<Address, Unlowered>;

    /// What the declaration the linker provides names.
    fn linked(&mut self, decl: &DeclRef, at: OriginId) -> Result<Address, Unlowered>;

    /// The id of `datum`, its addresses and trap sites placed already.
    fn datum(&mut self, datum: Datum) -> DataId;

    /// The id of each of `sites`, each site once whatever function it comes from, reported at
    /// `at` past what an id holds.
    fn sites(&mut self, sites: &[Site], at: OriginId) -> Result<Vec<u32>, Unlowered>;

    /// Where the ids `refs` name go in these tables, the data they name placed.
    fn placing(&mut self, refs: &Refs, at: OriginId) -> Result<Placing, Unlowered> {
        let sites = self.sites(&refs.sites, at)?;
        let functions = refs.functions.iter().map(|f| self.function(f, at)).collect::<Result<_, _>>()?;
        let imports = refs.imports.iter().map(|decl| self.linked(decl, at)).collect::<Result<_, _>>()?;
        let mut placing = Placing { functions, imports, data: Vec::with_capacity(refs.data.len()), sites };
        for datum in &refs.data {
            let id = self.datum(placing.datum(datum));
            placing.data.push(id);
        }
        Ok(placing)
    }

    /// The function of `lir`, its ids placed in these tables.
    fn place(&mut self, lir: &Lir, at: OriginId) -> Result<Function, Unlowered> {
        let placing = self.placing(&lir.refs, at)?;
        let mut function = lir.function.clone();
        function.map_addresses(|a| placing.address(a));
        for (_, inst) in function.insts.iter_mut() {
            if let InstKind::Asm(_, Some(check)) = &mut inst.kind {
                check.site = placing.sites[check.site as usize];
            }
        }
        if let Some(check) = &mut function.stack_check {
            check.site = placing.sites[check.site as usize];
        }
        Ok(function)
    }
}

/// The trap sites of a table, each once: the id each has.
#[derive(Default)]
pub(crate) struct SiteIds(Map<Site, u32>);

impl SiteIds {
    /// The ids of `sites` already in `table`.
    pub(crate) fn of(table: &[Site]) -> Self {
        Self(table.iter().cloned().zip(0..).collect())
    }

    /// The id of `site` in `table`, added there the first time, reported at `at` past what an
    /// id holds.
    pub(crate) fn id(&mut self, table: &mut Vec<Site>, site: &Site, at: OriginId) -> Result<u32, Unlowered> {
        if let Some(&id) = self.0.get(site) {
            return Ok(id);
        }
        let id = u32::try_from(table.len()).map_err(|_| Unlowered::unsupported(at, Form::Callee))?;
        table.push(site.clone());
        self.0.insert(site.clone(), id);
        Ok(id)
    }
}

/// Where the ids of one function, or one exported datum, go.
pub(crate) struct Placing {
    functions: Vec<Address>,
    imports: Vec<Address>,
    data: Vec<DataId>,
    /// The id of each of its trap sites.
    sites: Vec<u32>,
}

impl Placing {
    /// Where `address`, of the function's own ids, goes.
    fn address(&self, address: Address) -> Address {
        match address {
            Address::Func(f) => self.functions[f.index()],
            Address::Import(i) => self.imports[i.index()],
            Address::Data(d) => Address::Data(self.data[d.index()]),
        }
    }

    /// `datum` with the addresses its bytes hold and the ids of its trap sites placed.
    fn datum(&self, datum: &Datum) -> Datum {
        Datum { bytes: self.bytes(&datum.bytes), ..datum.clone() }
    }

    /// `bytes` with the addresses they hold and the ids of their trap sites placed.
    pub(crate) fn bytes(&self, bytes: &Bytes) -> Bytes {
        let mut bytes = bytes.clone();
        for reloc in &mut bytes.relocs {
            reloc.target = self.address(reloc.target);
        }
        for site in &mut bytes.sites {
            site.site = self.sites[site.site as usize];
        }
        bytes
    }
}
