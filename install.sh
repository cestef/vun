#!/bin/sh
# Installs the latest release of Vun: this machine's toolchain is downloaded and unpacked in a
# temporary directory, then installs itself with `vun toolchain update`, which checks it against
# the release's manifest, keeps it in ~/.local/share/vun/toolchains and links ~/.local/bin/vun to
# it. VUN_RELEASES names another place releases are found, as `vun` reads it.
set -eu

releases="${VUN_RELEASES:-https://github.com/cestef/vun}"

case "$(uname -m)" in
    x86_64 | amd64) arch=x86_64 ;;
    aarch64 | arm64) arch=arm64 ;;
    *) echo "vun: no release is built for $(uname -m)" >&2; exit 1 ;;
esac
case "$(uname -s)" in
    Linux) os=linux ;;
    Darwin) os=macos ;;
    *) echo "vun: no release is built for $(uname -s)" >&2; exit 1 ;;
esac

scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
curl --proto '=https,file' -fsSL "$releases/releases/latest/download/vun-$arch-$os.tar.gz" | tar -xz -C "$scratch"
"$scratch/vun/bin/vun" toolchain update

case ":$PATH:" in
    *":$HOME/.local/bin:"*) ;;
    *) echo "vun: add ~/.local/bin to PATH to run it" >&2 ;;
esac
