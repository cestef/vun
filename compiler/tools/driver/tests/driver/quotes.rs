//! Code values: quotes, `Code(T)` parameters and splices (spec 03, section 4).

use crate::common::Fixture;
use vun_diag::errors;

#[test]
fn quotes_and_code_arguments_splice_where_they_were_written() {
    let fixture = Fixture::new("quotes");
    let file = fixture.root.write(
        "quotes.vun",
        "\
square := ($x: Code(u32)) -> Code(u64) { #{ $x * $x } };
pick := ($cond: Code(bool)) -> u8 { if $cond { 1 } else { 2 } };
nine: Int(0..=$square(3)) = 10;
one: Int(0..=pick(2 > 1)) = 2;
typed: bool = $square(3);
argument := () -> u8 { pick(3) };
",
    );
    let found =
        |expected: &str, ty: &str| (errors::TYPES_MISMATCH, format!("expected `{expected}`, found `{ty}`"));
    let fit = |ty: &str| (errors::INT_DOES_NOT_FIT, format!("value does not fit the type `{ty}`"));
    assert_eq!(
        fixture.reported(&file),
        [
            fit("Int(0..=$square(3))"),
            fit("Int(0..=pick(2 > 1))"),
            found("bool", "Int(0..2^64)"),
            found("bool", "Int(3..=3)"),
        ]
    );
}

#[test]
fn code_values_give_their_source() {
    let fixture = Fixture::new("quotes-source");
    let file = fixture.root.write(
        "source.vun",
        "\
width := ($c: Code(u32)) -> usize { c.source().len() };
five: Int(0..=width(1 + 2)) = 6;
",
    );
    let reported: Vec<_> = fixture.reported(&file).into_iter().map(|(id, _)| id).collect();
    assert_eq!(reported, [errors::INT_DOES_NOT_FIT]);
}
