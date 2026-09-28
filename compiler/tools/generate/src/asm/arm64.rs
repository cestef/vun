//! `vun-asm-arm64`'s tables, from `lang.arch.arm64`: its forms as `table::Form` values.

use super::emit::Crate;
use super::{ARCH, Table, words};
use crate::generate::{Generator, Module, Read, Source};

pub(crate) const ARM64: Generator =
    Generator { source: ARCH, output: "compiler/isa/arm64/src/generated", emit };

const CRATE: Crate = Crate {
    generator: &ARM64,
    arch: "arm64",
    types: &[
        ("Size", "reg", "Size"),
        ("Reg", "reg", "Reg"),
        ("Kind", "table", "Kind"),
        ("Access", "table", "Access"),
        ("R31", "table", "R31"),
        ("Width", "table", "Width"),
        ("Imm", "table", "Imm"),
        ("Mode", "table", "Mode"),
        ("Shifts", "table", "Shifts"),
        ("Condition", "table", "Cond"),
        ("Branch", "table", "Branch"),
    ],
    widths: &["w", "x"],
    enums: &[],
    payloads: &[
        ("Kind", "gpr", &["R31", "Width"]),
        ("Kind", "fpr", &["Width"]),
        ("Kind", "imm", &["Imm"]),
        ("Kind", "shifted", &["Shifts"]),
        ("Kind", "mem", &["Mode", "Width"]),
        ("Kind", "cond", &["Condition"]),
        ("Kind", "target", &["Branch"]),
        ("Width", "is", &["Size"]),
    ],
};

fn emit(sources: &[Source<'_>]) -> Read<Vec<Module>> {
    CRATE.modules(&Table::read(&ARM64, CRATE.arch, sources, words::fill)?, words::form)
}
