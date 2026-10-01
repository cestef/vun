//! Where everything the program keeps goes: its types, its function index space (the imports,
//! the objects' functions in order, then what the linker makes: the call of the initialization
//! functions and a stub that traps per type of undefined weak function called), and its memory:
//! the stack first, growing down from [`STACK`], so overflowing it leaves memory rather than
//! overwriting data, then the sections, each segment at its alignment.

use std::ops::Range;
use vun_hash::Map;

use vun_wasm::{FuncType, Segment};

use crate::live::Live;
use crate::provided::Provided;
use crate::resolve::{Func, Import, Resolved, Seg, Symbols};
use crate::section::Section;
use crate::unlinked::Unlinked;

/// The bytes of the stack: a megabyte, as Rust's wasm targets give it, since `wasm-ld`'s 64 KiB
/// holds two buffers of a command line's size and no more.
pub const STACK: u32 = 1024 * 1024;
/// The alignment of the stack's top and of the heap's start: of anything.
const ALIGN: u32 = 16;

/// What a relocation writes for a symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Value {
    /// A function's index; a missing one's is its stub's, and its address null.
    Func {
        index: u32,
        missing: bool,
    },
    Addr(u32),
    Global(u32),
    Table(u32),
    None,
}

/// The function the linker makes to call the initialization functions.
pub(crate) struct Ctors {
    pub(crate) index: u32,
    pub(crate) ty: u32,
    /// The functions it calls, in order.
    pub(crate) calls: Vec<u32>,
}

pub(crate) struct Layout<'o> {
    pub(crate) types: Vec<FuncType>,
    type_of: Vec<Vec<u32>>,
    pub(crate) imports: Vec<Import<'o>>,
    pub(crate) funcs: Vec<Func>,
    index_of: Map<Func, u32>,
    /// The function calling the initialization functions, when something calls it.
    pub(crate) ctors: Option<Ctors>,
    /// The type of each stub, in order after the other functions.
    pub(crate) stubs: Vec<u32>,
    /// The segments kept, in memory order, each at its address, in its section.
    pub(crate) segments: Vec<(Seg, u32, Section<'o>)>,
    addr_of: Map<Seg, u32>,
    bounds: Map<Section<'o>, (u32, u32)>,
    /// Where the data ends.
    pub(crate) end: u32,
}

impl<'o> Layout<'o> {
    /// Where what `live` keeps of `symbols` goes.
    ///
    /// # Errors
    /// Where one function is imported under one name with two types.
    pub(crate) fn of(symbols: &Symbols<'o>, live: &Live<'o>) -> Result<Self, Unlinked> {
        let mut types = Vec::new();
        let mut listed: Map<FuncType, u32> = Map::default();
        let mut list = |ty: &FuncType| {
            *listed.entry(ty.clone()).or_insert_with(|| {
                types.push(ty.clone());
                u32::try_from(types.len() - 1).expect("types fit u32")
            })
        };
        let type_of: Vec<Vec<u32>> =
            symbols.objects.iter().map(|o| o.types.iter().map(&mut list).collect()).collect();
        let ctors_type = live.ctors.then(|| list(&FuncType::default()));
        let mut layout = Self {
            types,
            type_of,
            imports: Vec::new(),
            funcs: Vec::new(),
            index_of: Map::default(),
            ctors: None,
            stubs: Vec::new(),
            segments: Vec::new(),
            addr_of: Map::default(),
            bounds: Map::default(),
            end: STACK,
        };
        layout.imports(symbols, live)?;
        layout.funcs(symbols, live, ctors_type);
        layout.memory(symbols, live);
        Ok(layout)
    }

    /// The imports called, once each, in the order the objects name them.
    fn imports(&mut self, symbols: &Symbols<'o>, live: &Live<'o>) -> Result<(), Unlinked> {
        let named = symbols.resolved.iter().flatten().filter_map(|r| match r {
            Resolved::Import(import) if live.imports.contains(import) => Some(*import),
            _ => None,
        });
        for import in named {
            let ty = self.type_of[import.object][import.ty as usize];
            match self.imports.iter().find(|i| i.module == import.module && i.name == import.name) {
                Some(same) if self.type_of[same.object][same.ty as usize] != ty => {
                    return Err(Unlinked::Clash {
                        module: import.module.to_owned(),
                        name: import.name.to_owned(),
                    });
                }
                Some(_) => {}
                None => self.imports.push(import),
            }
        }
        Ok(())
    }

    /// The objects' functions kept, in order, then the linker's.
    fn funcs(&mut self, symbols: &Symbols<'o>, live: &Live<'o>, ctors: Option<u32>) {
        let mut funcs: Vec<Func> = live.funcs.iter().copied().collect();
        funcs.sort_unstable();
        let first = self.imports.len();
        self.index_of = funcs.iter().enumerate().map(|(i, &f)| (f, Self::index(first + i))).collect();
        self.funcs = funcs;
        self.ctors = ctors.map(|ty| {
            let calls = Live::inits(symbols).filter_map(|(_, r)| match r {
                Resolved::Func(f) => self.index_of.get(&f).copied(),
                _ => None,
            });
            Ctors { index: Self::index(first + self.funcs.len()), ty, calls: calls.collect() }
        });
        let mut stubs: Vec<u32> = live.missing.iter().map(|&(o, ty)| self.type_of[o][ty as usize]).collect();
        stubs.sort_unstable();
        stubs.dedup();
        self.stubs = stubs;
    }

