//! The network scheme of package downloads (spec 12, section 3): `https://`, which a program
//! installs into its `vun_pkg::Schemes`. It lives apart from `vun-pkg` so that the TLS stack
//! builds beside the compiler, not before it. Redirects are followed here, a hop at a time, each
//! target checked to be `https` before it is requested (decision 248).

use std::time::{Duration, Instant};

use ureq::Agent;
use ureq::http::header::LOCATION;
use vun_pkg::{Failure, Format, Locator, Scheme};

mod secure;
mod unfetched;

pub use secure::Secure;
pub use unfetched::Unfetched;

/// `https://`, read by [`get`].
pub const SCHEMES: &[Scheme] = &[Scheme { prefix: Secure::PREFIX, read: get, locator: Locator::Url }];

/// How long a whole download may take, redirects included, against a mirror that stalls.
const TIMEOUT: Duration = Duration::from_mins(10);

/// The redirects followed, as forges redirect an archive to where it is served.
const MAX_REDIRECTS: usize = 8;

/// The body at `url`, up to [`Format::MAX_ARCHIVE`] bytes, following up to [`MAX_REDIRECTS`]
/// redirects to `https` URLs; a status other than a success is refused.
fn get(url: &str, _: &str) -> Result<Vec<u8>, Failure> {
    let download = |e: ureq::Error| Failure::Download(Box::new(e));
    let agent: Agent =
        Agent::config_builder().https_only(true).max_redirects(0).http_status_as_error(false).build().into();
    let deadline = Instant::now() + TIMEOUT;
    let mut at = Secure::parse(url)?;
    for _ in 0..=MAX_REDIRECTS {
        let left = deadline.saturating_duration_since(Instant::now());
        let request = agent.get(at.uri()).config().timeout_global(Some(left)).build();
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
        let body = response.into_body().into_with_config().limit(Format::MAX_ARCHIVE);
        return body.read_to_vec().map_err(download);
    }
    Err(Unfetched::Endless.into())
}

#[cfg(test)]
mod tests;
