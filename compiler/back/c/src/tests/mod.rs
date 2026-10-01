use vun_backend::{Backend, Emits};
use vun_index::Idx;
use vun_intern::Symbol;
use vun_lir::{AbiType, Builder, Conv, Ext, FuncId, Linkage, Scalar, Signature, Terminator, Unit, Value};
use vun_origin::{OriginId, OriginTable};
use vun_span::{SourceFiles, Span};
use vun_target::{
    Atomics, Attributes, Backends, CCompiler, CInt, CTypes, Convention, Endian, FrameSizes, TargetSpec,
};

use crate::ast::{Ident, Names};
use crate::{CBackend, Lack};
use vun_backend::Missing;

mod asm;
mod naked;

pub(crate) fn origin(origins: &OriginTable) -> OriginId {
    let file = SourceFiles::new().add("a.vun", "f := () {};");
    origins.source(Span::new(file, 0, 1))
}

/// A 64-bit target whose C compiler has no atomics.
pub(crate) fn target() -> TargetSpec {
    let int = |bits, align| CInt { bits, align };
    let name = Symbol::intern;
    TargetSpec {
        arch: name("x86_64"),
        isa: name("x86_64"),
        features: Vec::new(),
        cpu: name("generic"),
        os: name("linux"),
        abi: name("gnu"),
        convention: Convention::Sysv,
        pointer_bits: 64,
        register_bits: 64,
        floats: vun_target::Floats::ALL,
        threads: vun_target::Threads::One,
        endian: Endian::Little,
        object_format: name("elf"),
        entry_symbol: None,
        os_version: None,
        backends: Backends { debug: vec![name("c")], release: vec![name("c")] },
        linker: name("gnu"),
        c: CTypes {
            short: int(16, 2),
            int: int(32, 4),
            long: int(64, 8),
            long_long: int(64, 8),
            pointer_align: 8,
            compiler: CCompiler {
                builds_for: vun_target::BuildsFor::Target,
                command: name("cc"),
                archiver: name("ar"),
                flags: Vec::new(),
                optimize: Vec::new(),
                shared: Vec::new(),
                link_flags: Vec::new(),
                libraries: Vec::new(),
                atomics: Atomics::None,
                attributes: Attributes::None,
                frame_sizes: FrameSizes::StackUsage,
                version_flag: None,
                sysroot: None,
            },
        },
    }
}

pub(crate) struct NoLines;

impl vun_backend::Lines for NoLines {
    fn line(&self, _: OriginId) -> Option<vun_backend::Line> {
        None
    }
}

/// A function of `linkage` returning a constant of `ty`.
fn returning(linkage: Linkage, ty: Scalar, origin: OriginId) -> vun_lir::Function {
    let result = if ty.is_int() { AbiType::Int(ty, Ext::Zero) } else { AbiType::Scalar(ty) };
    let signature = Signature { conv: Conv::Vun, params: Vec::new(), result: Some(result), variadic: None };
    let mut b = Builder::new(linkage, signature, origin);
    let v = b.constant(ty, 7, origin);
    b.terminate(Terminator::Return(Some(v)));
    b.finish()
}

/// A unit exporting `symbol`, returning a constant of `ty`.
fn constant(symbol: &str, ty: Scalar, origin: OriginId) -> Unit {
    let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
    unit.functions.push(returning(Linkage::Export(Symbol::intern(symbol)), ty, origin));
    unit
}

#[test]
fn what_c99_lacks_is_refused_where_it_is_needed() {
    let origins = OriginTable::new();
    let at = origin(&origins);
    let wide = CBackend::default().sources(&constant("wide", Scalar::I128, at), &target(), &NoLines);
    assert_eq!(wide, Err(Lack { missing: Missing::Int128, at }));
    let keyword = CBackend::default().sources(&constant("int", Scalar::I32, at), &target(), &NoLines);
    assert_eq!(keyword, Err(Lack { missing: Missing::Symbol(Symbol::intern("int")), at }));
    assert!(CBackend::default().sources(&constant("seven", Scalar::I32, at), &target(), &NoLines).is_ok());
}

#[test]
fn own_names_step_aside_for_link_symbols() {
    let names = Names::new([Symbol::intern("v3"), Symbol::intern("v3_")], None);
    assert_eq!(names.spell(Ident::value(Value::new(3))), "v3__");
    assert_eq!(names.spell(Ident::value(Value::new(4))), "v4");
    assert_eq!(names.spell(Ident::Symbol(Symbol::intern("v3"))), "v3");
}

