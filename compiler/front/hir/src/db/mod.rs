//! The front end as queries (spec 13, section 2): the driver sets the inputs, and a file's HIR,
//! its package's namespaces and scope, and its name checks are computed once and read by every
//! later stage.

mod input;
mod layout;
mod names;
mod namespace;
mod parsed;
mod scope;

pub use input::{DepData, FileRef, FileText, Package, PackageData, PackageId, Unloaded};
pub use layout::Namespace;
pub use names::{CheckedNames, FileNames, FileScope, Spreads};
pub use namespace::{Exports, Members, NamespaceAt, Unfound};
pub use parsed::{Parsed, ParsedFile};
pub use scope::{Outer, Scope, ScopeData};
