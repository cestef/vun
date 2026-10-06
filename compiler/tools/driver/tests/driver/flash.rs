//! Giving a program to its device (spec 12, section 2): the ways its plan lists, tried in order
//! until one reaches a device.

use vun_diag::{ChildKind, errors};
use vun_intern::Symbol;
use vun_sema_plan::{Argument, Delivery, Invocation};

use crate::common::Fixture;

fn text(text: &str) -> Argument {
    Argument::Text(Symbol::intern(text))
}

fn run(command: &str, args: Vec<Argument>) -> Delivery {
    Delivery::Run(Invocation { command: Symbol::intern(command), args })
}

/// A way that cannot reach a device on any machine: a program nothing installs.
fn missing() -> Delivery {
    run("vun-no-such-programmer", vec![Argument::File])
}

/// A way that reaches a device when the file it is given is there.
fn shell_test() -> Delivery {
    run("sh", vec![text("-c"), text("test -f \"$0\""), Argument::File])
}

#[test]
fn a_way_is_given_the_file() {
    let fixture = Fixture::new("flash-file");
    let file = fixture.root.write("app.uf2", "blocks");
    let mut session = fixture.session();
    let ways = [shell_test()];
    assert_eq!(session.deliver(&file, &ways).ok(), Some(&ways[0]));
    let absent = fixture.root.join("absent.uf2");
    assert!(session.deliver(&absent, &ways).is_err(), "the program it runs failed");
}

#[test]
fn the_ways_are_tried_in_order_until_one_reaches_a_device() {
    let fixture = Fixture::new("flash-order");
    let file = fixture.root.write("app.uf2", "blocks");
    let mut session = fixture.session();
    let ways = [missing(), Delivery::Drive { mark: Symbol::intern("NO_SUCH_MARK.TXT") }, shell_test()];
    let Ok(reached) = session.deliver(&file, &ways) else { panic!("the last way reaches a device") };
    assert_eq!(reached, &ways[2]);
    assert!(!session.failed());
}

#[test]
fn when_no_way_reaches_a_device_each_says_why() {
    let fixture = Fixture::new("flash-none");
    let file = fixture.root.write("app.uf2", "blocks");
    let mut session = fixture.session();
    let ways = [missing(), Delivery::Drive { mark: Symbol::intern("NO_SUCH_MARK.TXT") }];
    assert!(session.deliver(&file, &ways).is_err());
    let reports = session.diag.sorted(session.origins);
    let ids: Vec<_> = reports.iter().map(|d| d.id).collect();
    assert_eq!(ids, [errors::PLAN_UNDELIVERED]);
    let notes = reports[0].children.iter().filter(|c| c.kind == ChildKind::Note);
    assert_eq!(notes.count(), ways.len(), "a note for each way");
}
