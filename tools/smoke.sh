#!/usr/bin/env bash
# Drives the real window, on a display nobody sees: starts Suit on a virtual X display,
# presses the keys a presenter would, and saves a picture after each step to out/smoke/.
#
# Needs Xvfb, openbox, xdotool, ImageMagick's import, and a software Vulkan driver
# (Debian/Ubuntu: xvfb openbox xdotool imagemagick x11-utils mesa-vulkan-drivers).
# Builds nothing: run `cargo build -p suit` first.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/out/smoke"
mkdir -p "$out"
rm -f "$out"/*.png "$out/app.log"

export DISPLAY=:98
Xvfb "$DISPLAY" -screen 0 1600x900x24 -nolisten tcp >/dev/null 2>&1 &
xvfb=$!
app=""
wm=""
trap 'kill $app $wm $xvfb 2>/dev/null || true' EXIT
sleep 1
openbox >/dev/null 2>&1 &
wm=$!
sleep 1

# X11 rather than whatever Wayland session this was started from, drawn in software.
env -u WAYLAND_DISPLAY VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json \
    "$root/target/debug/suit" "$root/examples/sample.suit.json" >"$out/app.log" 2>&1 &
app=$!

window() { xdotool search --onlyvisible --name "$1" 2>/dev/null | head -1 || true; }
shot() { import -window root "$out/$1.png"; echo "  saved out/smoke/$1.png"; }
fail() { echo "FAILED: $1"; tail -5 "$out/app.log"; exit 1; }
# Keys go to whatever has the focus, as they do from a keyboard.
press() { xdotool key "$@"; }

main=""
for _ in $(seq 60); do
    main="$(window '^Suit$')"
    [ -n "$main" ] && break
    sleep 0.5
done
[ -n "$main" ] || fail "the window did not appear"
xdotool windowfocus --sync "$main"
sleep 6
echo "the editor is up"
shot 1-editor

press F5
sleep 4
geometry="$(xdotool getwindowgeometry "$main" | grep Geometry)"
[[ "$geometry" == *"1600x900"* ]] || fail "not full screen: $geometry"
echo "F5 presents, full screen"
shot 2-presenting

press n
sleep 2
notes="$(window 'notes')"
[ -n "$notes" ] || fail "N did not open the notes window"
echo "N opens the notes in a window of their own"
shot 3-notes

# From the notes window, as a presenter with the notes in front of them would.
xdotool windowfocus --sync "$notes"
press Right
sleep 4
shot 4-next-from-notes
press space
press n
sleep 1
[ -z "$(window 'notes')" ] || fail "N did not put the notes away"
echo "keys work from the notes window, and N puts it away"

xdotool windowfocus --sync "$main"
press q
sleep 2
shot 5-editor-again
echo "Q returns to the editor"

# Asked to close the way a window manager asks; nothing is unsaved, so it goes.
xdotool windowfocus --sync "$main"
press alt+F4
for _ in $(seq 20); do
    kill -0 "$app" 2>/dev/null || break
    sleep 0.5
done
if kill -0 "$app" 2>/dev/null; then fail "the window did not close"; fi
if wait "$app"; then echo "closed cleanly"; else fail "the app ended with an error"; fi
app=""
echo "ok"