    /// The segments kept, section by section after the stack.
    fn memory(&mut self, symbols: &Symbols<'o>, live: &Live<'o>) {
        let rank = |section: Section<'_>| match section {
            Section::Rodata => (0, 0),
            Section::Data => (1, 0),
            Section::Named(name) => (2, symbols.sections.iter().position(|&s| s == name).unwrap_or(0)),
            Section::Bss => (3, 0),
        };
        let mut kept: Vec<(Section<'o>, Seg)> = live
            .segs
            .iter()
            .map(|&seg| {
                (Section::of(&symbols.objects[seg.object].data.segments[seg.index as usize].name), seg)
            })
            .collect();
        kept.sort_by_key(|&(section, seg)| (rank(section), seg));
        let mut at = STACK;
        for (section, seg) in kept {
            let segment = &symbols.objects[seg.object].data.segments[seg.index as usize];
            at = at.next_multiple_of(u32::try_from(segment.align).unwrap_or(ALIGN).max(1));
            self.addr_of.insert(seg, at);
            self.segments.push((seg, at, section));
            let bound = self.bounds.entry(section).or_insert((at, at));
            at += u32::try_from(segment.bytes.len()).expect("memory fits 4 GiB");
            bound.1 = at;
        }
        self.end = at;
    }

    fn index(at: usize) -> u32 {
        u32::try_from(at).expect("functions fit u32")
    }

    /// Whether the program keeps the function of the object at `o` whose body, among `bodies`,
    /// holds `offset` of its code.
    pub(crate) fn keeps_func(&self, o: usize, bodies: &[Range<usize>], offset: usize) -> bool {
        Self::holding(bodies, |body| body, offset)
            .is_some_and(|index| self.index_of.contains_key(&Func { object: o, index }))
    }

    /// Whether the program keeps the segment of the object at `o` whose bytes hold `offset` of
    /// its data.
    pub(crate) fn keeps_seg(&self, o: usize, segments: &[Segment], offset: usize) -> bool {
        let held = Self::holding(segments, |segment| &segment.bytes, offset);
        held.is_some_and(|index| self.addr_of.contains_key(&Seg { object: o, index }))
    }

    /// Which of `items`, in order of the `range` each covers, holds `offset`.
    fn holding<T>(items: &[T], range: impl Fn(&T) -> &Range<usize>, offset: usize) -> Option<u32> {
        let at = items.partition_point(|item| range(item).end <= offset);
        items.get(at).filter(|item| range(item).contains(&offset)).and_then(|_| u32::try_from(at).ok())
    }

    /// The program's type of the object at `o`'s type `ty`.
    pub(crate) fn ty(&self, o: usize, ty: u32) -> u32 {
        self.type_of[o][ty as usize]
    }

    /// The pages of memory: the stack and the data.
    pub(crate) fn pages(&self) -> u32 {
        u32::try_from(u64::from(self.end).div_ceil(vun_wasm::PAGE)).expect("pages fit u32")
    }

    /// What a relocation writes for a symbol leading to `resolved`.
    pub(crate) fn value(&self, resolved: Resolved<'o>) -> Value {
        let func = |index| Value::Func { index, missing: false };
        match resolved {
            Resolved::Func(f) => self.index_of.get(&f).copied().map_or(Value::None, func),
            Resolved::Import(import) => {
                let at = self.imports.iter().position(|i| i.module == import.module && i.name == import.name);
                at.map_or(Value::None, |at| func(Self::index(at)))
            }
            Resolved::Missing { object, ty } => {
                let ty = self.type_of[object][ty as usize];
                let stub = self.stubs.iter().position(|&s| s == ty).map(|at| self.stub(at));
                stub.map_or(Value::None, |index| Value::Func { index, missing: true })
            }
            Resolved::Data(seg, offset) => {
                self.addr_of.get(&seg).map_or(Value::None, |&at| Value::Addr(at + offset))
            }
            Resolved::Null => Value::Addr(0),
            Resolved::Provided(provided) => self.provided(provided),
            Resolved::Unnamed => Value::None,
        }
    }

    /// The index of the stub at `at` among them.
    pub(crate) fn stub(&self, at: usize) -> u32 {
        let first = self.imports.len() + self.funcs.len() + usize::from(self.ctors.is_some());
        Self::index(first + at)
    }

    fn provided(&self, provided: Provided<'o>) -> Value {
        match provided {
            Provided::StackPointer => Value::Global(0),
            Provided::Table => Value::Table(0),
            Provided::CallCtors => {
                self.ctors.as_ref().map_or(Value::None, |c| Value::Func { index: c.index, missing: false })
            }
            Provided::DataEnd => Value::Addr(self.end),
            Provided::HeapBase => Value::Addr(self.end.next_multiple_of(ALIGN)),
            Provided::DsoHandle | Provided::StackLow => Value::Addr(0),
            Provided::StackHigh => Value::Addr(STACK),
            Provided::Bound { section, end } => {
                let bounds =
                    self.bounds.get(&Section::Named(section)).copied().unwrap_or((self.end, self.end));
                Value::Addr(if end { bounds.1 } else { bounds.0 })
            }
        }
    }
}
