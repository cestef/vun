# 06. Functions can suspend

One primitive covers iterators, generators, async I/O and hand-written state machines.

## 1. The protocol

```vun
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
  Cancellation is explicit and structured. Each `yield` is therefore an exit of the function, the
  one ending the frame takes: every linear local held there must be consumed by a statement
  deferred before it, or it leaks (`place.linear_leak`). Frames driven by `for` or `yield ..` at
  that point are ended there too.
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

```vun
// std
Iter := ($T: type) -> Interface { Coroutine(T, ()) };
Async := ($R: type) -> Interface { Coroutine(task.Wait, R) };
```

`task.Wait` is a request yielded to the executor: "resume me when this event fires". Leaf futures
(timers, peripherals, channels) yield it; ordinary async code only delegates with `yield ..`. The
executor, not the task, owns wakeups, so no waker is threaded through every resume.

```vun
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
        yield ..s.write(&buf[0..n]);
    }
};
```

Slices, ranges, maps and generator functions all implement `Iter`. Adapters are coroutine
functions too: `std.iter` gives every frame `enumerate`, `map`, `filter`, `take` and `count`, by an
impl without an interface over `lang.Coroutine`, and slices and arrays have `enumerate` over their
items' borrows. The optimizer flattens simple
iterators into plain loops.

## 4. Executors

Executors are `std` library code with no compiler support. A frame answers its interface's
`resume` and `end` itself (the table `iteration`), so an executor calls `t.resume()` and
`t.end()` like any method. An `Event` carries an atomic mark that `fire` sets, from an interrupt
handler too, and an executor takes; a `Wait` names an event and, through `also`, another wait,
a chain living in the waiting frames, so waiting on either of two tasks allocates nothing. A task
resumed before its event fires finds no mark and yields again.

- `std.task.run(tasks)`: single-threaded, over a tuple of borrowed tasks that never finish; it
  resumes each task whose wait is ready and sleeps through the runtime's `std.task.sleep` (a
  typed link symbol) between rounds (embedded, bare metal);
- `std.task.block_on(t)`: hosted, one task;
- `std.task.select(.{ &mut a, &mut b })`: inside a task, runs two tasks until one finishes and
  gives `.first(r)` or `.second(r)`; both stay for the caller to end;
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

The frame-size algorithm is part of the specification, so sizes are the same on every backend:

- A frame starts with its **state**, the narrowest unsigned integer telling apart: not started,
  suspended at each `yield` (in the order of the blocks ending in one), and done.
- A local is **held** by the frame when it is a run-time parameter, when it is borrowed anywhere in
  the body (a borrow may reach it across a `yield`), or when it is **live across a `yield`**: read,
  after resuming or on the path ending the frame there, before it is written whole. A delegated
  frame is a local like any other, so it is constructed in place inside the parent's frame.
- Two held locals may **share storage** unless one of them is a parameter or borrowed, or both
  are live across one `yield`.
- Held locals are **placed** in order, parameters first, then by local: each at the lowest offset
  past the state, aligned to its alignment, overlapping no placed local it may not share storage
  with. The frame's alignment is the largest of the state's and its locals', and its size the end
  of the last local, rounded up to it.

A coroutine function lowers to two functions from its one body (spec 13, section 3): `resume`
goes, by the state, to the start or past the `yield` the frame is suspended at, runs to the next
`yield` or the return, sets the state and gives the step; resuming a finished frame traps. `end`
goes to the path ending the frame at its `yield`, which runs what is pending there. Calling the
coroutine function writes the state and the parameters into the place receiving the frame.

The compiler stage that knows liveness gives frame types their layout, so a frame's size and those
of types holding frames are known wherever a layout is (`std.meta.size`). A coroutine function
written by a `$for` step is one per step: its frames are laid out with that step's bindings.
