//! The driver's integration tests, in one binary so that a change links it once: each module
//! runs whole sessions over fixture packages and the toolchain's `lang` and `std`.

mod common;

mod add;
mod backend;
mod bind;
mod borrows;
mod comptime;
mod coroutines;
mod fetch;
mod forms;
mod generics;
mod impls;
mod layout;
mod library;
mod lir;
mod loops;
mod markers;
mod mir;
mod mutation;
mod namespaces;
mod places;
mod plan;
mod quotes;
mod reflection;
mod replace;
mod select;
mod sequences;
mod staged;
mod stdlib;
mod typing;
mod update;
mod visibility;
