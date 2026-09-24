# cli.unavailable_command

The command exists in the command tree but is not implemented yet.

The command line of spec 12, section 7 is declared whole, so help lists every command and marks
the ones still to come `(planned)`; running one reports this instead of doing nothing.

## Example

```console
$ vun pkg update
error[cli.unavailable_command]: `vun pkg update` is not available yet
```

## Fix

Use a command that is available; `vun pkg --help` marks the planned ones:

```console
$ vun pkg --help
dependencies, releases and API diffs

usage: vun pkg <command> [options]
       vun pkg help <command>

commands:
  add     declare a dependency, fetching an archive to learn its hash
  update  raise dependency versions (planned)
  fetch   download dependencies into the local store and write the lockfile
  ...
```
