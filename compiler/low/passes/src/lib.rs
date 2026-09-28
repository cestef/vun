//! LIR transforms (spec 13, section 3): legalization for a target, and the passes simplifying a
//! unit after lowering. Each is an extension trait of the LIR type it rewrites.

mod fold;
mod hoist;
pub mod inline;
pub mod legalize;
mod narrow;
pub mod promote;
mod replaced;
mod reuse;
pub mod scalarize;
mod simplify;
mod sweep;
pub mod thread;

pub use fold::Fold;
pub use hoist::Hoist;
pub use inline::{Inline, InlineCalls};
pub use legalize::Legalize;
pub use narrow::Narrow;
pub use promote::Promote;
pub use reuse::Reuse;
pub use scalarize::Scalarize;
pub use simplify::{Simplify, Tidy};
pub use sweep::Sweep;
pub use thread::Thread;
