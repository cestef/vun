//! Writing a reference from its components (RFC 3986, section 5.3), recording where each lies, so
//! that what resolution, normalization and the builder produce needs no parsing again.

use crate::parse::{AT, COLON, HASH, NETWORK, QUESTION, SLASH};
use crate::parts::{AuthorityParts, HostKind, Part, Parts};
use crate::uri::{Kind, UriOf};

/// The pieces of an authority to write, each as it is to be written.
#[derive(Clone, Copy)]
pub(crate) struct Pieces<'a> {
    pub(crate) userinfo: Option<&'a str>,
    pub(crate) host: &'a str,
    pub(crate) kind: HostKind,
    pub(crate) port: Option<&'a str>,
}

/// A reference written component by component, in order.
#[derive(Default)]
pub(crate) struct Composer {
    text: String,
    parts: Parts,
}

/// What keeps a path from reading as an authority, or its first segment as a scheme.
const ROOTED: &str = "/.";
const HERE: &str = "./";

impl Composer {
    fn written(&mut self, text: &str) -> Part {
        let start = self.text.len();
        self.text.push_str(text);
        Part::new(start, self.text.len())
    }

    fn push(&mut self, b: u8) {
        self.text.push(char::from(b));
    }

    pub(crate) fn scheme(&mut self, scheme: &str) {
        self.parts.scheme = Some(self.written(scheme));
        self.push(COLON);
    }

    pub(crate) fn authority(&mut self, pieces: Pieces<'_>) {
        self.text.push_str(NETWORK);
        let start = self.text.len();
        let userinfo = pieces.userinfo.map(|u| {
            let part = self.written(u);
            self.push(AT);
            part
        });
        let host = self.written(pieces.host);
        let port = pieces.port.map(|p| {
            self.push(COLON);
            self.written(p)
        });
        let whole = Part::new(start, self.text.len());
        self.parts.authority = Some(AuthorityParts { whole, userinfo, host, kind: pieces.kind, port });
    }

    /// Writes `path`. Without an authority, a path starting with `//` is written `/.//`, and
    /// without a scheme either, a first segment holding `:` is written after `./`, so that the
    /// text reads back as the same components.
    pub(crate) fn path(&mut self, path: &str) {
        let start = self.text.len();
        if self.parts.authority.is_none() {
            if path.starts_with(NETWORK) {
                self.text.push_str(ROOTED);
            } else if self.parts.scheme.is_none()
                && path.split(char::from(SLASH)).next().is_some_and(|s| s.contains(char::from(COLON)))
            {
                self.text.push_str(HERE);
            }
        }
        self.text.push_str(path);
        self.parts.path = Part::new(start, self.text.len());
    }

    pub(crate) fn query(&mut self, query: Option<&str>) {
        self.parts.query = query.map(|q| {
            self.push(QUESTION);
            self.written(q)
        });
    }

    pub(crate) fn fragment(&mut self, fragment: Option<&str>) {
        self.parts.fragment = fragment.map(|f| {
            self.push(HASH);
            self.written(f)
        });
    }

    pub(crate) fn finish<K: Kind>(self) -> UriOf<K> {
        debug_assert!(!K::SCHEME || self.parts.scheme.is_some(), "a URI is composed with its scheme");
        UriOf::from_parts(self.text, self.parts)
    }
}

impl<'a> Pieces<'a> {
    /// The pieces of the authority `parts` of `text`, as written there.
    pub(crate) fn of(text: &'a str, parts: &AuthorityParts) -> Self {
        Self {
            userinfo: parts.userinfo.map(|p| p.of(text)),
            host: parts.host.of(text),
            kind: parts.kind,
            port: parts.port.map(|p| p.of(text)),
        }
    }
}
