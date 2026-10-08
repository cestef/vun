//! The machine code of a program's functions kept between builds (decision 669): one file per
//! program, toolchain, target and profile under `.vun/code/`, read when code is generated and
//! written again, whole, when a build compiled a function it did not hold. A file holds the code
//! the last build used, so it does not grow with edits; the entries share one stream, so a source
//! file the origins of many functions name is written once.
//!
//! A function is looked up as lowered, before legalizing, so that one whose code is kept is
//! neither legalized nor compiled. It is keyed in its own numbering of what it names, so that its
//! code is the same whatever else the unit holds, with what legalizing and compiling read of the
//! unit: the signature each name is called with and the width of pointers.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use vun_asm::Reloc;
use vun_backend::Compiled;
use vun_codec::{Context, Decode, Encode, Pools, Writer};
use vun_fs::Edit;
use vun_hash::{Hash, Map};
use vun_index::{Idx, IndexVec};
use vun_lir::unit::{ImportKind, Unit};
use vun_lir::{Address, DataId, FuncId, Function, ImportId};
use vun_par::Parallel;
use vun_query::{Build, Fingerprint};
use vun_target::{Profile, TargetSpec};
use vun_trace::{Activity, tracing};

/// Where builds keep machine code, among what they keep.
const CODE: &str = "code";

/// Looking up the kept code of a unit's functions, as timed.
const LOOK_UP: Activity = Activity::stage("look up kept code");

/// A file of kept code, as written.
#[derive(Encode, Decode)]
struct Held {
    entries: Vec<(Fingerprint, Vec<u8>)>,
    pools: Pools,
}

/// The code a build reads and keeps, for the functions of one unit.
pub(super) struct Kept {
    at: PathBuf,
    read: Option<(Context, Map<Fingerprint, Vec<u8>>)>,
    functions: IndexVec<FuncId, Keyed>,
    /// The code this build compiled anew.
    compiled: Mutex<Vec<(Fingerprint, Compiled<()>)>>,
}

/// A function of the unit: its key and names, and the code kept for it.
struct Keyed {
    key: Fingerprint,
    names: Names,
    code: Option<Compiled<()>>,
}

impl Kept {
    /// The code kept in `work` for programs of `target` built under `profile` by this toolchain,
    /// looked up for the functions of `unit` as lowered; `context` names what else legalizing and
    /// compiling read: the backend and how it legalizes. None when the toolchain cannot be named.
    pub(super) fn open(
        work: &Path,
        root: &Path,
        (target, profile): (&TargetSpec, Profile),
        context: Fingerprint,
        unit: &Unit,
    ) -> Option<Self> {
        let key = Hash::of_value(&(Build::identity()?, root, target, profile));
        let at = work.join(CODE).join(key.digest());
        let read: Option<(Context, Map<Fingerprint, Vec<u8>>)> =
            std::fs::read(&at).ok().and_then(|bytes| match Held::decoded(&bytes) {
                Ok(held) => Some((Context::new(held.pools), held.entries.into_iter().collect())),
                Err(error) => {
                    tracing::debug!(%error, path = %at.display(), "kept code does not read");
                    None
                }
            });
        let looking = LOOK_UP.start();
        let functions: Vec<&Function> = unit.functions.iter().map(|(_, f)| f).collect();
        let functions = functions.par_map(|f| {
            let (names, key) = Names::of(f, unit, context);
            let code = read.as_ref().and_then(|(context, held)| {
                let code = context.read::<Compiled<()>>(held.get(&key)?).ok()?;
                let relocs = Names::relocs(&code.code.relocs, |a| names.unit(a))?;
                Some(Compiled { code: vun_asm::Code { relocs, ..code.code }, ..code })
            });
            Keyed { key, names, code }
        });
        drop(looking);
        Some(Self { at, read, functions: functions.into(), compiled: Mutex::default() })
    }

    /// Whether code is kept for `id`, which is then neither legalized nor compiled.
    pub(super) fn holds(&self, id: FuncId) -> bool {
        self.functions.get(id).is_some_and(|f| f.code.is_some())
    }

    /// Writes the code this build used, when it compiled code the file did not hold.
    pub(super) fn close(self) {
        let compiled = self.compiled.into_inner().unwrap_or_else(std::sync::PoisonError::into_inner);
        let held: Vec<&Keyed> = self.functions.iter().map(|(_, f)| f).filter(|f| f.code.is_some()).collect();
        tracing::debug!(read = held.len(), compiled = compiled.len(), "kept code");
        if compiled.is_empty() {
            return;
        }
        let mut writer = Writer::new();
        let mut entries: Vec<(Fingerprint, Vec<u8>)> =
            compiled.iter().map(|(key, code)| (*key, writer.encode(code))).collect();
        if let Some((context, read)) = &self.read {
            for f in held {
                if let Ok(code) = context.read::<Compiled<()>>(&read[&f.key]) {
                    entries.push((f.key, writer.encode(&code)));
                }
            }
        }
        let held = Held { entries, pools: writer.into_pools() };
        if let Err(error) = self.at.put(held.encoded()) {
            tracing::debug!(%error, path = %self.at.display(), "kept code not written");
        }
    }
}

impl vun_backend::Kept for Kept {
    fn code(&self, id: FuncId) -> Option<Compiled<()>> {
        self.functions.get(id)?.code.as_ref().map(Compiled::to_unlisted)
    }

    /// Code naming what the function did not as lowered, a helper legalizing added, is not kept.
    fn keep(&self, id: FuncId, code: Compiled<()>) {
        let Some(f) = self.functions.get(id) else { return };
        let Some(relocs) = Names::relocs(&code.code.relocs, |a| f.names.local.get(&a).copied()) else {
            return;
        };
        let code = Compiled { code: vun_asm::Code { relocs, ..code.code }, ..code };
        self.compiled.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push((f.key, code));
    }
}

/// The addresses of the unit a function names, numbered in the order first met.
struct Names {
    unit: Vec<Address>,
    local: Map<Address, Address>,
}

impl Names {
    /// The names of `f`, a function of `unit`, and the key of its code under `context`.
    fn of(f: &Function, unit: &Unit, context: Fingerprint) -> (Self, Fingerprint) {
        let mut names = Self { unit: Vec::new(), local: Map::default() };
        let mut local = f.clone();
        local.map_addresses(|address| names.number(address));
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
        (names, key)
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

    /// The address of the unit numbered `local`, if the function names one so.
    fn unit(&self, local: Address) -> Option<Address> {
        let n = match local {
            Address::Func(id) => id.index(),
            Address::Data(id) => id.index(),
            Address::Import(id) => id.index(),
        };
        self.unit.get(n).copied()
    }

    /// `relocs` with each target renamed by `to`; none where `to` renames one not.
    fn relocs(
        relocs: &[Reloc<Address>],
        to: impl Fn(Address) -> Option<Address>,
    ) -> Option<Vec<Reloc<Address>>> {
        relocs.iter().map(|&r| Some(Reloc { target: to(r.target)?, ..r })).collect()
    }
}
