//! The driver's integration tests, in one binary so that a change links it once: each module
//! runs whole sessions over fixture packages and the toolchain's `lang` and `std`.

mod common;

mod add;
mod comptime;
mod fetch;
mod forms;
mod generics;
mod impls;
mod loops;
mod mir;
mod namespaces;
mod replace;
mod select;
mod sequences;
mod staged;
mod update;
mod visibility;
