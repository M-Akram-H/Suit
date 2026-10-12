#!/usr/bin/env bash
# Builds Suit for Windows, on Linux, and makes its setup program:
#
#   out/dist/Suit-<version>-Setup.exe
#
# Needs the Windows side of Rust and of GCC, and NSIS, which makes the setup program:
#
#   rustup target add x86_64-pc-windows-gnu
#   sudo apt install mingw-w64 nsis          (Fedora: mingw64-gcc mingw32-nsis)
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$root/Cargo.toml" | head -1)"
target=x86_64-pc-windows-gnu

if ! command -v cargo >/dev/null 2>&1 && [ -f "$HOME/.cargo/env" ]; then
    . "$HOME/.cargo/env"
fi
for needed in cargo x86_64-w64-mingw32-gcc makensis; do
    command -v "$needed" >/dev/null 2>&1 || { echo "build-windows.sh: $needed was not found" >&2; exit 1; }
done

cargo build --release --locked --target "$target" --target-dir "$root/target/windows" \
    --manifest-path "$root/Cargo.toml" -p suit -p suit-cli

# What goes into the setup program, without what only a debugger would read.
stage="$root/target/windows/stage"
rm -rf "$stage"
mkdir -p "$stage" "$root/out/dist"
cp "$root/target/windows/$target/release/suit.exe" "$root/target/windows/$target/release/suit-cli.exe" "$stage/"
x86_64-w64-mingw32-strip "$stage/suit.exe" "$stage/suit-cli.exe"

setup="$root/out/dist/Suit-$version-Setup.exe"
makensis -V2 -DVERSION="$version" -DBIN="$stage" -DROOT="$root" -DOUT="$setup" \
    "$root/packaging/windows/suit.nsi"
ls -l "$setup"
