//! Normalization (RFC 3986, section 6.2): equivalent references written the same way.
//!
//! Syntax-based (6.2.2): the scheme and a host name in lowercase, escapes in uppercase and those
//! of unreserved characters decoded, dot segments removed. An IPv6 host is written in its RFC 5952
//! form. Scheme-based (6.2.3), for the schemes of `Known`: a port equal to the scheme's is left
//! out, as is an empty one, and an empty path after an authority is `/` when the scheme's paths
//! are rooted.

use crate::chars::Set;
use crate::compose::{Composer, Pieces};
use crate::parts::HostKind;
use crate::path::Path;
use crate::percent::Encoded;
use crate::scheme::Scheme;
use crate::uri::{Kind, UriOf};

/// The root path, which an empty one stands for after an authority under a rooted scheme.
const ROOT: &str = "/";

impl<K: Kind> UriOf<K> {
    /// This reference in its normal form: equal to every reference equivalent to it.
    #[must_use]
    pub fn normalized(&self) -> Self {
        let mut out = Composer::default();
        let known = self.parts().scheme.and_then(|p| Scheme(p.of(self.as_str())).known());
        let scheme = self.parts().scheme.map(|p| p.of(self.as_str()).to_ascii_lowercase());
        if let Some(scheme) = &scheme {
            out.scheme(scheme);
        }
        let normal = |set: Set, text: &str| {
            let mut out = String::with_capacity(text.len());
            set.normalize_into(text, &mut out);
            out
        };
        if let Some(authority) = self.authority() {
            let userinfo = authority.userinfo().map(|u| normal(Set::UserInfo, u.as_str()));
            let host = authority.host();
            let host = match host.kind() {
                HostKind::Ipv6 => host.ip().map_or_else(|| host.as_str().to_owned(), |a| format!("[{a}]")),
                HostKind::Ipv4 => host.as_str().to_owned(),
                HostKind::Name | HostKind::Future => {
                    Encoded(&normal(Set::Host, host.as_str())).to_ascii_lowercase()
                }
            };
            let default = known.and_then(|k| k.port);
            let port = authority.port_str().filter(|p| !p.is_empty() && authority.port() != default);
            out.authority(Pieces {
                userinfo: userinfo.as_deref(),
                host: &host,
                kind: authority.host().kind(),
                port,
            });
        }
        let path = normal(Set::Path, self.path().as_str());
        let mut path = Path::remove_dots(&path).into_owned();
        if path.is_empty() && self.authority().is_some() && known.is_some_and(|k| k.rooted) {
            path.push_str(ROOT);
        }
        out.path(&path);
        let query = self.query().map(|q| normal(Set::Query, q.as_str()));
        out.query(query.as_deref());
        let fragment = self.fragment().map(|f| normal(Set::Fragment, f.as_str()));
        out.fragment(fragment.as_deref());
        out.finish()
    }

    /// Whether this reference and `other` are equivalent: equal once normalized.
    #[must_use]
    pub fn equivalent(&self, other: &Self) -> bool {
        self == other || self.normalized() == other.normalized()
    }
}
