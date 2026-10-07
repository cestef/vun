//! The backend's code for units of LIR: functions compile for each architecture it lists, what
//! it cannot express is refused where it is, and the object reads back.

use cranelift_codegen::ir::{Type, types};
use vun_asm::Reloc;
use vun_backend::Compiled;
use vun_backend::{Lack, Missing};
use vun_intern::Symbol;
use vun_lir::testing::{origin, sum};
use vun_lir::{AbiType, Builder, Conv, FuncId, InstKind, Linkage, Scalar, Signature, Terminator, Unit};
use vun_origin::OriginTable;
use vun_target::{Convention, Profile};

mod tail;

use crate::compile::Compilation;
use crate::failed::Failed;
use crate::isa::{Isa, NAMES};
use crate::plan::{Lane, Layout, Parts, Pass, Returned, Skip};

/// What the assisting backend's code is, as far as these tests tell.
const ASSISTED: u8 = 0xC3;

fn convention(arch: &str) -> Convention {
    if arch == "arm64" { Convention::Aapcs64 } else { Convention::Sysv }
}

fn isa(arch: &str) -> Isa {
    let name = Symbol::intern;
    Isa::build(name(arch), name("linux"), name("elf"), convention(arch), Profile::Debug)
        .expect("elf is written")
}

/// A unit of one exported function of `signature`, holding `kind` and returning.
fn unit_of(signature: Signature, kind: Option<InstKind>) -> Unit {
    let origins = OriginTable::new();
    let at = origin(&origins);
    let mut b = Builder::new(Linkage::Export(Symbol::intern("f")), signature, at);
    if let Some(kind) = kind {
        b.push(kind, None, at);
    }
    b.terminate(Terminator::Return(None));
    let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
    unit.functions.push(b.finish());
    unit
}

fn signature(conv: Conv, params: Vec<AbiType>) -> Signature {
    Signature { conv, params, result: None, variadic: None, convention: None }
}

/// What the backend lacks of `unit`, with nothing to assist it.
fn lacks(unit: &Unit) -> Option<Missing> {
    match Compilation::of(unit, &isa(NAMES[0].0), None, false) {
        Err(Failed::Lacks(Lack { missing, .. })) => Some(missing),
        _ => None,
    }
}

/// A block of inline assembly of no instructions, which no processor lacks and Cranelift cannot
/// express.
fn asm() -> InstKind {
    let block = vun_inline::Block {
        arch: vun_items::lang::arch::x86_64,
        insts: Vec::new(),
        returns: true,
        needs: Vec::new(),
    };
    InstKind::Asm(Box::new(block), None)
}

#[test]
fn a_loop_compiles_for_each_architecture() {
    let origins = OriginTable::new();
    let unit = sum(origin(&origins));
    for name in NAMES {
        let compiled =
            Compilation::of(&unit, &isa(name.0), None, true).unwrap_or_else(|_| panic!("{name:?}"));
        let code = &compiled.code.iter().next().expect("a function").1.code;
        assert!(!code.bytes.is_empty());
        assert!(
            code.relocs.is_empty(),
            "{:?}",
            code.relocs.iter().map(|r: &Reloc<_>| r.kind).collect::<Vec<_>>()
        );
        assert_eq!(compiled.listing.len(), 1);
    }
}

#[test]
fn what_cranelift_does_not_express_is_refused_where_it_is() {
    assert_eq!(lacks(&unit_of(signature(Conv::Vun, Vec::new()), Some(asm()))), Some(Missing::Asm));
    let float = AbiType::Scalar(Scalar::F64);
    let variadic = Signature { variadic: Some(0), ..signature(Conv::C, vec![float]) };
    assert_eq!(lacks(&unit_of(variadic, None)), Some(Missing::Variadic));
    let wide = AbiType::Wide { limb: Scalar::I64, limbs: 2, ext: vun_lir::Ext::Zero };
    assert_eq!(lacks(&unit_of(signature(Conv::C, vec![wide.clone()]), None)), Some(Missing::Int128));
    assert_eq!(lacks(&unit_of(signature(Conv::Vun, vec![wide]), None)), None);
}

#[test]
fn a_function_it_cannot_express_is_the_assisting_backends() {
    let unit = unit_of(signature(Conv::Vun, Vec::new()), Some(asm()));
    let assisted = |ids: &[FuncId]| {
        let code = |id| {
            let mut code = Compiled::unreached();
            code.code.bytes = vec![ASSISTED];
            (id, code)
        };
        Ok(ids.iter().copied().map(code).collect())
    };
    let compiled = Compilation::of(&unit, &isa(NAMES[0].0), Some(&assisted), false).expect("assisted");
    let (_, code) = compiled.code.iter().next().expect("a function");
    assert_eq!(code.code.bytes, [ASSISTED]);
}

#[test]
fn a_format_it_does_not_write_is_refused() {
    let name = Symbol::intern;
    let isa = Isa::build(name("x86_64"), name("linux"), name("macho"), Convention::Sysv, Profile::Debug);
    assert!(matches!(isa, Err(Missing::Format(_))));
}

fn record(fields: &[(u64, Scalar)], size: u64) -> AbiType {
    AbiType::Record(vun_lir::Record { size, align: 8, fields: fields.to_vec() })
}

fn parts(size: u64, lanes: &[(u64, Type)]) -> Parts {
    Parts {
        layout: Layout { size, align: 8 },
        lanes: lanes.iter().map(|&(offset, ty)| Lane { offset, bytes: u64::from(ty.bytes()), ty }).collect(),
    }
}

/// How each parameter of `plan` is passed, without what comes before it.
fn passes(plan: &crate::abi::Plan) -> Vec<&Pass> {
    plan.params.iter().map(|p| &p.pass).collect()
}

