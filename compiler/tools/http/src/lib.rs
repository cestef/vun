//! The network schemes of package downloads (spec 12, section 3): `https://` and `http://`,
//! which a program installs into its `vun_pkg::Schemes`. They live apart from `vun-pkg` so
//! that the TLS stack builds beside the compiler, not before it.

use std::error::Error;
use std::fmt;
use std::ops::RangeInclusive;

use vun_pkg::{Failure, Format, Locator, Scheme};

/// `https://` and `http://`, read by [`get`].
pub const SCHEMES: &[Scheme] = &[
    Scheme { prefix: HTTPS, read: get, locator: Locator::Url },
    Scheme { prefix: "http://", read: get, locator: Locator::Url },
];

/// The seconds a whole download may take, against a mirror that stalls.
const TIMEOUT: u64 = 10 * 60;

/// The redirects followed, as forges redirect an archive to where it is served.
const MAX_REDIRECTS: usize = 8;

/// The secure scheme, which a redirect may not leave.
const HTTPS: &str = "https://";

/// The statuses of a successful response (RFC 9110, section 15.3).
const SUCCESS: RangeInclusive<i32> = 200..=299;

/// A response whose status is not a success.
#[derive(Debug)]
pub struct Refused {
    pub code: i32,
    pub reason: String,
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the server answered {} {}", self.code, self.reason)
    }
}

impl Error for Refused {}

/// A download over `https://` redirected to a URL that is not.
#[derive(Debug)]
pub struct Downgraded {
    pub to: String,
}

impl fmt::Display for Downgraded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "redirected from {HTTPS} to {}", self.to)
    }
}

impl Error for Downgraded {}

/// The body at `url`, up to [`Format::MAX_ARCHIVE`] bytes; a status other than a success, or a
/// redirect from `https://` to another scheme, is refused.
fn get(url: &str, _: &str) -> Result<Vec<u8>, Failure> {
    let limit = usize::try_from(Format::MAX_ARCHIVE).unwrap_or(usize::MAX);
    let request =
        bitreq::get(url).with_max_body_size(limit).with_timeout(TIMEOUT).with_max_redirects(MAX_REDIRECTS);
    let response = request.send().map_err(|e| Failure::Download(Box::new(e)))?;
    if url.starts_with(HTTPS) && !response.url.starts_with(HTTPS) {
        return Err(Failure::Download(Box::new(Downgraded { to: response.url })));
    }
    if !SUCCESS.contains(&response.status_code) {
        let refused = Refused { code: response.status_code, reason: response.reason_phrase };
        return Err(Failure::Download(Box::new(refused)));
    }
    Ok(response.into_bytes())
}
