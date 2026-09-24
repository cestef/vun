# types.cannot_infer

A call leaves out `$` arguments that the other arguments do not determine.

A `$` parameter named in the type of a later parameter may be left out of a call, all such
parameters or none, and its value is found by matching the types of the arguments against the
declared ones (spec 02, section 1). When no argument's type fixes it, it must be written.

## Example

```ov
first := ($T: type, xs: ?T) -> ?T { xs };

none := () {
    first(.none); //~ types.cannot_infer
};
```

## Fix

Write the `$` arguments:

```ov
first := ($T: type, xs: ?T) -> ?T { xs };

none := () {
    first(u8, .none);
};
```