/// How `arch` plans a call of the C function of `params` and `result`.
fn plan(arch: &str, params: Vec<AbiType>, result: Option<AbiType>) -> Result<crate::abi::Plan, Missing> {
    isa(arch).plan(&Signature { result, ..signature(Conv::C, params) })
}

#[test]
fn a_record_by_value_travels_as_the_convention_places_it() {
    let pair = || record(&[(0, Scalar::I64), (8, Scalar::I64)], 16);
    let mixed = record(&[(0, Scalar::F64), (8, Scalar::I64)], 16);
    let big = || record(&[(0, Scalar::I64), (8, Scalar::I64), (16, Scalar::I64)], 24);
    for arch in ["x86_64", "arm64"] {
        let in_registers = parts(16, &[(0, types::I64), (8, types::I64)]);
        let planned = plan(arch, vec![pair()], None).expect(arch);
        assert_eq!(passes(&planned), [&Pass::Parts(in_registers.clone())], "{arch}");
        assert_eq!(planned.signature.params.len(), 2, "{arch}");
        let returned = plan(arch, Vec::new(), Some(pair())).expect(arch);
        assert_eq!(returned.returned, Returned::Parts(in_registers), "{arch}");
        assert_eq!(returned.signature.returns.len(), 2, "{arch}");
        let large = plan(arch, Vec::new(), Some(big())).expect(arch);
        assert_eq!(large.returned, Returned::Sret, "{arch}");
        assert!(large.signature.returns.is_empty(), "{arch}");
        // Cranelift accepts each signature as the function's own and as a call's.
        for signature in [
            signature(Conv::C, vec![pair(), big(), pair()]),
            Signature { result: Some(pair()), ..signature(Conv::C, vec![big()]) },
            Signature { result: Some(big()), ..signature(Conv::C, vec![pair()]) },
        ] {
            let compiled = Compilation::of(&unit_of(signature, None), &isa(arch), None, false);
            assert!(compiled.is_ok(), "{arch}");
        }
    }
    let x86 = plan("x86_64", vec![mixed], None).expect("x86_64");
    assert_eq!(passes(&x86), [&Pass::Parts(parts(16, &[(0, types::F64), (8, types::I64)]))]);
    let layout = Layout { size: 24, align: 8 };
    assert_eq!(passes(&plan("x86_64", vec![big()], None).expect("x86_64")), [&Pass::Struct(layout)]);
    assert_eq!(passes(&plan("arm64", vec![big()], None).expect("arm64")), [&Pass::Copy(layout)]);
}

/// A record of `size` bytes aligned to `align`, its fields bytes.
fn bytes(size: u64, align: u64) -> AbiType {
    let fields = (0..size).map(|at| (at, Scalar::I8)).collect();
    AbiType::Record(vun_lir::Record { size, align, fields })
}

fn accepted(arch: &str, signature: Signature) {
    let compiled = Compilation::of(&unit_of(signature, None), &isa(arch), None, false);
    assert!(compiled.is_ok(), "{arch}");
}

#[test]
fn a_part_of_an_odd_size_travels_in_a_register_wide_enough() {
    for arch in ["x86_64", "arm64"] {
        for size in [3, 5, 6, 7] {
            let planned = plan(arch, vec![bytes(size, 1)], None).expect(arch);
            let [Pass::Parts(parts)] = passes(&planned)[..] else {
                panic!("{arch}: {size} bytes in a register")
            };
            let wide = if size == 3 { types::I32 } else { types::I64 };
            assert_eq!(parts.lanes, [Lane { offset: 0, bytes: size, ty: wide }], "{arch}");
            accepted(arch, signature(Conv::C, vec![bytes(size, 1)]));
            accepted(arch, Signature { result: Some(bytes(size, 1)), ..signature(Conv::C, Vec::new()) });
        }
    }
}

#[test]
fn a_struct_argument_is_whole_words_and_may_start_past_the_arguments_so_far() {
    let ints = || vec![AbiType::Int(Scalar::I64, vun_lir::Ext::Zero); 7];
    // 20 bytes on the stack take 24, as a struct argument.
    let odd = plan("x86_64", vec![bytes(20, 4)], None).expect("x86_64");
    assert_eq!(passes(&odd), [&Pass::Struct(Layout { size: 20, align: 4 })]);
    accepted("x86_64", signature(Conv::C, vec![bytes(20, 4)]));
    // Six registers and a word on the stack leave a record aligned to 16 a word to skip.
    let mut params = ints();
    params.push(bytes(32, 16));
    let aligned = plan("x86_64", params.clone(), None).expect("x86_64");
    assert_eq!(aligned.params[7].skips, [Skip::Stack]);
    accepted("x86_64", signature(Conv::C, params));
}

#[test]
fn registers_a_convention_closes_are_taken_by_parameters_carrying_nothing() {
    let pair = record(&[(0, Scalar::I64), (8, Scalar::I64)], 16);
    // AAPCS64 closes the registers a record did not fit: the one left is skipped, and the
    // record's words reach the stack.
    let mut params = vec![AbiType::Int(Scalar::I64, vun_lir::Ext::Zero); 7];
    params.push(pair);
    params.push(AbiType::Int(Scalar::I64, vun_lir::Ext::Zero));
    let planned = plan("arm64", params.clone(), None).expect("arm64");
    assert_eq!(planned.params[7].skips, [Skip::Int]);
    assert_eq!(planned.params[7].pass, Pass::Words(Layout { size: 16, align: 8 }));
    // The integer after it is on the stack too, with no register left for it.
    assert_eq!(planned.params[8].skips, []);
    accepted("arm64", signature(Conv::C, params));
}
