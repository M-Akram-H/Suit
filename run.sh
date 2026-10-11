#!/usr/bin/env bash
# Builds Suit if anything changed, and starts it. What follows the script's name is passed
# on to the app:
#
#   ./run.sh                              the sample deck
#   ./run.sh --new                        an empty deck
#   ./run.sh talk.suit.json               a deck of your own
#   ./run.sh talk.suit.json --present     straight into presenting it
#   ./run.sh --help                       everything the app can be started with
#
# It can be called from anywhere; a deck is looked for from where you are.
set -euo pipefail

root="$(cd "$(dirname "$0")" && pwd)"

# rustup may have been installed without putting cargo on the PATH.
if ! command -v cargo >/dev/null 2>&1 && [ -f "$HOME/.cargo/env" ]; then
    . "$HOME/.cargo/env"
fi
if ! command -v cargo >/dev/null 2>&1; then
    echo "run.sh: cargo was not found. Install Rust from https://rustup.rs first." >&2
    exit 1
fi

# The Python that has Manim: the project's own, unless another one was asked for.
if [ -z "${SUIT_PYTHON:-}" ]; then
    if [ -x "$root/.venv/bin/python" ]; then
        export SUIT_PYTHON="$root/.venv/bin/python"
    else
        echo "run.sh: there is no .venv in $root, so the system's python3 will be used." >&2
        echo "        Without Manim in it the canvas stays empty. To set one up:" >&2
        echo "          python3 -m venv .venv && .venv/bin/pip install manim==0.21.0 manim-slides==5.7.0" >&2
    fi
fi

exec cargo run --quiet --manifest-path "$root/Cargo.toml" -p suit -- "$@"
