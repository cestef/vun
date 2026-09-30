//! Resolving the objects' symbols: which definition each name has, strong over weak and the first
//! declaring object's COMDAT groups over later ones', and where each symbol of each object
//! leads: a function or segment of some object, an import of the program, something the linker
//! provides, or nothing for an undefined weak symbol.

use vun_hash::{Map, Set};
use vun_wasm::{ENV, Object, SymbolFlags, Symbolic};

use crate::provided::Provided;
use crate::section::Section;
use crate::unlinked::Unlinked;

/// A function an object defines: the object, and its index past the imported ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Func {
    pub(crate) object: usize,
    pub(crate) index: u32,
}

/// A data segment of an object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Seg {
    pub(crate) object: usize,
    pub(crate) index: u32,
}

/// A function the program imports: from a library, under a name, of one of an object's types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Import<'o> {
    pub(crate) module: &'o str,
    pub(crate) name: &'o str,
    /// The object whose type it has, and the type's index there.
    pub(crate) object: usize,
    pub(crate) ty: u32,
}

/// Where a symbol leads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Resolved<'o> {
    Func(Func),
    Import(Import<'o>),
    /// Bytes at an offset into a segment.
    Data(Seg, u32),
    Provided(Provided<'o>),
    /// An undefined weak function, called as a stub that traps, of its object's type at this
    /// index; its address is null.
    Missing {
        object: usize,
        ty: u32,
    },
    /// An undefined weak datum: its address is null.
    Null,
    /// A symbol no relocation names.
    Unnamed,
}

/// A named definition: its object and its place in the object's symbols.
#[derive(Clone, Copy)]
struct Def {
    object: usize,
    symbol: usize,
    weak: bool,
}

pub(crate) struct Symbols<'o> {
    pub(crate) objects: &'o [Object],
    /// Where each symbol of each object leads.
    pub(crate) resolved: Vec<Vec<Resolved<'o>>>,
    /// The functions and segments of COMDAT groups another object declared first.
    pub(crate) dropped: (Set<Func>, Set<Seg>),
    /// The sections sources name, in the order the objects first do.
    pub(crate) sections: Vec<&'o str>,
    defs: Map<&'o str, Def>,
}

impl<'o> Symbols<'o> {
    /// The symbols of `objects`, resolved.
    ///
    /// # Errors
    /// Where a symbol is defined twice, or used and defined nowhere.
    pub(crate) fn of(objects: &'o [Object]) -> Result<Self, Unlinked> {
        let mut sections: Vec<&str> = Vec::new();
        let named =
            objects.iter().flat_map(|o| &o.data.segments).filter_map(|s| Section::of(&s.name).named());
        for name in named {
            if !sections.contains(&name) {
                sections.push(name);
            }
        }
        let dropped = Self::dropped(objects);
        let mut symbols = Self { objects, resolved: Vec::new(), dropped, sections, defs: Map::default() };
        symbols.define()?;
        symbols.resolved = (0..objects.len()).map(|o| symbols.resolve(o)).collect::<Result<_, _>>()?;
        Ok(symbols)
    }

    /// The members of every COMDAT group an earlier object declared.
    fn dropped(objects: &[Object]) -> (Set<Func>, Set<Seg>) {
        let mut kept: Set<&str> = Set::default();
        let (mut funcs, mut segs) = (Set::default(), Set::default());
        for (object, o) in objects.iter().enumerate() {
            for comdat in &o.comdats {
                if kept.insert(&comdat.name) {
                    continue;
                }
                funcs.extend(comdat.funcs.iter().map(|&index| Func { object, index }));
                segs.extend(comdat.segments.iter().map(|&index| Seg { object, index }));
            }
        }
        (funcs, segs)
    }

    /// Whether a symbol an object defines is dropped with its COMDAT group.
    fn is_dropped(&self, object: usize, what: Symbolic) -> bool {
        match what {
            Symbolic::Function(index) => {
                let own = index.checked_sub(Self::imported(&self.objects[object]));
                own.is_some_and(|index| self.dropped.0.contains(&Func { object, index }))
            }
            Symbolic::Data(Some(def)) => self.dropped.1.contains(&Seg { object, index: def.segment }),
            _ => false,
        }
    }

    fn define(&mut self) -> Result<(), Unlinked> {
        for (object, o) in self.objects.iter().enumerate() {
            for (symbol, s) in o.symbols.iter().enumerate() {
                let Some(name) = s.name.as_deref().filter(|_| s.defined() && !s.local()) else { continue };
                if self.is_dropped(object, s.what) {
                    continue;
                }
                let def = Def { object, symbol, weak: s.flags.has(SymbolFlags::WEAK) };
                match self.defs.get(name) {
                    Some(old) if !old.weak && !def.weak => return Err(Unlinked::Duplicate(name.to_owned())),
                    Some(old) if !old.weak || def.weak => {}
                    _ => {
                        self.defs.insert(name, def);
                    }
                }
            }
        }
        Ok(())
    }

