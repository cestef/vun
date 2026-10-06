//! The table's invariants: opcodes are unique, and what a form pops and pushes is as its operands
//! allow, and a branch names its block.

use std::collections::BTreeSet;

use crate::{Control, Imm, Kind, Op, Ty};

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

#[test]
fn a_branch_takes_the_depth_of_its_block_and_a_block_is_of_no_type() {
    for op in Op::ALL {
        for form in op.forms() {
            let depth = [Kind::Imm(Imm::Unsigned(32))];
            let kinds: Vec<_> = form.args.iter().map(|arg| arg.kind).collect();
            match form.control {
                Control::Branch | Control::BranchIf => assert_eq!(kinds, depth, "{}", op.name()),
                Control::Open | Control::OpenIf => assert_eq!(form.tail, [0x40], "{}", op.name()),
                Control::Reopen | Control::Close | Control::None => {}
            }
        }
    }
}