#[test]
fn a_program_past_a_shard_spans_a_header_its_data_and_shards() {
    let origins = OriginTable::new();
    let at = origin(&origins);
    let mut unit = constant("seven", Scalar::I32, at);
    for _ in 0..3 {
        unit.functions.push(returning(Linkage::Internal, Scalar::I64, at));
    }
    let sharded = CBackend { shard: 2 }.sources(&unit, &target(), &NoLines).unwrap();
    let kinds: Vec<Emits> = sharded.iter().map(|a| a.kind).collect();
    assert_eq!(kinds, [Emits::Header, Emits::Source, Emits::Source, Emits::Source]);
    let whole = CBackend::default().sources(&unit, &target(), &NoLines).unwrap();
    assert_eq!(whole.iter().map(|a| a.kind).collect::<Vec<_>>(), [Emits::Source]);
}

#[test]
fn frame_sizes_are_read_by_function() {
    let origins = OriginTable::new();
    let at = origin(&origins);
    let mut unit = constant("seven", Scalar::I32, at);
    unit.functions.push(returning(Linkage::Internal, Scalar::I64, at));
    let report =
        b"program.c:3:5:seven\t16\tstatic\nprogram.c:9:12:f1\t48\tstatic\nother.c:1:1:elsewhere\t8\tstatic\n\
          program.c:3:5:seven.part.0\t32\tstatic\nprogram.c:9:12:f1.constprop.0\t64\tstatic\n";
    let sizes = CBackend::default().frame_sizes(&unit, report);
    assert_eq!(sizes, [(FuncId::new(0), 48), (FuncId::new(1), 64)]);
}

#[test]
fn data_aligned_past_c_or_placed_in_a_section_needs_gnu_attributes() {
    use crate::ast::{Attribute, Item};
    use crate::emit::Emitter;
    let origins = OriginTable::new();
    let at = origin(&origins);
    let mut unit = constant("seven", Scalar::I32, at);
    let data = |section| vun_lir::Data {
        linkage: Linkage::Internal,
        bytes: vec![1; 64],
        relocs: Vec::new(),
        align: 64,
        writable: false,
        section,
        storage: vun_lir::Storage::Program,
        origin: at,
    };
    unit.data.push(data(None));
    let lacks = |unit: &Unit, target: &TargetSpec| Emitter::sources(unit, target, &NoLines, usize::MAX).err();
    assert_eq!(lacks(&unit, &target()), Some(Lack { missing: Missing::Align(64), at }));
    let vectors = Symbol::intern(".vectors");
    unit.data[vun_lir::DataId::new(0)] = data(Some(vectors));
    assert_eq!(lacks(&unit, &target()), Some(Lack { missing: Missing::Section, at }));
    let mut gnu = target();
    gnu.c.compiler.attributes = Attributes::Gnu;
    let sources = Emitter::sources(&unit, &gnu, &NoLines, usize::MAX).unwrap();
    let placed = sources.iter().flat_map(|s| &s.file.items).find_map(|item| match item {
        Item::Object { attributes, .. } if !attributes.is_empty() => Some(attributes.clone()),
        _ => None,
    });
    assert_eq!(placed, Some(vec![Attribute::Section(vectors), Attribute::Aligned(64)]));
}

#[test]
fn qualifiers_on_pointers_follow_their_star() {
    use crate::ast::{Decl, File, Item, Ty};
    let name = Symbol::intern("p");
    let typedef = |ty: Ty| {
        let file = File {
            names: Names::new([name], None),
            items: vec![Item::Typedef(Decl::new(ty, Ident::Symbol(name)))],
        };
        file.to_string()
    };
    let volatile = |ty: Ty| Ty::Volatile(Box::new(ty));
    assert_eq!(typedef(volatile(Ty::address()).pointer()), "typedef u8 *volatile *p;\n");
    assert_eq!(typedef(volatile(Ty::BYTE).pointer()), "typedef volatile u8 *p;\n");
    assert_eq!(typedef(Ty::code().pointer()), "typedef void (**p)(void);\n");
    assert_eq!(typedef(volatile(Ty::code())), "typedef void (*volatile p)(void);\n");
}