    /// The number of functions `object` imports.
    pub(crate) fn imported(object: &Object) -> u32 {
        u32::try_from(object.imported_funcs().count()).expect("functions fit u32")
    }

    /// Where the symbols of the object at `o` lead.
    fn resolve(&self, o: usize) -> Result<Vec<Resolved<'o>>, Unlinked> {
        let object = &self.objects[o];
        let resolved = object.symbols.iter().map(|s| {
            // A definition dropped with its group is the kept one's.
            let named = s.name.as_deref().filter(|_| !s.local());
            if s.defined() && !self.is_dropped(o, s.what) {
                return Ok(self.defined(o, s.what));
            }
            if let Some(def) = named.and_then(|name| self.defs.get(name)) {
                let there = self.objects[def.object].symbols[def.symbol].what;
                let found = self.defined(def.object, there);
                let same = matches!(
                    (s.what, found),
                    (Symbolic::Function(_), Resolved::Func(_)) | (Symbolic::Data(_), Resolved::Data(..))
                );
                return if same {
                    Ok(found)
                } else {
                    Err(Unlinked::Undefined(named.unwrap_or_default().to_owned()))
                };
            }
            if let Some(provided) = named.and_then(|name| Provided::named(name, s.what, &self.sections)) {
                return Ok(Resolved::Provided(provided));
            }
            let weak = s.flags.has(SymbolFlags::WEAK);
            match s.what {
                Symbolic::Function(index) => {
                    let (import, ty) = object
                        .imported_funcs()
                        .nth(index as usize)
                        .ok_or(Unlinked::Relocation { input: o, symbol: index })?;
                    if import.module != ENV {
                        Ok(Resolved::Import(Import {
                            module: &import.module,
                            name: &import.name,
                            object: o,
                            ty,
                        }))
                    } else if weak {
                        Ok(Resolved::Missing { object: o, ty })
                    } else {
                        Err(Unlinked::Undefined(named.unwrap_or(&import.name).to_owned()))
                    }
                }
                Symbolic::Data(_) if weak => Ok(Resolved::Null),
                Symbolic::Other(_) => Ok(Resolved::Unnamed),
                _ => Err(Unlinked::Undefined(named.unwrap_or_default().to_owned())),
            }
        });
        resolved.collect()
    }

    /// Where a symbol the object at `o` defines leads.
    fn defined(&self, o: usize, what: Symbolic) -> Resolved<'o> {
        let imported = Self::imported(&self.objects[o]);
        match what {
            Symbolic::Function(index) => index
                .checked_sub(imported)
                .map_or(Resolved::Unnamed, |index| Resolved::Func(Func { object: o, index })),
            Symbolic::Data(Some(def)) => Resolved::Data(Seg { object: o, index: def.segment }, def.offset),
            _ => Resolved::Unnamed,
        }
    }

    /// Where the name `name` leads, if an object defines it.
    pub(crate) fn named(&self, name: &str) -> Option<Resolved<'o>> {
        self.defs.get(name).map(|def| self.resolved[def.object][def.symbol])
    }

    /// Every function an object exports, under the name its object gives its export or else its
    /// own, with where it leads.
    pub(crate) fn exported(&self) -> impl Iterator<Item = (&'o str, Resolved<'o>)> + '_ {
        let exported = self.defs.iter().filter(|(_, def)| {
            let flags = self.objects[def.object].symbols[def.symbol].flags;
            flags.has(SymbolFlags::EXPORTED)
        });
        exported.map(|(&name, def)| {
            let object = &self.objects[def.object];
            let named = match object.symbols[def.symbol].what {
                Symbolic::Function(index) => {
                    object.exports.iter().find(|(_, at)| *at == index).map(|(name, _)| name.as_str())
                }
                _ => None,
            };
            (named.unwrap_or(name), self.resolved[def.object][def.symbol])
        })
    }

    /// Every symbol an object keeps whether reached or not.
    pub(crate) fn kept(&self) -> impl Iterator<Item = Resolved<'o>> + '_ {
        self.objects.iter().zip(&self.resolved).flat_map(|(object, resolved)| {
            let kept = object.symbols.iter().zip(resolved);
            kept.filter(|(s, _)| s.defined() && s.flags.has(SymbolFlags::NO_STRIP)).map(|(_, &r)| r)
        })
    }
}
