//! The driver's integration tests, in one binary so that a change links it once: each module
//! runs whole sessions over fixture packages and the toolchain's `lang` and `std`.

mod common;

mod add;
mod asm;
mod backend;
mod bare;
mod bind;
mod borrows;
mod checks;
mod comptime;
mod coroutines;
mod doc;
mod evaluator;
mod fetch;
mod forms;
mod generics;
mod impls;
mod layout;
mod library;
mod link;
mod lir;
mod loops;
mod markers;
mod mir;
mod mutation;
mod namespaces;
mod narrowing;
mod patterns;
mod persist;
mod places;
mod plan;
mod quotes;
mod reduce;
mod reflection;
mod replace;
mod runtime;
mod select;
mod sequences;
mod staged;
mod stdlib;
mod typing;
mod update;
mod variadic;
mod visibility;
