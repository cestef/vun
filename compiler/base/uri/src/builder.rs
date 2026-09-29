//! Building a reference from its components as text, each escaped as its component requires:
//! `Uri::builder("https").host("example.org").segment("a b").pair("q", "x&y").build()`.

use std::marker::PhantomData;
use std::net::IpAddr;

use crate::chars::Set;
use crate::compose::{Composer, Pieces};
use crate::error::{Invalid, Why};
use crate::parse::SLASH;
use crate::parts::HostKind;
use crate::uri::{Absolute, Any, Kind, Reference, Uri, UriOf};

/// A reference being built; the first invalid component is reported by `build`.
#[must_use]
pub struct Builder<K: Kind> {
    scheme: Option<String>,
    authority: Option<Authority>,
    path: String,
    query: Option<String>,
    fragment: Option<String>,
    invalid: Option<Invalid>,
    kind: PhantomData<fn() -> K>,
}

#[derive(Default)]
struct Authority {
    userinfo: Option<String>,
    host: String,
    kind: Option<HostKind>,
    port: Option<String>,
}

/// What separates a query's pairs, and a pair's name from its value.
const PAIRS: char = '&';
const VALUE: char = '=';

impl Uri {
    /// A URI of `scheme`, to build.
    pub fn builder(scheme: &str) -> Builder<Absolute> {
        Builder::new().scheme(scheme)
    }
}

impl Reference {
    /// A reference, relative until given a scheme, to build.
    pub fn builder() -> Builder<Any> {
        Builder::new()
    }
}

impl<K: Kind> Builder<K> {
    fn new() -> Self {
        Self {
            scheme: None,
            authority: None,
            path: String::new(),
            query: None,
            fragment: None,
            invalid: None,
            kind: PhantomData,
        }
    }

    fn fail(mut self, invalid: Invalid) -> Self {
        self.invalid.get_or_insert(invalid);
        self
    }

    /// The scheme, which is never escaped: letters, digits, `+`, `-` and `.`, a letter first.
    pub fn scheme(mut self, scheme: &str) -> Self {
        let valid = scheme.bytes().next().is_some_and(|b| b.is_ascii_alphabetic());
        if let Err(invalid) = Set::Scheme.check(scheme) {
            return self.fail(invalid);
        }
        if !valid {
            return self.fail(Invalid::new(Why::Scheme, 0));
        }
        self.scheme = Some(scheme.to_owned());
        self
    }

    pub fn userinfo(mut self, userinfo: &str) -> Self {
        self.authority.get_or_insert_default().userinfo =
            Some(Set::UserInfo.encode(userinfo.as_bytes()).into_owned());
        self
    }

    /// The host: an IP address, written in brackets when it is IPv6, or a registered name.
    pub fn host(mut self, host: &str) -> Self {
        let authority = self.authority.get_or_insert_default();
        match host.parse::<IpAddr>() {
            Ok(IpAddr::V6(address)) => {
                authority.host = format!("[{address}]");
                authority.kind = Some(HostKind::Ipv6);
            }
            Ok(IpAddr::V4(address)) => {
                authority.host = address.to_string();
                authority.kind = Some(HostKind::Ipv4);
            }
            Err(_) => {
                authority.host = Set::Host.encode(host.as_bytes()).into_owned();
                authority.kind = Some(HostKind::Name);
            }
        }
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.authority.get_or_insert_default().port = Some(port.to_string());
        self
    }

    /// Appends `/` and `segment`, its `/` escaped with the rest.
    pub fn segment(mut self, segment: &str) -> Self {
        self.path.push(char::from(SLASH));
        Set::Segment.encode_into(segment.as_bytes(), &mut self.path);
        self
    }

    /// The whole path, its `/` separating segments.
    pub fn path(mut self, path: &str) -> Self {
        self.path.clear();
        Set::Path.encode_into(path.as_bytes(), &mut self.path);
        self
    }

    /// The whole query.
    pub fn query(mut self, query: &str) -> Self {
        self.query = Some(Set::Query.encode(query.as_bytes()).into_owned());
        self
    }

    /// Appends the pair `name=value` to the query, each escaped as a pair's parts are.
    pub fn pair(mut self, name: &str, value: &str) -> Self {
        let query = self.query.get_or_insert_default();
        if !query.is_empty() {
            query.push(PAIRS);
        }
        Set::Pair.encode_into(name.as_bytes(), query);
        query.push(VALUE);
        Set::Pair.encode_into(value.as_bytes(), query);
        self
    }

    pub fn fragment(mut self, fragment: &str) -> Self {
        self.fragment = Some(Set::Fragment.encode(fragment.as_bytes()).into_owned());
        self
    }

    /// The reference built.
    ///
    /// # Errors
    /// The first invalid component; for a `Uri`, a missing scheme; a path not starting with `/`
    /// after an authority.
    pub fn build(self) -> Result<UriOf<K>, Invalid> {
        if let Some(invalid) = self.invalid {
            return Err(invalid);
        }
        if K::SCHEME && self.scheme.is_none() {
            return Err(Invalid::new(Why::Relative, 0));
        }
        let mut out = Composer::default();
        if let Some(scheme) = &self.scheme {
            out.scheme(scheme);
        }
        if let Some(a) = &self.authority {
            if !self.path.is_empty() && !self.path.starts_with(char::from(SLASH)) {
                return Err(Invalid::new(Why::AuthorityPath, 0));
            }
            let kind = a.kind.unwrap_or(HostKind::Name);
            out.authority(Pieces {
                userinfo: a.userinfo.as_deref(),
                host: &a.host,
                kind,
                port: a.port.as_deref(),
            });
        }
        out.path(&self.path);
        out.query(self.query.as_deref());
        out.fragment(self.fragment.as_deref());
        Ok(out.finish())
    }
}

impl<K: Kind> UriOf<K> {
    /// This reference with `segments` appended to its path, each escaped as a segment and after
    /// a `/`, a `/` ending the path taken as the one before the first; the query is kept, the
    /// fragment, which names a part of the old resource, dropped.
    #[must_use]
    pub fn with_segments<S: AsRef<str>>(&self, segments: impl IntoIterator<Item = S>) -> Self {
        let mut path =
            self.path().as_str().strip_suffix(char::from(SLASH)).unwrap_or(self.path().as_str()).to_owned();
        for segment in segments {
            path.push(char::from(SLASH));
            Set::Segment.encode_into(segment.as_ref().as_bytes(), &mut path);
        }
        let mut out = Composer::default();
        let text = self.as_str();
        if let Some(scheme) = self.parts().scheme {
            out.scheme(scheme.of(text));
        }
        if let Some(authority) = &self.parts().authority {
            out.authority(Pieces::of(text, authority));
        }
        out.path(&path);
        out.query(self.query().map(crate::query::Query::as_str));
        out.finish()
    }
}
