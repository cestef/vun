# 06. Functions can suspend

One primitive covers iterators, generators, async I/O and hand-written state machines.

## 1. The protocol

```ov
// lang
Step := ($Y: type, $R: type) -> type { enum { yielded(Y), done(R) } };

Coroutine := ($Y: type, $R: type) -> Interface {
    interface(($Self: type) -> type { struct {
        resume: (self: &mut Self) -> Step(Y, R) = _;
        end: (self: &take Self) = _;    // done: frees; suspended: runs the pending defers
    }})
};
```

- A function whose body contains `yield` is a coroutine function. Its result type is a `Coroutine`
  interface (or an alias). Calling it runs nothing: it constructs the frame in place and returns it.
  The call's type is the frame type, `f.Frame`; the body yields what the interface's `resume`
  carries in `yielded` and ends with what it carries in `done`.
- The frame is a value of an anonymous type, named `f.Frame`. Its size is known at compile time
  (`std.meta.size(blink.Frame)`), so frames can live on the stack, in a static, in a pool or in an
  `Own`, and none of these needs a heap.
- Frames are **linear**: you must `end` them, which runs the defers of the current suspension point.
  Cancellation is explicit and structured.
- Frames are **fixed**: locals may be borrowed across a `yield`, so a frame never moves. No `Pin`
  wrapper exists, because "fixed" is a kind.
- `yield v` suspends with `v`. It has no result: data flows into a coroutine only through the
  capabilities and borrows it was created with.
- A coroutine whose frame would contain itself (direct or indirect recursion through `yield ..`)
  is a compile error unless the recursion goes through an `Own`.

## 2. Consuming coroutines

`for p in e { body }` drives any coroutine (appendix B), through `lang.ops.Iterate`: a frame is
its own iterator (`CoroutineIterate`, an impl over the interface constructor `Coroutine`), and
bounds, slices and arrays have iterators written in `lang.ops` as coroutine functions, over the
primitives `lang.bounds.{empty, first, ends}` and `lang.mem.len`:

- each `.yielded(v)` is matched against `p` and runs `body`;
- the `for` expression's value is the coroutine's `.done` value, unless `break v` supplies another;
- the frame is `end`ed on every exit, including `break` and `?`.

`yield ..e` **delegates**: it yields everything `e` yields, and its value is `e`'s result. It is
exactly `for y in e { yield y; }` (appendix B), so there is no hidden resume input and no separate
`await` keyword.

## 3. Iterators and async are aliases

```ov
// std
Iter := ($T: type) -> Interface { Coroutine(T, ()) };
Async := ($R: type) -> Interface { Coroutine(task.Wait, R) };
```

`task.Wait` is a request yielded to the executor: "resume me when this event fires". Leaf futures
(timers, peripherals, channels) yield it; ordinary async code only delegates with `yield ..`. The
executor, not the task, owns wakeups, so no waker is threaded through every resume.

```ov
evens := (limit: u32) -> Iter(u32) {
    for i in 0..limit { if i % 2 == 0 { yield i; } }
};

blink := (led: &mut Led, timer: &Timer) -> Async(never) {
    for {
        led.toggle();
        yield ..timer.after(.ms(500));
    }
};

echo := (s: &mut Serial) -> Async(never) {
    mut buf: [64]u8;
    for {
        n := yield ..s.read(&out buf);   // a borrow of a frame local, held across suspension
        yield ..s.write(buf[0..n]);
    }
};
```

Slices, ranges, maps and generator functions all implement `Iter`. The optimizer flattens simple
iterators into plain loops.

## 4. Executors

Executors are `std` library code with no compiler support:

- `std.task.run(tasks)`: single-threaded, sleeps until an interrupt or event (embedded, bare metal);
- `std.task.block_on(t)`: hosted, one task;
- `std.task.Pool(n)`: hosted, thread pool;
- `std.testing.Sim`: deterministic, seeded, virtual time (spec 11).

Since frames have static sizes, a program's total task memory is known at build time:
`$assert(std.meta.size(main.Frame) <= 4096)`.

## 5. Frame layout

Specified, because frame size is a resource budget (research notes, section 3: Rust async frames
double per forwarding layer):

1. Parameters are stored once, in the frame.
2. The frame of a delegated coroutine (`yield ..e`) is constructed in place inside the parent's
   frame, in a slot reused by other suspension points whose live ranges do not overlap.
3. Locals are stored only if they are live across a `yield`; storage for locals with disjoint
   suspension-point liveness overlaps.
4. A function without `yield` that returns another coroutine returns that frame directly, with no
   wrapper.

The frame-size algorithm is part of the specification, so sizes are the same on every backend.
