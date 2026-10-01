# plan.unknown_setting

A package's plan sets an option of a command that does not exist.

`b.commands(.{ .fmt = .{ .width = 100 } })` gives the options of commands the values to use where
neither the command line nor the environment gives one (spec 12, section 2). Its sections are the
commands, its fields their options, named as `vun <command> --help` names them; a name that is
neither is refused when a command of the package runs, with the closest name it knows.

## Example

```console
$ cat build.vun
build := (b: &mut std.build.Build) {
    b.commands(.{ .fmt = .{ .widht = 100 } });
};
$ vun fmt
error[plan.unknown_setting]: `vun fmt` has no option `widht`
```

## Fix

Name an option the command has:

```console
$ cat build.vun
build := (b: &mut std.build.Build) {
    b.commands(.{ .fmt = .{ .width = 100 } });
};
```
