//! URIs as RFC 3986 defines them, and file URIs as RFC 8089 does.
//!
//! A reference is kept as written, with where each component lies: a scheme, an authority (user
//! information, a host that is a registered name, an IPv4 or IPv6 address or an `IPvFuture`, and
//! a port), a path, a query and a fragment, each checked against its grammar and read through a
//! typed view that decodes its escapes on request. Whether a scheme is required is in the type:
//! `Uri` has one, a `Reference` may be relative, and `Uri::resolve` reads one against the other
//! (section 5). `normalized` writes equivalent references alike (section 6), the builder escapes
//! each component as it requires, and a path of this system and its file URI convert both ways.
//!
//! ```
//! use vun_uri::{Reference, Uri};
//!
//! let base: Uri = "http://a/b/c/d;p?q".parse().unwrap();
//! let reference: Reference = "../g?x#s".parse().unwrap();
//! assert_eq!(base.resolve(&reference).as_str(), "http://a/b/g?x#s");
//! ```

mod authority;
mod builder;
mod chars;
mod compose;
mod error;
mod file;
mod json;
mod normalize;
mod parse;
mod parts;
mod path;
mod percent;
mod query;
mod resolve;
mod scheme;
#[cfg(test)]
mod tests;
mod uri;

pub use authority::{Authority, Host};
pub use builder::Builder;
pub use chars::Set;
pub use error::{Invalid, NotFile, Why};
pub use parts::HostKind;
pub use path::Path;
pub use percent::Encoded;
pub use query::Query;
pub use scheme::{Known, Scheme};
pub use uri::{Absolute, Any, Kind, Reference, Uri, UriOf};
