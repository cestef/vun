# Nested async fn forwarding doubles future size per layer (16 MiB at depth 10); 2 MiB-stack overflow at depth 7

<https://github.com/rust-lang/rust/issues/163195>

jferments 2026-09-23T05:36:35Z

Still-reproducible case of #62958 on current nightly: nested `async fn` forwarding wrappers double the future size per layer, and construction codegen reserves stack proportional to it (hard overflow at depth 7 on a 2 MiB thread stack). Related layout work: #135527.

I tried this code:

```rust
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

struct YieldOnce { yielded: bool }

impl Future for YieldOnce {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.yielded { Poll::Ready(()) }
        else { self.yielded = true; cx.waker().wake_by_ref(); Poll::Pending }
    }
}

// leaf retains an 8 KiB payload across one suspension point
async fn leaf(payload: [u8; 8192]) -> u64 {
    YieldOnce { yielded: false }.await;
    payload.iter().map(|&x| x as u64).sum()
}

// pure forwarding, but adds a coroutine per layer
async fn bad_delegate<F: Future>(f: F) -> F::Output { f.await }
// control: forwarding without a new coroutine
fn good_delegate<F: Future>(f: F) -> F { f }

fn bad_0(p: [u8; 8192]) -> impl Future<Output = u64> { leaf(p) }
fn good_0(p: [u8; 8192]) -> impl Future<Output = u64> { leaf(p) }

macro_rules! chain {
    ($wrap:ident, $($n:ident = $prev:ident),* $(,)?) => {
        $( fn $n(p: [u8; 8192]) -> impl Future<Output = u64> { $wrap($prev(p)) } )*
    };
}

chain!(bad_delegate,  bad_1 = bad_0,  bad_2 = bad_1,  bad_3 = bad_2,  bad_4 = bad_3,
                       bad_5 = bad_4,  bad_6 = bad_5,  bad_7 = bad_6,  bad_8 = bad_7,
                       bad_9 = bad_8,  bad_10 = bad_9);
chain!(good_delegate, good_1 = good_0, good_2 = good_1, good_3 = good_2, good_4 = good_3,
                       good_5 = good_4, good_6 = good_5, good_7 = good_6, good_8 = good_7,
                       good_9 = good_8, good_10 = good_9);

fn main() {
    let p = [0u8; 8192];
    println!("leaf    {}", std::mem::size_of_val(&leaf(p)));
    println!("bad-1   {}", std::mem::size_of_val(&bad_1(p)));
    println!("bad-2   {}", std::mem::size_of_val(&bad_2(p)));
    println!("bad-3   {}", std::mem::size_of_val(&bad_3(p)));
    println!("bad-4   {}", std::mem::size_of_val(&bad_4(p)));
    println!("bad-5   {}", std::mem::size_of_val(&bad_5(p)));
    println!("bad-6   {}", std::mem::size_of_val(&bad_6(p)));
    println!("bad-7   {}", std::mem::size_of_val(&bad_7(p)));
    println!("bad-8   {}", std::mem::size_of_val(&bad_8(p)));
    println!("bad-9   {}", std::mem::size_of_val(&bad_9(p)));
    println!("bad-10  {}", std::mem::size_of_val(&bad_10(p)));
    println!("good-10 {}", std::mem::size_of_val(&good_10(p)));
}
```

The stack-overflow half of the repro (constructing and polling each depth on a 2 MiB thread, one subprocess per depth) uses the same futures; full harness in the details block at the bottom. -O assembly: https://godbolt.org/z/GhEaMaT3K

I expected to see this happen: each `bad_delegate` layer only awaits and returns its child, so it should add O(1) bytes on top of the child future — sizes grow linearly with depth. The `good_delegate` control, which forwards without introducing a coroutine, confirms the semantics need no extra storage: it stays at the leaf size at every depth. And constructing/polling futures of these sizes on a 2 MiB thread stack should not abort.

Instead, this happened: every layer exactly doubles the future size (S(n+1) = 2*S(n) + 1), so ten no-op forwarding layers turn a 16 KiB leaf into a 16 MiB future:

```text
leaf    16386      (= 2*8192+2, i.e. the original #62958 argument doubling)
bad-1   32773
bad-2   65547
bad-3   131095
bad-4   262191
bad-5   524383
bad-6   1048767
bad-7   2097535
bad-8   4195071
bad-9   8390143
bad-10  16780287
good-10 16386      (flat, as expected)
```

Identical numbers on stable and nightly. The practical consequence is stack use: each layer's construction keeps the child future in a stack temporary. In the -O assembly (godbolt link), the depth-5 constructor's prologue runs a stack-probe loop reserving ~270 KB before calling the depth-4 constructor, which repeats — so the construction call chain alone uses ~17 MB of stack at depth 10, before polling. Constructing and polling on a 2 MiB thread stack: depths 0–6 OK, depth 7 aborts with `has overflowed its stack` (bad-7 is 2,097,535 bytes vs the 2,097,152-byte stack); the `good` control passes depths 0–10.

