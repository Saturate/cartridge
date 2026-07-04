# Cartridge - AI Dev Harness Container
# Build: docker build -t cartridge .

FROM debian:bookworm-slim

LABEL org.opencontainers.image.source=https://github.com/Saturate/cartridge

ARG S6_OVERLAY_VERSION=3.2.3.0
ARG TARGETARCH

ENV DEBIAN_FRONTEND=noninteractive \
    LANG=en_US.UTF-8 \
    LC_ALL=en_US.UTF-8 \
    DISPLAY=:99 \
    CHROMIUM_FLAGS="--no-sandbox --disable-gpu --disable-dev-shm-usage" \
    CHROME_PATH=/usr/bin/chromium

# TODO: System packages, s6-overlay, Node.js, Python, AI CLIs, ttyd
# See DESIGN.md for the full install list.
# This is the scaffold - implementation follows.

RUN echo "Cartridge scaffold - not yet built" > /etc/cartridge-version

WORKDIR /workspace
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
