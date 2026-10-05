//! The table's invariants: opcodes are unique, and what a form pops and pushes is as its operands
//! allow.

use std::collections::BTreeSet;

use crate::{Kind, Op, Ty};

#[test]
fn opcodes_are_unique() {
    let mut seen = BTreeSet::new();
    for op in Op::ALL {
        for form in op.forms() {
            assert!(seen.insert(form.base), "{} repeats the opcode {:#x}", op.name(), form.base);
        }
    }
}

#[test]
fn a_form_popping_or_pushing_its_local_binds_one() {
    for op in Op::ALL {
        for form in op.forms() {
            let uses = form.pops.iter().chain(form.pushes).any(|&ty| ty == Ty::Local);
            let binds = form.args.iter().any(|arg| arg.kind == Kind::Local);
            assert_eq!(uses, binds, "{}", op.name());
        }
    }
}
