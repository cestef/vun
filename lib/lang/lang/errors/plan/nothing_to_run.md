# plan.nothing_to_run

`vun run` was given a package whose plan builds no program.

With a package's directory, or no path in a package, `vun run` carries out the package's plan and
runs the program it builds (spec 12, section 7). A plan recording no `b.program` has nothing to
run: a library package, or a workspace's root, builds none.

## Example

```console
$ vun run
error[plan.nothing_to_run]: the plan of `build.vun` builds no program to run
```

## Fix

Name the root file of a program instead, or add the program to the plan's `build`:

```console
$ vun run main.vun
```
