//! The network schemes of package downloads (spec 12, section 3): `https://` and `http://`,
//! which a program installs into its `overt_pkg::Schemes`. They live apart from `overt-pkg` so
//! that the TLS stack builds beside the compiler, not before it.

use std::error::Error;
use std::fmt;
use std::ops::RangeInclusive;

use overt_pkg::{Failure, Scheme};

/// `https://` and `http://`, read by [`get`].
pub const SCHEMES: &[Scheme] =
    &[Scheme { prefix: "https://", read: get }, Scheme { prefix: "http://", read: get }];

/// The largest body read, against a mirror serving without end.
const MAX_BODY: u64 = 4 << 30;

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

/// The body at `url`, up to [`MAX_BODY`] bytes; a status other than a success is refused.
fn get(url: &str, _: &str) -> Result<Vec<u8>, Failure> {
    let limit = usize::try_from(MAX_BODY).unwrap_or(usize::MAX);
    let request = bitreq::get(url).with_max_body_size(limit);
    let response = request.send().map_err(|e| Failure::Download(Box::new(e)))?;
    if !SUCCESS.contains(&response.status_code) {
        let refused = Refused { code: response.status_code, reason: response.reason_phrase };
        return Err(Failure::Download(Box::new(refused)));
    }
    Ok(response.into_bytes())
}
