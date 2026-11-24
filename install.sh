#!/bin/sh
# Installs the latest release of Vun: this machine's toolchain is downloaded, checked, unpacked in
# a temporary directory, then installs itself with `vun toolchain update`, which checks it again
# against the release's manifest, keeps it in ~/.local/share/vun/toolchains and links
# ~/.local/bin/vun to it.
#
# Nothing downloaded runs before it is checked: the release's SHA256SUMS is verified against the
# public key below (Ed25519, as the release workflow signs it with openssl), and the archive
# against SHA256SUMS, as is the `https` component `vun` fetches the manifest with. Verifying needs
# OpenSSL 3 or LibreSSL 3.7 and sha256sum or shasum.
#
# VUN_RELEASES names another place releases are found, as `vun` reads it; VUN_INSTALL_KEY the
# key that place signs with, as the base64 of its public key's DER (what a PEM file holds).
set -eu

releases="${VUN_RELEASES:-https://github.com/cestef/vun}"
key="${VUN_INSTALL_KEY:-MCowBQYDK2VwAyEAT3WaESk80t457PaxHO413LulRasD7rMVD+SAK1pRRTc=}"

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
archive="vun-$arch-$os.tar.gz"
scheme="vun-scheme-https-$arch-$os.tar.gz"

scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
cd "$scratch"

for file in "$archive" "$scheme" SHA256SUMS SHA256SUMS.sig; do
    curl --proto '=https,file' -fsSLO "$releases/releases/latest/download/$file"
done

printf '%s\n%s\n%s\n' '-----BEGIN PUBLIC KEY-----' "$key" '-----END PUBLIC KEY-----' > key.pem
openssl pkeyutl -verify -pubin -inkey key.pem -rawin -in SHA256SUMS -sigfile SHA256SUMS.sig >/dev/null ||
    { echo "vun: SHA256SUMS is not signed by the release key" >&2; exit 1; }

if command -v sha256sum >/dev/null; then sum="sha256sum"; else sum="shasum -a 256"; fi
for file in "$archive" "$scheme"; do
    grep -F " $file" SHA256SUMS | $sum -c - >/dev/null ||
        { echo "vun: $file does not match SHA256SUMS" >&2; exit 1; }
done

tar -xzf "$archive"
tar -xzf "$scheme"
vun/bin/vun toolchain update

case ":$PATH:" in
    *":$HOME/.local/bin:"*) ;;
    *) echo "vun: add ~/.local/bin to PATH to run it" >&2 ;;
esac
