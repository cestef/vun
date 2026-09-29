//! Reading a URI reference (RFC 3986, section 4.1): its components split as appendix B splits
//! them, then each checked against its grammar.

use std::net::{Ipv4Addr, Ipv6Addr};

use crate::parts::MAX_LEN;

use crate::chars::Set;
use crate::error::{Invalid, Why};
use crate::parts::{AuthorityParts, HostKind, Part, Parts};

/// What starts an authority, after the scheme.
pub(crate) const NETWORK: &str = "//";
pub(crate) const SLASH: u8 = b'/';
pub(crate) const QUESTION: u8 = b'?';
pub(crate) const HASH: u8 = b'#';
pub(crate) const COLON: u8 = b':';
pub(crate) const AT: u8 = b'@';
const OPEN: u8 = b'[';
const CLOSE: u8 = b']';
/// What starts an `IPvFuture` literal.
const FUTURE: &[u8] = b"vV";

/// The position of the first of `stops` in `bytes` from `from`, or the end.
fn until(bytes: &[u8], from: usize, stops: &[u8]) -> usize {
    bytes[from..].iter().position(|b| stops.contains(b)).map_or(bytes.len(), |p| from + p)
}

impl Parts {
    pub(crate) fn parse(text: &str) -> Result<Self, Invalid> {
        let bytes = text.as_bytes();
        if bytes.len() > MAX_LEN {
            return Err(Invalid::new(Why::Length, MAX_LEN));
        }
        let mut parts = Self::default();
        let mut i = 0;
        // A `:` before any `/`, `?` or `#` ends a scheme; a relative reference has none there.
        let first = until(bytes, 0, &[COLON, SLASH, QUESTION, HASH]);
        if bytes.get(first) == Some(&COLON) {
            if !bytes[0].is_ascii_alphabetic() {
                return Err(Invalid::new(Why::Scheme, 0));
            }
            Set::Scheme.check(&text[..first])?;
            parts.scheme = Some(Part::new(0, first));
            i = first + 1;
        }
        if text[i..].starts_with(NETWORK) {
            let start = i + NETWORK.len();
            let end = until(bytes, start, &[SLASH, QUESTION, HASH]);
            parts.authority = Some(AuthorityParts::parse(text, Part::new(start, end))?);
            i = end;
        }
        let end = until(bytes, i, &[QUESTION, HASH]);
        parts.path = Part::new(i, end);
        Set::Path.check(parts.path.of(text)).map_err(|e| e.shifted(i))?;
        i = end;
        if bytes.get(i) == Some(&QUESTION) {
            let end = until(bytes, i + 1, &[HASH]);
            parts.query = Some(Part::new(i + 1, end));
            Set::Query.check(&text[i + 1..end]).map_err(|e| e.shifted(i + 1))?;
            i = end;
        }
        if bytes.get(i) == Some(&HASH) {
            parts.fragment = Some(Part::new(i + 1, bytes.len()));
            Set::Fragment.check(&text[i + 1..]).map_err(|e| e.shifted(i + 1))?;
        }
        Ok(parts)
    }
}

impl AuthorityParts {
    /// The authority `whole` of `text`: `[userinfo@]host[:port]`.
    fn parse(text: &str, whole: Part) -> Result<Self, Invalid> {
        let bytes = text.as_bytes();
        let within = &bytes[whole.start()..whole.end()];
        let (userinfo, host_start) = match within.iter().rposition(|&b| b == AT) {
            Some(at) => {
                let userinfo = Part::new(whole.start(), whole.start() + at);
                Set::UserInfo.check(userinfo.of(text)).map_err(|e| e.shifted(userinfo.start()))?;
                (Some(userinfo), whole.start() + at + 1)
            }
            None => (None, whole.start()),
        };
        let (host, kind, rest) = if bytes.get(host_start) == Some(&OPEN) && host_start < whole.end() {
            let close = bytes[host_start..whole.end()].iter().position(|&b| b == CLOSE);
            let close = close.ok_or(Invalid::new(Why::Bracket, host_start))? + host_start;
            let kind = HostKind::literal(&text[host_start + 1..close])
                .ok_or(Invalid::new(Why::IpLiteral, host_start))?;
            let rest = close + 1;
            if rest < whole.end() && bytes[rest] != COLON {
                return Err(Invalid::new(Why::Bracket, rest));
            }
            (Part::new(host_start, rest), kind, rest)
        } else {
            let colon = bytes[host_start..whole.end()].iter().rposition(|&b| b == COLON);
            let end = colon.map_or(whole.end(), |c| host_start + c);
            let host = Part::new(host_start, end);
            (host, HostKind::named(host.of(text)).map_err(|e| e.shifted(host_start))?, end)
        };
        let port = if rest < whole.end() {
            let digits = Part::new(rest + 1, whole.end());
            let written = digits.of(text);
            let fits = written.is_empty() || written.parse::<u16>().is_ok();
            if !written.bytes().all(|b| b.is_ascii_digit()) || !fits {
                return Err(Invalid::new(Why::Port, digits.start()));
            }
            Some(digits)
        } else {
            None
        };
        Ok(Self { whole, userinfo, host, kind, port })
    }
}

impl HostKind {
    /// What the inside of an IP literal `[...]` holds.
    pub(crate) fn literal(inside: &str) -> Option<Self> {
        let bytes = inside.as_bytes();
        if bytes.first().is_some_and(|b| FUTURE.contains(b)) {
            // `v` 1*HEXDIG `.` 1*( unreserved / sub-delims / `:` )
            let dot = bytes.iter().position(|&b| b == b'.')?;
            let version = &bytes[1..dot];
            let address = &inside[dot + 1..];
            let valid = !version.is_empty()
                && version.iter().all(u8::is_ascii_hexdigit)
                && !address.is_empty()
                && address.bytes().all(|b| b == COLON || (b != b'%' && Set::Host.allows(b)));
            return valid.then_some(Self::Future);
        }
        inside.parse::<Ipv6Addr>().ok().map(|_| Self::Ipv6)
    }

    /// What a host written without brackets is: an IPv4 address in dotted decimal, else a
    /// registered name.
    pub(crate) fn named(host: &str) -> Result<Self, Invalid> {
        // The standard parser takes exactly RFC 3986's `dec-octet`s: four, without leading zeros.
        if host.parse::<Ipv4Addr>().is_ok() {
            return Ok(Self::Ipv4);
        }
        Set::Host.check(host)?;
        Ok(Self::Name)
    }
}
