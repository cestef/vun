# lint.unknown

A configured lint names no lint and no group.

A lint is a compile-time value, so configuring one names that value: a typo is reported rather
than ignored (spec 10, section 4). The build names the lint or its group directly, which the
compiler resolves; `--lint` names it by its binding path, and a name matching no lint of the
package checked, its dependencies or `std`, and no group of them, is reported here.

## Example

```console
$ vun check . --lint std.lints.naming.kase=deny
error[lint.unknown]: no lint or group is named `std.lints.naming.kase`
  │
  ╰ help: did you mean `std.lints.naming.case`?
```

## Fix

```console
$ vun check . --lint std.lints.naming.case=deny
```
