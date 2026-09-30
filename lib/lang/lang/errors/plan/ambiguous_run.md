# plan.ambiguous_run

`vun run` was given a package, or a workspace, that builds more than one program.

With a package's directory, or no path in a package, `vun run` carries out the plan and runs the
program it builds (spec 12, section 7). When the plan records several programs, or several members
of a workspace build one each, which to run is not said; each is listed.

## Example

```console
$ vun run
error[plan.ambiguous_run]: the plan builds 2 programs; which to run is not said
  = note: it builds `server`
  = note: it builds `client`
```

## Fix

Name the root file of the program to run:

```console
$ vun run server.vun
```
