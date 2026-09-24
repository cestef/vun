//! Comparisons narrow (spec 04, rule 4): an immutable integer binding has the range a
//! comparison known to hold leaves it, so counting up to a bound needs no wider type.

use vun_diag::errors;

use crate::common::Fixture;

/// The ids reported checking `app` as a file of its own, in source order.
fn reported(name: &str, app: &str) -> Vec<vun_diag::DiagId> {
    let fixture = Fixture::new(name);
    let file = fixture.root.write("app.vun", app);
    fixture.reported(&file).into_iter().map(|(id, _)| id).collect()
}

#[test]
fn branches_loops_arms_and_connectives_narrow_immutable_bindings() {
    let app = "\
branch := (k: usize, n: usize) -> usize { if k < n { k + 1 } else { 0 } };
other := (k: u8) -> u8 { if k >= 255 { 0 } else { k + 1 } };
both := (k: u8) -> u8 { if k != 255 and k > 3 { k + 1 } else { 0 } };
either := (k: u8) -> u8 { if k == 255 or k < 3 { 0 } else { k + 1 } };
negated := (k: u8) -> u8 { if !(k == 255) { k + 1 } else { 0 } };
early := (k: usize, n: usize) -> usize { if k >= n { return 0; } k + 1 };
guarded := (k: u8) -> u8 { match k { x if x < 9 => x + 1, _ => 0 } };
looped := (k: u8) -> u8 { for k < 200 { return k + 1; } 0 };
";
    assert_eq!(reported("narrowing", app), []);
}

#[test]
fn mutable_bindings_and_comparisons_that_may_fail_do_not_narrow() {
    let app = "\
wide := (k: usize, n: usize) -> usize { if k <= n { k + 1 } else { 0 } };
changing := (k: u8) -> u8 { mut j := k; if j < 3 { j + 253 } else { 0 } };
after := (k: u8) -> u8 { if k < 3 { return 0; } k + 1 };
";
    assert_eq!(reported("narrowing-not", app), [errors::INT_DOES_NOT_FIT; 3]);
}
