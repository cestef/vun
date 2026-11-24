#import "../template.typ": *
#import "../figures.typ": frame-layout

= Suspension <sec-suspension>

One primitive covers iterators, generators, asynchronous input and output, and hand-written state
machines.

== The protocol

A function whose body contains `yield` is a coroutine function. Calling it runs nothing: it
constructs a _frame_ in place and returns it. The frame's type implements the `lang` interface
`Coroutine(Y, R)`:

```vun
Step := ($Y: type, $R: type) -> type { enum { yielded(Y), done(R) } };

Coroutine := ($Y: type, $R: type) -> Interface {
    interface(($Self: type) -> type { struct {
        resume: (self: &mut Self) -> Step(Y, R) = _;
        end: (self: &take Self) = _;    // finished: frees; suspended: runs pending defers
    }})
};
```

Frames have three properties, and together they replace the usual support machinery of
asynchronous programming.

- *Sized.* The frame type and its size are known at compile time, so frames may live on a stack,
  in a static, in a pool or behind an owning pointer, and a program's total task memory is known at
  build time: `$assert(std.meta.size(std.meta.Frame(main)) <= 4096)` is an ordinary compile-time check.
- *Linear.* A frame must be ended, and ending a suspended frame runs the deferred statements
  pending at its current suspension point, which makes cancellation explicit and structured.
- *Fixed.* Locals may be borrowed across a suspension point, so a frame never moves after
  construction. This is a kind (@sec-memory), so no pinning wrapper type is needed.

Data flows into a coroutine only through the borrows and capabilities it was created with; `yield`
has no result. Delegation is written `yield ..e`, which is defined as `for y in e { yield y; }` and
whose value is the result of `e`. A coroutine whose frame would contain itself, through direct or
indirect delegation, is an error unless the recursion goes through an owning pointer.

== Consuming coroutines

The `for` loop is defined over any coroutine, through the interface `lang.ops.Iterate`. A frame is
its own iterator, by a single implementation over the interface constructor `Coroutine`, and the
iterators of ranges, slices and arrays are themselves coroutine functions written in `lang`. Each
yielded value is matched against the loop pattern and runs the body; the loop's value is the
coroutine's result unless `break v` supplies another; and the frame is ended on every exit,
including `break` and `?`. The desugaring is given in @app-desugar.

== Iterators and asynchronous tasks

With these definitions, iterators and asynchronous computations are aliases of one interface:

```vun
Iter := ($T: type) -> Interface { Coroutine(T, ()) };
Async := ($R: type) -> Interface { Coroutine(task.Wait, R) };
```

A `Wait` is a request, yielded outward to the executor, to be resumed when some event fires. Leaf
operations such as timers and peripheral transfers yield such requests; ordinary asynchronous code
only delegates. The executor therefore owns wake-ups, and no resumption has to pass them along.

```vun
blink := (led: &mut Led, timer: &Timer) -> Async(never) {
    for { led.toggle(); yield ..timer.after(.millis(500)); }
};

echo := (s: &mut Serial) -> Async(never) {
    mut buf: [64]u8;
    for {
        n := yield ..s.read(&out buf);   // a borrow of a frame local, held across suspension
        yield ..s.write(&buf[0..n]);
    }
};
```

Iterator adapters are coroutine functions as well: `std.iter` gives every frame `enumerate`,
`map`, `filter`, `take` and `count` through one implementation without an interface over
`lang.Coroutine`, each adapter consuming the frame it adapts.

== Executors

Executors are library code with no compiler support. A frame answers its interface's `resume` and
`end` like any method, so an executor is an ordinary loop over frames. An `Event` carries an atomic
mark that `fire` sets, possibly from an interrupt handler; executors only look at it, and the task
waiting takes it through `Event.wait`, which finds no mark when resumed early and yields again, so
an executor may resume a task whenever it likes. A `Wait` is `.on(event)` or `.either(a, b)`, a
join of two waits living in the waiting frames, so waiting on either of two tasks allocates
nothing. `std.task` provides a single-threaded executor over a tuple of
borrowed tasks that sleeps between rounds through a symbol the runtime provides (@sec-capabilities),
a `block_on` for hosted programs, and `select`, which runs two tasks until one finishes and leaves
both for the caller to end. A deterministic simulator (@sec-tooling) follows the same protocol. A thread-pool
executor is planned.

== Frame layout

Frame layout is specified, because frame size is a resource budget:

+ parameters are stored once, in the frame;
+ the frame of a delegated coroutine (`yield ..e`) is constructed in place inside the parent's
  frame, in a slot reused by other suspension points whose live ranges do not overlap;
+ locals are stored only if they are live across a suspension point, and storage for locals with
  disjoint live ranges overlaps;
+ a function without `yield` that returns another coroutine returns that frame directly, with no
  wrapper.

Because the algorithm is part of the specification, frame sizes are the same on every backend.
The rules exclude two failure modes observed in Rust, in which parameters are stored twice in a
frame and each layer of forwarding doubles the size of the frame, reaching 16 MiB at a depth of ten
@rust-async-size. @fig-frame illustrates the second rule.

#figure(
  frame-layout,
  caption: [Layout of a frame with two suspension points. The frames of the two delegated
    coroutines, live at different suspension points, share one slot; only `buf`, live across both,
    has its own storage.],
) <fig-frame>
