# stack.unbounded

A function bound by `#[stack.max(n)]` reaches a call whose stack use has no bound.

Under a bound, every call a function makes, directly or through others, must have a known frame
(spec 07, section 4). A call has none when it recurses (its function is called again while it
runs), when it calls a function through an address, or when it calls a function of another
program whose declaration states no bound. A declaration provided from outside states its stack use
with `#[stack.max(n)]`, trusted as its signature is.

## Example

```console
$ cat tick.vun
#[link.symbol("tick"), abi.c]
tick: (x: u8) -> u8 = _;

#[link.export("step"), abi.c, stack.max(512)]
step := (x: u8) -> u8 { tick(x) };
$ vun build --backend baseline tick.vun
error[stack.unbounded]: the stack this function may take has no bound
```

## Fix

State the stack the other program's function takes, as its documentation or its own build gives
it; the bound is trusted, as the signature is:

```console
$ cat tick.vun
#[link.symbol("tick"), abi.c, stack.max(64)]
tick: (x: u8) -> u8 = _;

#[link.export("step"), abi.c, stack.max(512)]
step := (x: u8) -> u8 { tick(x) };
$ vun build --backend baseline tick.vun
```

Every trap site calls the trap handler, `lang.panic.trap`; a program without a runtime exporting
it imports it, so it declares the handler itself the same way:

```console
$ cat handler.vun
#[link.symbol(lang.panic.symbols.trap), stack.max(128)]
trap: (t: &lang.panic.Trap) -> lang.never = _;
```

Declarations of one link symbol are one import, whose bound is the largest they state. Recursion
has no such fix: write it as a loop, whose frame is fixed.