Side observation: the reservation is per-function, not per executed path — a function that merely contains `bad_10(...)` in a match arm that is never taken still gets the ~16 MiB frame, and overflowed the 2 MiB stack at depth 0 in an earlier version of the harness.

I also tried to check this against the layout work in #135527: on this nightly, `-Z pack-coroutine-layout=captures-only` is not a recognized codegen option, so that mode could not be tested here.

### Meta

`rustc --version --verbose` (stable):
```
rustc 1.97.1 (8bab26f4f 2026-07-14)
binary: rustc
commit-hash: 8bab26f4f68e0e26f0bb7960be334d5b520ea452
commit-date: 2026-07-14
host: x86_64-unknown-linux-gnu
release: 1.97.1
LLVM version: 22.1.6
```

`rustc --version --verbose` (nightly, identical results):
```
rustc 1.100.0-nightly (6bb1652a0 2026-09-22)
binary: rustc
commit-hash: 6bb1652a020e80cef79332741d89e996d71933c9
commit-date: 2026-09-22
host: x86_64-unknown-linux-gnu
release: 1.100.0-nightly
LLVM version: 23.1.1
```

No Rust backtrace is available for the crash: it is a guard-page stack overflow, which aborts the process (`fatal runtime error: stack overflow, aborting`) before any unwinding, and `RUST_BACKTRACE=1` changes nothing. Harness output in its place:

<details><summary>stack-test harness output (identical on stable and nightly)</summary>
<p>

```text
--- Testing Stack Overflow (2 MiB Stack) ---

Testing `bad` delegates:
Depth 0: OK
Depth 1: OK
Depth 2: OK
Depth 3: OK
Depth 4: OK
Depth 5: OK
Depth 6: OK

thread '<unknown>' (454572) has overflowed its stack
fatal runtime error: stack overflow, aborting
Depth 7: CRASHED (Stack Overflow)

Testing `good` delegates:
Depth 0: OK
Depth 1: OK
Depth 2: OK
Depth 3: OK
Depth 4: OK
Depth 5: OK
Depth 6: OK
Depth 7: OK
Depth 8: OK
Depth 9: OK
Depth 10: OK
```

</p>
</details>

<details><summary>full harness (main.rs, no dependencies)</summary>
<p>

