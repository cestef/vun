//! The network scheme of package downloads (spec 12, section 3): `https://`, which a program
//! installs into its `vun_fetch::Schemes`. It lives apart from `vun-fetch` so that the TLS stack
//! builds beside the compiler, not before it. Redirects are followed here, a hop at a time, each
//! target checked to be `https` before it is requested (decision 248).

use std::io::Read;
use std::time::{Duration, Instant};

use ureq::Agent;
use ureq::http::header::{CONTENT_LENGTH, LOCATION};
use vun_fetch::{Failure, Format, Scheme};
use vun_term::{Progress, Style, Text, Unit, Verb};
use vun_uri::{Known, Uri};

mod secure;
mod unfetched;

pub use secure::Secure;
pub use unfetched::Unfetched;

/// `https://`, read by [`get`].
pub struct Https;

impl Scheme for Https {
    fn name(&self) -> &str {
        Known::HTTPS.name
    }

    fn read(&self, uri: &Uri) -> Result<Vec<u8>, Failure> {
        get(uri)
    }
}

/// How long a whole download may take, redirects included, against a mirror that stalls.
const TIMEOUT: Duration = Duration::from_mins(10);

/// How much of a body is read at a time, between advances of the bar.
const CHUNK: usize = 16 << 10;

/// The redirects followed, as forges redirect an archive to where it is served.
const MAX_REDIRECTS: usize = 8;

/// The body at `url`, up to [`Format::MAX_ARCHIVE`] bytes, following up to [`MAX_REDIRECTS`]
/// redirects to `https` URLs; a status other than a success is refused.
fn get(url: &Uri) -> Result<Vec<u8>, Failure> {
    let download = |e: ureq::Error| Failure::Download(Box::new(e));
    let agent: Agent =
        Agent::config_builder().https_only(true).max_redirects(0).http_status_as_error(false).build().into();
    let deadline = Instant::now() + TIMEOUT;
    let mut at = Secure::of(url)?;
    for _ in 0..=MAX_REDIRECTS {
        let left = deadline.saturating_duration_since(Instant::now());
        let request = agent.get(at.uri().as_str()).config().timeout_global(Some(left)).build();
        let response = request.call().map_err(download)?;
        let status = response.status();
        if status.is_redirection() {
            let location = response.headers().get(LOCATION).ok_or(Unfetched::Unlocated)?;
            at = at.follow(location.as_bytes())?;
            continue;
        }
        if !status.is_success() {
            return Err(Unfetched::Refused(status).into());
        }
        let total = response.headers().get(CONTENT_LENGTH).and_then(|v| v.to_str().ok()?.parse().ok());
        let subject = Text::of(Style::Strong, at.uri().to_string());
        let bar =
            Progress::ambient().bar(Verb::good("Downloading"), subject, total.unwrap_or(0), Unit::Bytes);
        let mut body = response.into_body().into_with_config().limit(Format::MAX_ARCHIVE).reader();
        let (mut bytes, mut chunk) = (Vec::new(), [0_u8; CHUNK]);
        loop {
            let read = body.read(&mut chunk).map_err(|e| download(ureq::Error::Io(e)))?;
            if read == 0 {
                return Ok(bytes);
            }
            bytes.extend_from_slice(&chunk[..read]);
            bar.advance(u64::try_from(read).unwrap_or(0));
        }
    }
    Err(Unfetched::Endless.into())
}

#[cfg(test)]
mod tests;
