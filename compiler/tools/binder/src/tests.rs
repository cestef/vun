use std::path::PathBuf;

use vun_component::Component;

use crate::{
    Binder, Binders, Bindings, Failure, Glob, Library, Missing, Reason, Remote, Request, Skipped,
    Unavailable, Unsupported,
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
        scope: vec![PathBuf::from("/usr/include/glib-2.0")],
        only: vec![Glob::new("gtk_*")],
        libraries: vec![Library { name: "gtk-4".to_owned(), headers: None }],
        ..Request::new(
            PathBuf::from(header),
            vec!["-Iinclude".to_owned(), "-DREV=2".to_owned()],
            vec!["type".to_owned(), "test".to_owned()],
        )
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

#[test]
fn a_glob_matches_the_whole_name_with_stars_for_runs() {
    let matches = |pattern: &str, name: &str| Glob::new(pattern).matches(name);
    assert!(matches("gtk_window_new", "gtk_window_new"));
    assert!(!matches("gtk_window_new", "gtk_window_new2"));
    assert!(matches("gtk_*", "gtk_window_new"));
    assert!(matches("*_new", "gtk_window_new"));
    assert!(!matches("*_new", "gtk_window_new_with"));
    assert!(matches("gtk_*_new*", "gtk_window_new_with"));
    assert!(matches("*", ""));
    assert!(!matches("a*b*c", "acb"));
    assert!(matches("a*b*c", "aXbYc"));
    assert!(matches("a*a", "aa"));
    assert!(!matches("a*a", "a"));
}
