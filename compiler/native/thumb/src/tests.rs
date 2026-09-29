//! Thumb code for LIR functions, each instruction checked against the table it was encoded by:
//! the processor compiled for has every instruction emitted, and what it lacks is refused.

use vun_asm_thumb::{Feature, Need, Size};
use vun_backend::{Lack, Missing};
use vun_codegen::Processor;
use vun_lir::{BinOp, CastOp, InstKind, Scalar, Terminator};
use vun_native::Native;
use vun_native::testing::{compile_for, text};
use vun_target::Floats;

use crate::Thumb2;

/// `n / (n + 1)`, the divisor kept from folding into a constant.
fn divided(processor: Processor<Thumb2>) -> Result<Vec<String>, Lack> {
    let code = compile_for::<Thumb2>(processor, Scalar::I32, |b, n, origin| {
        let one = b.constant(Scalar::I32, 1, origin);
        let divisor = b.binary(BinOp::Add, n, one, origin);
        let quotient = b.binary(BinOp::UDiv, n, divisor, origin);
        b.terminate(Terminator::Return(Some(quotient)));
    })?;
    Ok(text(&code))
}

/// `n`'s bits as a single, added to itself, as bits again.
fn added(processor: Processor<Thumb2>) -> Result<Vec<String>, Lack> {
    let code = compile_for::<Thumb2>(processor, Scalar::I32, |b, n, origin| {
        let cast = |b: &mut vun_lir::Builder, value, to| {
            let kind = InstKind::Cast { op: CastOp::Bitcast, value, to };
            b.push(kind, Some(to), origin).expect("a cast gives a value")
        };
        let single = cast(b, n, Scalar::F32);
        let sum = b.binary(BinOp::FAdd, single, single, origin);
        let bits = cast(b, sum, Scalar::I32);
        b.terminate(Terminator::Return(Some(bits)));
    })?;
    Ok(text(&code))
}

#[test]
fn division_is_an_instruction_where_the_processor_divides_and_refused_where_it_does_not() {
    let divides = Processor::<Thumb2>::new([Feature::Divide], Floats::default());
    let text = divided(divides).unwrap();
    assert!(text.iter().any(|i| i.starts_with("udiv")), "{text:#?}");
    let lacking = Processor::<Thumb2>::new([Feature::Application], Floats::default());
    let missing = Missing::Unavailable(Need::Feature(Feature::Divide).needed());
    assert_eq!(divided(lacking).map_err(|lack| lack.missing), Err(missing));
}

#[test]
fn a_single_is_computed_where_the_processor_has_a_floating_point_unit_and_refused_where_not() {
    let single = Floats::default().with(32).expect("a format");
    let text = added(Processor::<Thumb2>::new([], single)).unwrap();
    assert!(text.iter().any(|i| i.starts_with("vadd.f32")), "{text:#?}");
    let missing = Missing::Unavailable(Need::Float(Size::B32).needed());
    assert_eq!(
        added(Processor::<Thumb2>::new([], Floats::default())).map_err(|lack| lack.missing),
        Err(missing)
    );
}

#[test]
fn integers_are_divided_through_legalization_s_helper_where_the_processor_has_no_division() {
    let divides = Processor::<Thumb2>::new([Feature::Divide], Floats::default());
    assert!(Thumb2::divides(divides));
    assert!(!Thumb2::divides(Processor::<Thumb2>::new([Feature::Application], Floats::default())));
}
