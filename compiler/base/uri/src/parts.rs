//! Where each component lies in a URI's text: the offsets parsing records, which every view reads.

/// The longest text a reference may be, so that offsets fit `u32`.
pub(crate) const MAX_LEN: usize = u32::MAX as usize;

/// The bytes `start..end` of the text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct Part {
    start: u32,
    end: u32,
}

impl Part {
    pub(crate) fn new(start: usize, end: usize) -> Self {
        let offset = |n| u32::try_from(n).expect("a reference is at most `MAX_LEN` bytes, as parsing checks");
        Self { start: offset(start), end: offset(end) }
    }

    pub(crate) const fn start(self) -> usize {
        self.start as usize
    }

    pub(crate) const fn end(self) -> usize {
        self.end as usize
    }

    pub(crate) fn of(self, text: &str) -> &str {
        &text[self.start()..self.end()]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct Parts {
    pub(crate) scheme: Option<Part>,
    pub(crate) authority: Option<AuthorityParts>,
    pub(crate) path: Part,
    pub(crate) query: Option<Part>,
    pub(crate) fragment: Option<Part>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct AuthorityParts {
    /// The authority without its `//`.
    pub(crate) whole: Part,
    pub(crate) userinfo: Option<Part>,
    /// The host as written, brackets included.
    pub(crate) host: Part,
    pub(crate) kind: HostKind,
    /// The port's digits after its `:`, possibly none.
    pub(crate) port: Option<Part>,
}

/// What a host is (RFC 3986, section 3.2.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HostKind {
    /// A registered name, such as a DNS name; possibly empty.
    Name,
    Ipv4,
    /// An IPv6 address, in brackets.
    Ipv6,
    /// An IP literal of a later version, `[v1.x]`.
    Future,
}
