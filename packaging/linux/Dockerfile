# What Suit's RPM is built with: Fedora's own Rust and packaging tools. Nothing of Suit is
# in this image; the source is handed to it when it runs (see build-rpm.sh).
ARG FEDORA=44
FROM fedora:${FEDORA}
RUN dnf install -y rust cargo gcc rpm-build desktop-file-utils rpmlint \
    && dnf clean all
