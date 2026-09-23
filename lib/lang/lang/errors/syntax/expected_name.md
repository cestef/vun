# syntax.expected_name

A name is required where the parser finds none.

A `.` is followed by a member name, a tuple index or `$` and a compile-time operand (appendix D).
The same diagnostic covers other places that take only a name.

## Example

```ov
Point := struct { x: i32, y: i32 };
origin := Point{ .x = 0, .y = 0 };
x := origin.; //~ syntax.expected_name
```

## Fix

Name the member:

```ov
Point := struct { x: i32, y: i32 };
origin := Point{ .x = 0, .y = 0 };
x := origin.x;
```
