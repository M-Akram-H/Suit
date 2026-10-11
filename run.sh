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
#
# Manim and what it needs (LaTeX, Cairo, Pango) may live in a container while this is
# called from outside it. The script notices, and starts the app in that container.
set -euo pipefail

root="$(cd "$(dirname "$0")" && pwd)"
venv="$root/.venv"
# Which container the .venv was set up in, if it was in one.
made_in="$venv/made-in"

container_here=""
if [ -e /run/.containerenv ]; then
    container_here="$(sed -n 's/^name="\(.*\)"$/\1/p' /run/.containerenv)"
fi

# The Python that has Manim: the project's own, unless another one was asked for.
python="${SUIT_PYTHON:-$venv/bin/python}"
has_manim() {
    [ -x "$python" ] && "$python" -c \
        'import importlib.util, sys; sys.exit(importlib.util.find_spec("manim") is None)' \
        2>/dev/null
}

if ! has_manim; then
    there="$(cat "$made_in" 2>/dev/null || true)"
    # A .venv belongs to the Python it was made with. From outside the container it was
    # made in, its python is another program altogether and finds none of its packages.
    if [ -z "$container_here" ] && [ -n "$there" ] && [ -z "${SUIT_RETRIED:-}" ] \
        && command -v distrobox >/dev/null 2>&1; then
        echo "run.sh: Manim is set up in the container \"$there\"; starting Suit there." >&2
        tty=()
        [ -t 0 ] || tty=(--no-tty)
        exec distrobox enter "${tty[@]}" "$there" -- env SUIT_RETRIED=1 "$root/run.sh" "$@"
    fi
    {
        echo "run.sh: $python cannot import Manim, so the canvas would stay empty."
        if [ -f "$venv/pyvenv.cfg" ]; then
            made_for="$(sed -n 's/^version = //p' "$venv/pyvenv.cfg")"
            here="$("$python" -c 'import platform; print(platform.python_version())' 2>/dev/null || echo "none")"
            echo "        The .venv was made for Python $made_for; the python here is $here."
        fi
        if [ -n "$there" ] && [ -z "$container_here" ]; then
            echo "        It was set up in the container \"$there\". Run this from inside it:"
            echo "          distrobox enter $there -- \"$root/run.sh\""
        else
            echo "        To set Manim up for this Python (it needs Cairo, Pango and LaTeX too):"
            echo "          python3 -m venv .venv && .venv/bin/pip install manim==0.21.0 manim-slides==5.7.0"
        fi
    } >&2
    exit 1
fi
export SUIT_PYTHON="$python"
# Remember where this works, for the day it is called from somewhere it does not.
if [ -n "$container_here" ] && [ -d "$venv" ] && [ ! -e "$made_in" ]; then
    echo "$container_here" >"$made_in"
fi

# rustup may have been installed without putting cargo on the PATH.
if ! command -v cargo >/dev/null 2>&1 && [ -f "$HOME/.cargo/env" ]; then
    . "$HOME/.cargo/env"
fi
if ! command -v cargo >/dev/null 2>&1; then
    echo "run.sh: cargo was not found. Install Rust from https://rustup.rs first." >&2
    exit 1
fi

exec cargo run --quiet --manifest-path "$root/Cargo.toml" -p suit -- "$@"
