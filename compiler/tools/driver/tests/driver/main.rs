//! The driver's integration tests, in one binary so that a change links it once: each module
//! runs whole sessions over fixture packages and the toolchain's `lang` and `std`.

mod common;

mod add;
mod borrows;
mod comptime;
mod coroutines;
mod fetch;
mod forms;
mod generics;
mod impls;
mod layout;
mod loops;
mod mir;
mod namespaces;
mod places;
mod quotes;
mod reflection;
mod replace;
mod select;
mod sequences;
mod staged;
mod update;
mod visibility;
