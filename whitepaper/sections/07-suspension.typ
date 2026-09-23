#import "../template.typ": *

= Suspension <sec-suspension>

A function whose body contains `yield` is a coroutine. Calling it runs nothing: it constructs a
_frame_ in place and returns it. The frame's type implements the `lang` interface
`Coroutine(Y, R)`, whose two operations are `resume`, returning either `.yielded(y)` or `.done(r)`,
and `end`, which consumes the frame.

Frames have three properties that together remove the usual machinery of asynchronous programming.
They are *sized*: the frame type and its size are known at compile time, so frames may live on a
stack, in a static, in a pool or behind an owning pointer, and a program's total task memory is known
at build time. They are *linear*: a frame must be ended, and ending a suspended frame runs the pending
deferred statements of its current suspension point, which makes cancellation explicit and
structured. And they are *fixed*: because locals may be borrowed across a suspension point, a frame
never moves after construction. The last property is a kind (@sec-memory), so no pinning wrapper type
is needed.

Data flows into a coroutine only through the borrows and capabilities it was created with; `yield`
has no result. Delegation is written `yield ..e`, which is defined as `for y in e { yield y; }` and
whose value is the result of `e`. The `for` loop, in turn, is defined over any coroutine: each
yielded value runs the body, the loop's value is the coroutine's result, and the frame is ended on
every exit.

With these definitions, iterators and asynchronous computations are both aliases of one interface.
An iterator is `Coroutine(T, ())`. An asynchronous task is `Coroutine(Wait, R)`, where a `Wait` is a
request, yielded outward to the executor, to be resumed when some event fires. Leaf operations such
as timers and peripheral transfers yield those requests; ordinary asynchronous code only delegates.
Wakeups are therefore owned by the executor rather than threaded through every resumption, and
executors, timers and channels are library code.

```ov
blink := (led: &mut Led, timer: &Timer) -> Async(never) {
    for { led.toggle(); yield ..timer.after(.ms(500)); }
};

echo := (s: &mut Serial) -> Async(never) {
    mut buf: [64]u8;
    for {
        n := yield ..s.read(&out buf);
        yield ..s.write(buf[0..n]);
    }
};
```

Frame layout is specified, because frame size is a resource budget: parameters are stored once;
delegated frames are constructed in place inside the parent, in slots shared by suspension points
with disjoint live ranges; only locals live across a suspension are stored. The specification closes
a known failure mode in which each layer of forwarding doubles the frame size @rust-async-size.
