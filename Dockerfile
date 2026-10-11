# syntax=docker/dockerfile:1
#
# Suit with everything it needs: the editor, Manim, LaTeX, and a screen that is looked at
# through a web browser, so that it runs the same on Linux, macOS and Windows.
#
#   docker compose up          and then open http://localhost:6080
#
# docs/02-installing.md says more.

ARG RUST_VERSION=1.99
ARG UBUNTU_VERSION=24.04

# ── 1. The editor, built from source ─────────────────────────────────────────
FROM rust:${RUST_VERSION}-bookworm AS editor
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY apps apps
# The sample deck is compiled into the editor.
COPY examples examples
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p suit -p suit-cli \
    && install -D -t /out target/release/suit target/release/suit-cli

# ── 2. Manim, in a Python environment of its own ─────────────────────────────
FROM ubuntu:${UBUNTU_VERSION} AS manim
RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        python3 python3-venv python3-dev build-essential pkg-config \
        libcairo2-dev libpango1.0-dev \
    && rm -rf /var/lib/apt/lists/*
COPY requirements.txt docker/constraints.txt /tmp/
RUN python3 -m venv /opt/manim \
    && /opt/manim/bin/pip install --no-cache-dir -r /tmp/requirements.txt -c /tmp/constraints.txt

# ── 3. What runs ─────────────────────────────────────────────────────────────
FROM ubuntu:${UBUNTU_VERSION}
RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        # Manim: Python, Cairo and Pango to draw with, LaTeX for the formulas, some fonts
        python3 libcairo2 libpango-1.0-0 libpangocairo-1.0-0 \
        texlive-latex-base texlive-latex-recommended texlive-latex-extra \
        texlive-fonts-recommended texlive-science cm-super dvisvgm \
        fonts-dejavu-core fonts-liberation2 fonts-noto-core \
        # a screen, a window manager for it, and the way to it from a web browser
        xvfb openbox x11vnc novnc websockify \
        # what the editor's window is drawn with, by the processor
        libvulkan1 mesa-vulkan-drivers libgl1-mesa-dri libegl1 \
        libxkbcommon0 libxkbcommon-x11-0 libx11-6 libx11-xcb1 libxcb1 \
        libxcursor1 libxi6 libxrandr2 \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=editor /out/ /usr/local/bin/
COPY --from=manim /opt/manim /opt/manim
COPY docker/start.sh /usr/local/bin/suit-start
COPY docker/openbox.xml /etc/suit/openbox.xml
COPY docker/index.html /usr/share/novnc/index.html
COPY crates/ui/assets/icon.png /usr/share/suit/icon.png

# The browser's tab shows Suit's picture rather than that of the page that carries it:
# the page's icons are redrawn from Suit's, each at its own size, and its drawn one,
# which browsers would prefer, is no longer offered.
RUN /opt/manim/bin/python -c "from PIL import Image; import glob; logo = Image.open('/usr/share/suit/icon.png'); [logo.resize(Image.open(name).size, Image.LANCZOS).save(name) for name in glob.glob('/usr/share/novnc/app/images/icons/novnc-*x*.png')]" \
    && sed -i '/image\/svg+xml/d' /usr/share/novnc/vnc.html

ENV SUIT_PYTHON=/opt/manim/bin/python \
    PATH=/opt/manim/bin:$PATH \
    HOME=/home/suit \
    XDG_CACHE_HOME=/home/suit/.cache \
    SUIT_SCREEN=1600x900

# Whoever the editor ends up running as has to be able to write to these. And the start
# script has to be runnable wherever the files were checked out, Windows included.
RUN mkdir -p /decks /home/suit/.cache /tmp/.X11-unix \
    && chmod 1777 /decks /home/suit /home/suit/.cache /tmp/.X11-unix \
    && chmod 755 /usr/local/bin/suit-start

WORKDIR /decks
EXPOSE 6080
ENTRYPOINT ["suit-start"]
