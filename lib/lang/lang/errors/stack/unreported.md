# stack.unreported

The program bounds a function with `#[stack.max(n)]`, and the backend building it reports no
frame sizes.

Stack bounds are checked from the frame size of each function (spec 07, section 4), which the
backend reports as it generates code (`baseline`) or reads from what its tools report (`c`, through
the C compiler's `-fstack-usage`, when the target's C compiler profile says it writes one). A
backend reporting neither cannot build a program that uses `stack.max`.

## Example

```console
$ vun build --backend c firmware.vun
error[stack.unreported]: this function is bound by `stack.max`, and backend `c` reported no frame sizes
```

## Fix

Build with a backend that reports frame sizes, or give the target's C compiler profile a way to
report them (`.frame_sizes = .stack_usage`):

```console
$ vun build --backend baseline firmware.vun
```
