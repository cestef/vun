//! The front end as queries (spec 13, section 2): the driver sets the inputs, and a file's HIR,
//! its package's namespaces and scope, and its name checks are computed once and read by every
//! later stage.

mod input;
mod layout;
mod names;
mod namespace;
mod parsed;
mod scope;

pub use input::{DepData, FileBytes, FileRef, FileText, Package, PackageData, PackageId, Unloaded};
pub use layout::Namespace;
pub use names::{CheckedNames, FileNames, FileScope, Spreads};
pub use namespace::{Exports, Members, NamespaceAt, Unfound, Written};
pub use parsed::{Parsed, ParsedFile, Reported};
pub use scope::{Outer, Scope, ScopeData};

use vun_query::Db;

/// The queries of the front end whose memos outlive the process: the inputs and the parsed file
/// by their traces, since they are set or cheap to read again, and the rest with their values.
pub const PERSISTED: &[fn(&Db)] = &[
    Db::trace::<Package>,
    Db::trace::<FileText>,
    Db::trace::<FileBytes>,
    Db::trace::<Parsed>,
    Db::persist::<Reported>,
    Db::persist::<FileNames>,
    Db::persist::<Scope>,
    Db::persist::<NamespaceAt>,
    Db::persist::<Written>,
    Db::persist::<Members>,
    Db::persist::<Exports>,
];
