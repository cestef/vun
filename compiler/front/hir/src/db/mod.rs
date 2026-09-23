//! The front end as queries (spec 13, section 2): the driver sets the inputs, and a file's HIR
//! and resolution are computed once per text and read by every later stage.

mod input;
mod parsed;

pub use input::{DepData, FileRef, FileText, Package, PackageData, PackageId, Unloaded};
pub use parsed::{Parsed, ParsedFile};
