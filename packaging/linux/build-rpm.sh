#!/usr/bin/env bash
# Builds Suit's RPM for Fedora, in a Fedora container, and leaves it in out/dist/.
# Needs Docker (or Podman with `DOCKER=podman`); nothing is installed on this computer.
#
#   packaging/linux/build-rpm.sh          for Fedora 44
#   FEDORA=43 packaging/linux/build-rpm.sh
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
docker="${DOCKER:-docker}"
fedora="${FEDORA:-44}"
image="suit-rpm-builder:f$fedora"
version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$root/Cargo.toml" | head -1)"

"$docker" build -q -t "$image" --build-arg "FEDORA=$fedora" "$root/packaging/linux" >/dev/null
mkdir -p "$root/out/dist" "$root/target/fedora"

# As the person who owns the source, so that what is built belongs to them as well.
"$docker" run --rm -u "$(id -u):$(id -g)" \
    -e HOME=/tmp/home -e CARGO_HOME=/src/target/fedora/cargo-home \
    -v "$root":/src -w /src "$image" bash -euc '
        mkdir -p "$HOME"
        cargo build --release --locked -p suit -p suit-cli --target-dir target/fedora
        rpmbuild -bb packaging/linux/suit.spec \
            --define "_topdir /tmp/home/rpmbuild" \
            --define "srcroot /src" \
            --define "suit_version '"$version"'"
        cp /tmp/home/rpmbuild/RPMS/*/suit-*.rpm out/dist/
        rpmlint out/dist/suit-*.rpm || true
    '
ls -l "$root"/out/dist/suit-*.rpm
