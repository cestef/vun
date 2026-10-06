//! The component's side: answering the host's calls from a service, asking the host back.

use std::panic::{self, AssertUnwindSafe};
use std::sync::Once;

use vun_codec::Malformed;
use vun_trace::{Activity, Config, Relay, Relaying, Timings};

use crate::abi::{Beat, Descriptor, Host, Owned, Relay as Forward, Slice, Status};
use crate::{Fault, Interface, Wire};

static TRACE: Once = Once::new();

/// The relay of a library: each beat goes to the host's table.
struct Forwarding(Forward);

impl Relay for Forwarding {
    fn begin(&self, activity: Activity) {
        // SAFETY: the host's function, given a name that lives as long as this library.
        unsafe { (self.0)(Beat::Begin, activity.kind as u32, Slice::of(activity.name.as_bytes())) };
    }

    fn end(&self) {
        // SAFETY: as above; the name is not read at the end.
        unsafe { (self.0)(Beat::End, 0, Slice::of(&[])) };
    }
}

impl Host {
    /// Asks the host. The host failing is a panic here, which [`answer`] reports as the
    /// component's.
    pub(crate) fn ask<I: Interface>(&self, ask: &I::Ask) -> I::Answer {
        let request = Wire::seal(ask);
        let mut out = Owned::EMPTY;
        // SAFETY: the host's function, given its own context, a live request and an `out`.
        let status = unsafe { (self.ask)(self.context, Slice::of(&request), &raw mut out) };
        // SAFETY: filled by the host's `ask`, whose `free` it is.
        let reply = unsafe { out.copied(self.free) };
        match status {
            Status::Done => {
                Wire::open(&reply).unwrap_or_else(|e| panic!("the host's answer is malformed: {e}"))
            }
            _ => panic!("the host failed: {}", String::from_utf8_lossy(&reply)),
        }
    }
}

/// Answers one call of the host: reads the request, serves it, writes the reply to `out`.
///
/// # Safety
/// Called as a [`Descriptor`]'s `call`: `host` is a live host, `input` a live slice and `out`
/// writable, for the call.
pub unsafe fn answer<I: Interface>(
    service: &I::Service,
    host: *const Host,
    input: Slice,
    out: *mut Owned,
) -> Status {
    // A library has its own tracing state, which the host's environment configures alike.
    TRACE.call_once(|| Config { timings: false, timeline: None, ..Config::from_env() }.install());
    // SAFETY: by the caller.
    let (request, host) = unsafe { (input.bytes(), &*host) };
    if let Some(relay) = host.relay {
        Timings::enable();
        Relaying::install(Box::new(Forwarding(relay)));
    }
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
        let call = Wire::open::<I::Call<'static>>(request)?;
        let ask = |ask: I::Ask| host.ask::<I>(&ask);
        Ok::<_, Malformed>(Wire::seal(&I::serve(service, call, &ask)))
    }));
    let (status, bytes) = match outcome {
        Ok(Ok(reply)) => (Status::Done, reply),
        Ok(Err(malformed)) => (Status::Malformed, malformed.to_string().into_bytes()),
        Err(payload) => (Status::Panicked, Fault::text_of(&*payload).into_bytes()),
    };
    // SAFETY: by the caller.
    unsafe { out.write(Owned::new(bytes)) };
    status
}

impl Descriptor {
    /// The descriptor of a component of interface `I` answering through `call`.
    #[must_use]
    pub const fn of<I: Interface>(call: crate::abi::Call) -> Self {
        Self {
            abi: crate::abi::ABI,
            kind: Slice::of(I::KIND.0.as_bytes()),
            release: Slice::of(crate::RELEASE.as_bytes()),
            call,
            free: Owned::release,
        }
    }
}

/// The descriptor of a component of interface `$interface` serving `$instance`, a value of a type
/// the interface's service is.
#[macro_export]
macro_rules! descriptor {
    ($interface:ty, $instance:expr) => {{
        static SERVICE: ::std::sync::LazyLock<::std::boxed::Box<<$interface as $crate::Interface>::Service>> =
            ::std::sync::LazyLock::new(|| ::std::boxed::Box::new($instance));

        #[allow(unsafe_code, reason = "the boundary's entry, called as a descriptor's")]
        unsafe extern "C" fn call(
            host: *const $crate::abi::Host,
            input: $crate::abi::Slice,
            out: *mut $crate::abi::Owned,
        ) -> $crate::abi::Status {
            // SAFETY: called by the host as a descriptor's `call`.
            unsafe { $crate::answer::<$interface>(&**SERVICE, host, input, out) }
        }

        static DESCRIPTOR: $crate::abi::Descriptor = $crate::abi::Descriptor::of::<$interface>(call);
        &DESCRIPTOR
    }};
}

/// Exports the component of a library: its descriptor, under the symbol loaders look for.
#[macro_export]
macro_rules! export {
    ($interface:ty, $instance:expr) => {
        #[allow(unsafe_code, reason = "the symbol a loader finds the component by")]
        #[unsafe(no_mangle)]
        pub extern "C" fn vun_component() -> &'static $crate::abi::Descriptor {
            $crate::descriptor!($interface, $instance)
        }
    };
}
