use std::path::PathBuf;

use vun_component::Component;

use crate::{
    Binder, Binders, Bindings, Failure, Missing, Reason, Remote, Request, Skipped, Unavailable, Unsupported,
};

/// Binds every header as one declaration, except the one that is missing.
struct Stub;

impl Binder for Stub {
    fn name(&self) -> &'static str {
        "stub"
    }

    fn bind(&self, request: &Request) -> Result<Bindings, Failure> {
        if request.header.ends_with("missing.h") {
            return Err(Failure::Missing(Missing { oldest: 9, why: Unavailable::Old }));
        }
        let skipped = Skipped {
            name: request.reserved.join(","),
            file: request.header.clone(),
            line: 3,
            reason: Reason::Unsupported(Unsupported::Type(request.args.join(" "))),
        };
        Ok(Bindings { source: "a := 1;".to_owned(), skipped: vec![skipped] })
    }
}

fn remote() -> Remote {
    Remote::of(Component::within(vun_component::descriptor!(Binders, Stub)).unwrap()).unwrap()
}

fn request(header: &str) -> Request {
    Request {
        header: PathBuf::from(header),
        args: vec!["-Iinclude".to_owned(), "-DREV=2".to_owned()],
        reserved: vec!["type".to_owned(), "test".to_owned()],
    }
}

#[test]
fn bindings_cross_the_boundary_with_what_was_left_out() {
    let remote = remote();
    assert_eq!(remote.name(), "stub");
    let request = request("board.h");
    assert_eq!(remote.bind(&request), Stub.bind(&request));
}

#[test]
fn a_failure_of_the_binder_arrives_as_it_was() {
    assert_eq!(
        remote().bind(&request("missing.h")),
        Err(Failure::Missing(Missing { oldest: 9, why: Unavailable::Old }))
    );
}
