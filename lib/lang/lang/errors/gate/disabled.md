# gate.disabled

A declaration behind a language feature is used by a package that does not enable the feature.

A language feature (`lang.gate.Feature`) is a rule of the language not yet settled. What is behind
one is marked `#[gate.behind(feature)]`, and usable only from the package declaring it and from
the packages whose manifest lists the feature in `.features` (spec 12, section 2).

## Example

```vun
#[comptime.deferred] //~ gate.disabled
check := ($cond: lang.meta.Code(bool)) {};
```

## Fix

Enable the feature in the manifest of the package:

```vun
package := .{
    // ...
    .features = .{ .deferred },
};
```
