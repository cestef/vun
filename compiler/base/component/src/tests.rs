use vun_intern::Symbol;
use vun_trace::{Activity, Timings};

use crate::abi::{ABI, Descriptor, Owned, Slice, Status};
use crate::{Asked, Cause, Component, Fault, Interface, Kind, RELEASE, Wire};

/// A service that answers with its call, asking the host for a number first.
trait Greeter: Send + Sync {
    fn greet(&self, name: Symbol, ask: &dyn Fn(u32) -> u32) -> String;
}

struct Polite;

impl Greeter for Polite {
    fn greet(&self, name: Symbol, ask: &dyn Fn(u32) -> u32) -> String {
        let _timed = Activity::stage("greet").start();
        assert!(name.as_str() != "panic", "told to");
        format!("{} x{}", name.as_str(), ask(u32::try_from(name.as_str().len()).unwrap()))
    }
}

struct Greeting;

impl Interface for Greeting {
    const KIND: Kind = Kind("greeting");
    type Service = dyn Greeter;
    type Call<'a> = Symbol;
    type Reply = String;
    type Ask = u32;
    type Answer = u32;

    fn serve(service: &dyn Greeter, call: Symbol, ask: Asked<'_, Self>) -> String {
        service.greet(call, ask)
    }
}

/// An interface of another kind.
struct Other;

impl Interface for Other {
    const KIND: Kind = Kind("other");
    type Service = ();
    type Call<'a> = ();
    type Reply = ();
    type Ask = ();
    type Answer = ();

    fn serve((): &(), (): (), _: Asked<'_, Self>) {}
}

fn component() -> Component {
    Component::within(crate::descriptor!(Greeting, Polite)).unwrap()
}

#[test]
fn a_call_is_answered_and_the_component_asks_the_host_back() {
    let asked = |n: u32| n * 10;
    let reply = component().call::<Greeting>(&Symbol::intern("vun"), &asked).unwrap();
    assert_eq!(reply, "vun x30");
}

#[test]
fn a_panic_inside_is_a_fault_and_the_component_is_called_again() {
    let asked = |n: u32| n;
    let component = component();
    let fault = component.call::<Greeting>(&Symbol::intern("panic"), &asked).unwrap_err();
    assert!(matches!(&fault, Fault::Panicked(message) if message.contains("told to")), "{fault}");
    assert_eq!(component.call::<Greeting>(&Symbol::intern("ok"), &asked).unwrap(), "ok x2");
}

#[test]
fn a_component_of_another_kind_is_refused() {
    let fault = component().call::<Other>(&(), &|(): ()| ()).unwrap_err();
    assert!(matches!(fault, Fault::Kind { expected: Kind("other"), .. }));
}

unsafe extern "C" fn never(_: *const crate::abi::Host, _: Slice, _: *mut Owned) -> Status {
    Status::Done
}

#[test]
fn a_descriptor_of_another_layout_or_release_is_refused() {
    let described = |abi, release: &'static str| {
        let descriptor = Descriptor {
            abi,
            kind: Slice::of(b"greeting"),
            release: Slice::of(release.as_bytes()),
            call: never,
            free: Owned::release,
        };
        Component::within(Box::leak(Box::new(descriptor))).err()
    };
    assert!(matches!(described(ABI + 1, RELEASE), Some(Fault::Abi { .. })));
    assert!(matches!(described(ABI, "0.0.0"), Some(Fault::Release { .. })));
    assert!(described(ABI, RELEASE).is_none());
}

#[test]
fn a_cause_keeps_the_messages_of_an_error_and_its_sources() {
    #[derive(Debug)]
    struct Outer(std::io::Error);

    impl std::fmt::Display for Outer {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("cannot read it")
        }
    }

    impl std::error::Error for Outer {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            Some(&self.0)
        }
    }

    let cause = Cause::of(&Outer(std::io::Error::other("the disk")));
    assert_eq!(cause.messages().collect::<Vec<_>>(), ["cannot read it", "the disk"]);
    assert_eq!(Wire::open::<Cause>(&Wire::seal(&cause)), Ok(cause));
}

#[test]
fn what_a_component_times_is_timed_in_the_hosts_table_inside_the_call() {
    Timings::enable();
    component().call::<Greeting>(&Symbol::intern("vun"), &|n: u32| n).unwrap();
    let rows = Timings::report().unwrap().rows;
    let row = |activity| rows.iter().find(|r| r.activity == activity).expect("timed");
    let (call, greet) = (row(Activity::component("greeting")), row(Activity::stage("greet")));
    assert_eq!((call.runs, greet.runs), (1, 1));
    assert!(call.own + greet.total <= call.total, "the call's own time excludes what it ran inside");
}
