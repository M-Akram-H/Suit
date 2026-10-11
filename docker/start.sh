#!/usr/bin/env bash
# Starts Suit inside the container: a screen nobody sees, the way to it from a web browser,
# and then the editor. What follows the name is passed on to the editor:
#
#   suit-start                          the sample deck
#   suit-start --new                    an empty deck
#   suit-start talk.suit.json           a deck from the decks folder
#
# Some other programs can be asked for instead, and are run next to the decks:
#
#   suit-start suit-cli check talk.suit.json
#   suit-start manim-slides render talk.py Deck
#   suit-start bash
set -euo pipefail

decks=/decks

# What is made here should belong to whoever owns the decks folder outside the container,
# not to root. (Where the folder is root's in here, as it is with Docker Desktop and with
# rootless Docker, root in here is already that person.)
if [ "$(id -u)" = 0 ]; then
    mkdir -p "$HOME" "$XDG_CACHE_HOME"
    owner="$(stat -c '%u:%g' "$decks")"
    if [ "${owner%%:*}" != 0 ]; then
        if [ "$(stat -c '%u' "$XDG_CACHE_HOME")" != "${owner%%:*}" ]; then
            chown -R "$owner" "$HOME"
        fi
        exec setpriv --reuid "${owner%%:*}" --regid "${owner##*:}" --clear-groups "$0" "$@"
    fi
fi

case "${1:-}" in
    suit-cli | manim | manim-slides | python | python3 | bash | sh)
        exec "$@"
        ;;
esac

export DISPLAY=:1
screen="${SUIT_SCREEN:-1600x900}"
# Left behind when the container was stopped rather than the editor closed.
rm -f /tmp/.X1-lock /tmp/.X11-unix/X1

Xvfb "$DISPLAY" -screen 0 "${screen}x24" -nolisten tcp >/dev/null 2>&1 &
for _ in $(seq 100); do
    [ -S /tmp/.X11-unix/X1 ] && break
    sleep 0.1
done
[ -S /tmp/.X11-unix/X1 ] || { echo "suit-start: the screen did not start (SUIT_SCREEN=$screen?)" >&2; exit 1; }

openbox --config-file /etc/suit/openbox.xml >/dev/null 2>&1 &
# The screen is reached from the browser through noVNC. Only the container itself may
# talk to the VNC server; what leaves the container is the web page on port 6080.
x11vnc -display "$DISPLAY" -forever -shared -nopw -localhost -rfbport 5900 -quiet >/dev/null 2>&1 &
websockify --web /usr/share/novnc 6080 localhost:5900 >/dev/null 2>&1 &

# The window is drawn by the processor: there is no graphics card in here.
icd="$(find /usr/share/vulkan/icd.d -name 'lvp_icd*.json' | head -1)"
[ -z "$icd" ] || export VK_ICD_FILENAMES="$icd"

echo "Suit is starting. Open http://localhost:${SUIT_PORT:-6080} in a web browser."
echo "Decks are opened from and saved to the decks folder, which is $decks in here."
exec suit "$@"