```rust
#![allow(dead_code)]

use std::env;
use std::future::Future;
use std::mem::size_of_val;
use std::pin::Pin;
use std::process::{exit, Command};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use std::thread;

struct YieldOnce { yielded: bool }

impl Future for YieldOnce {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.yielded { Poll::Ready(()) } else { self.yielded = true; cx.waker().wake_by_ref(); Poll::Pending }
    }
}

async fn leaf(payload: [u8; 8192]) -> u64 {
    YieldOnce { yielded: false }.await;
    payload.iter().map(|&x| x as u64).sum()
}

async fn bad_delegate<F: Future>(future: F) -> F::Output { future.await }
fn good_delegate<F: Future>(future: F) -> F { future }

fn bad_0(p: [u8; 8192]) -> impl Future<Output = u64> { leaf(p) }
fn good_0(p: [u8; 8192]) -> impl Future<Output = u64> { leaf(p) }

macro_rules! gen_bad  { ($n:ident, $p:ident) => { fn $n(p: [u8; 8192]) -> impl Future<Output = u64> { bad_delegate($p(p)) } }; }
macro_rules! gen_good { ($n:ident, $p:ident) => { fn $n(p: [u8; 8192]) -> impl Future<Output = u64> { good_delegate($p(p)) } }; }

gen_bad!(bad_1, bad_0);  gen_bad!(bad_2, bad_1);  gen_bad!(bad_3, bad_2);  gen_bad!(bad_4, bad_3);
gen_bad!(bad_5, bad_4);  gen_bad!(bad_6, bad_5);  gen_bad!(bad_7, bad_6);  gen_bad!(bad_8, bad_7);
gen_bad!(bad_9, bad_8);  gen_bad!(bad_10, bad_9);
gen_good!(good_1, good_0); gen_good!(good_2, good_1); gen_good!(good_3, good_2); gen_good!(good_4, good_3);
gen_good!(good_5, good_4); gen_good!(good_6, good_5); gen_good!(good_7, good_6); gen_good!(good_8, good_7);
gen_good!(good_9, good_8); gen_good!(good_10, good_9);

fn block_on<F: Future>(fut: F) -> F::Output {
    fn clone(_: *const ()) -> RawWaker { RawWaker::new(std::ptr::null(), &VT) }
    fn noop(_: *const ()) {}
    static VT: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);

    let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VT)) };
    let mut cx = Context::from_waker(&waker);
    let mut fut = fut;
    std::hint::black_box(std::ptr::from_mut(&mut fut));
    let mut pinned = unsafe { Pin::new_unchecked(&mut fut) };
    loop {
        if let Poll::Ready(out) = pinned.as_mut().poll(&mut cx) { return out; }
    }
}

macro_rules! gen_runners {
    ($($n:literal => $f:ident => $run:ident),* $(,)?) => {
        $(
            #[inline(never)]
            fn $run(payload_val: u8) {
                let p = [payload_val; 8192];
                let out = block_on($f(p));
                std::hint::black_box(out);
            }
        )*
    };
}

gen_runners!(
    0 => bad_0 => run_bad_0, 1 => bad_1 => run_bad_1, 2 => bad_2 => run_bad_2, 3 => bad_3 => run_bad_3,
    4 => bad_4 => run_bad_4, 5 => bad_5 => run_bad_5, 6 => bad_6 => run_bad_6, 7 => bad_7 => run_bad_7,
    8 => bad_8 => run_bad_8, 9 => bad_9 => run_bad_9, 10 => bad_10 => run_bad_10
);

gen_runners!(
    0 => good_0 => run_good_0, 1 => good_1 => run_good_1, 2 => good_2 => run_good_2, 3 => good_3 => run_good_3,
    4 => good_4 => run_good_4, 5 => good_5 => run_good_5, 6 => good_6 => run_good_6, 7 => good_7 => run_good_7,
    8 => good_8 => run_good_8, 9 => good_9 => run_good_9, 10 => good_10 => run_good_10
);

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 && args[1] == "stack-test" {
        let depth: usize = args[2].parse().unwrap();
        let is_bad = args[3] == "bad";

        let handle = thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(move || {
                let runner: fn(u8) = if is_bad {
                    match depth {
                        0 => run_bad_0, 1 => run_bad_1, 2 => run_bad_2, 3 => run_bad_3, 4 => run_bad_4,
                        5 => run_bad_5, 6 => run_bad_6, 7 => run_bad_7, 8 => run_bad_8, 9 => run_bad_9, 10 => run_bad_10,
                        _ => return,
                    }
                } else {
                    match depth {
                        0 => run_good_0, 1 => run_good_1, 2 => run_good_2, 3 => run_good_3, 4 => run_good_4,
                        5 => run_good_5, 6 => run_good_6, 7 => run_good_7, 8 => run_good_8, 9 => run_good_9, 10 => run_good_10,
                        _ => return,
                    }
                };
                runner(depth as u8);
            })
            .unwrap();

        if handle.join().is_err() { exit(1); } else { exit(0); }
    }

    println!("--- Measuring Future Sizes ---");
    let payload = [0u8; 8192];
    println!("leaf size (approx): {} bytes", size_of_val(&leaf(payload)));
    println!("\nPathological (`async fn`) sizes:");
    for (n, s) in [
        ("bad-0 ", size_of_val(&bad_0(payload))),   ("bad-1 ", size_of_val(&bad_1(payload))),
        ("bad-2 ", size_of_val(&bad_2(payload))),   ("bad-3 ", size_of_val(&bad_3(payload))),
        ("bad-4 ", size_of_val(&bad_4(payload))),   ("bad-5 ", size_of_val(&bad_5(payload))),
        ("bad-6 ", size_of_val(&bad_6(payload))),   ("bad-7 ", size_of_val(&bad_7(payload))),
        ("bad-8 ", size_of_val(&bad_8(payload))),   ("bad-9 ", size_of_val(&bad_9(payload))),
        ("bad-10", size_of_val(&bad_10(payload))),
    ] { println!("{n} size: {s} bytes"); }
    println!("\nControl (`fn -> F`) sizes:");
    println!("good-0  size: {} bytes", size_of_val(&good_0(payload)));
    println!("good-10 size: {} bytes", size_of_val(&good_10(payload)));

    println!("\n--- Testing Stack Overflow (2 MiB Stack) ---");
    let exe = env::current_exe().unwrap();
    for kind in ["bad", "good"] {
        println!("\nTesting `{kind}` delegates:");
        for depth in 0..=10 {
            let status = Command::new(&exe)
                .args(["stack-test", &depth.to_string(), kind])
                .status()
                .unwrap();
            if status.success() {
                println!("Depth {depth}: OK");
            } else {
                println!("Depth {depth}: CRASHED (Stack Overflow)");
                break;
            }
        }
    }
}
```

</p>
</details>


---

ds84182 2026-09-23T13:15:47Z

This isn't any different than #62958, which is still open, so of course this would occur here. `good_delegate` is a no-op, and the chain of bad delegates is just a symptom of #62958, as each produced future is the payload for another future.

