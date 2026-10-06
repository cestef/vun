use std::fmt;
use std::sync::Arc;

use vun_component::Component;
use vun_fetch::{Failure, Mirror, Scheme, Schemes as Installed};
use vun_uri::Uri;

use crate::{Remote, Schemes};

/// Reads the path of the URL as its bytes.
struct Echo;

/// Reads nothing, for a reason that has a reason.
struct Refusing;

#[derive(Debug)]
struct Network;

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the network is down")
    }
}

impl std::error::Error for Network {}

#[derive(Debug)]
struct Refused(Network);

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the server refused")
    }
}

impl std::error::Error for Refused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl Scheme for Echo {
    fn name(&self) -> &'static str {
        "echo"
    }

    fn read(&self, uri: &Uri) -> Result<Vec<u8>, Failure> {
        Ok(uri.path().as_str().as_bytes().to_vec())
    }
}

impl Scheme for Refusing {
    fn name(&self) -> &'static str {
        "refusing"
    }

    fn read(&self, _: &Uri) -> Result<Vec<u8>, Failure> {
        Err(Failure::Download(Box::new(Refused(Network))))
    }
}

macro_rules! remote {
    ($stub:expr) => {
        Remote::of(Component::within(vun_component::descriptor!(Schemes, $stub)).unwrap()).unwrap()
    };
}

#[test]
fn a_scheme_component_is_read_through_like_one_linked_in() {
    let mut schemes = Installed::default();
    schemes.install(Arc::new(remote!(Echo)));
    let uri: Uri = "echo://host/a/b.tar.gz".parse().unwrap();
    assert_eq!(schemes.get(&uri).unwrap(), b"/a/b.tar.gz");
    assert!(Mirror::of(&uri, &schemes).is_ok());
}

#[test]
fn a_failed_read_keeps_the_chain_of_its_causes() {
    let remote = remote!(Refusing);
    assert_eq!(remote.name(), "refusing");
    let Err(Failure::Download(error)) = remote.read(&"refusing://x/y".parse().unwrap()) else {
        panic!("the read fails")
    };
    let mut chain = vec![error.to_string()];
    let mut next = error.source();
    while let Some(cause) = next {
        chain.push(cause.to_string());
        next = cause.source();
    }
    assert_eq!(chain, ["cannot download it", "the server refused", "the network is down"]);
}
