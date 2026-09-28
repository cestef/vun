//! `vun-isa-thumb`'s tables, from `lang.arch.thumb`: its forms as `table::Form` values, words of
//! fixed fields as arm64's are, and the table's plain enums as they are.

use super::emit::Crate;
use super::{ARCH, Table, words};
use crate::generate::{Generator, Module, Read, Source};

pub(crate) const THUMB: Generator =
    Generator { source: ARCH, output: "compiler/base/thumb/src/generated", emit };

const CRATE: Crate = Crate {
    generator: &THUMB,
    arch: "thumb",
    types: &[
        ("Size", "reg", "Size"),
        ("Reg", "reg", "Reg"),
        ("Kind", "table", "Kind"),
        ("Access", "table", "Access"),
        ("Width", "table", "Width"),
        ("Regs", "table", "Regs"),
        ("Imm", "table", "Imm"),
        ("Place", "table", "Place"),
        ("Mode", "table", "Mode"),
        ("Fields", "table", "Fields"),
        ("List", "table", "List"),
        ("Branch", "table", "Branch"),
        ("Status", "reg", "Status"),
    ],
    widths: &[],
    enums: &["Size", "V", "Special", "Status", "Mask", "Barrier", "Regs", "Place", "Mode", "Fields", "List", "Branch"],
    payloads: &[
        ("Kind", "gpr", &["Regs"]),
        ("Kind", "fpr", &["Width", "Fields"]),
        ("Kind", "imm", &["Imm"]),
        ("Kind", "mem", &["Mode", "Width"]),
        ("Kind", "target", &["Branch"]),
        ("Kind", "list", &["List"]),
        ("Kind", "status", &["Status"]),
        ("Imm", "unsigned", &["u8"]),
        ("Imm", "words", &["u8"]),
        ("Imm", "lsl", &["Place"]),
        ("Imm", "shr", &["Place"]),
        ("Width", "is", &["Size"]),
    ],
};

fn emit(sources: &[Source<'_>]) -> Read<Vec<Module>> {
    CRATE.modules(&Table::read(&THUMB, CRATE.arch, sources, words::fill)?, words::form)
}
