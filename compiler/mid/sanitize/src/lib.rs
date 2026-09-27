//! Checks of unsafe code at run time (spec 07, section 7; spec 13, section 8.7), past what sema
//! knows of them (`vun_sema::checks`): the link symbols of the operations a runtime provides for
//! them (`Operations`).

mod operations;

pub use operations::Operations;

/// The queries of the checks whose memos outlive the process.
pub const PERSISTED: &[fn(&vun_query::Db)] = &[vun_query::Db::persist::<Operations>];
