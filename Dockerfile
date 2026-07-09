# Cartridge - AI Dev Harness Container
# Build: docker build -t cartridge .
# Run:   docker compose up -d

FROM debian:bookworm-slim

LABEL org.opencontainers.image.source=https://github.com/Saturate/cartridge

ARG S6_OVERLAY_VERSION=3.2.3.0
ARG NODE_DEFAULT=24
ARG TTYD_VERSION=1.7.7
ARG TARGETARCH

ENV DEBIAN_FRONTEND=noninteractive \
    LANG=en_US.UTF-8 \
    LC_ALL=en_US.UTF-8 \
    DISPLAY=:99 \
    CHROMIUM_FLAGS="--no-sandbox --disable-gpu --disable-dev-shm-usage" \
    CHROME_PATH=/usr/bin/chromium \
    S6_KEEP_ENV=1 \
    S6_BEHAVIOUR_IF_STAGE2_FAILS=2 \
    S6_CMD_WAIT_FOR_SERVICES_MAXTIME=30000

# ── System packages ──────────────────────────────────────────────
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates curl wget gnupg xz-utils \
      locales \
      git git-lfs \
      jq \
      tmux zsh \
      openssh-server \
      dbus dbus-x11 \
      chromium xvfb x11vnc \
      fonts-liberation \
      python3 python3-pip python3-venv \
      sqlite3 redis-tools postgresql-client \
      unzip \
      procps htop less \
    && sed -i '/en_US.UTF-8/s/^# //g' /etc/locale.gen && locale-gen \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# ── s6-overlay v3 ────────────────────────────────────────────────
RUN case "${TARGETARCH}" in \
      amd64) S6_ARCH="x86_64" ;; \
      arm64) S6_ARCH="aarch64" ;; \
      *) echo "Unsupported arch: ${TARGETARCH}" && exit 1 ;; \
    esac \
    && curl -fsSL "https://github.com/just-containers/s6-overlay/releases/download/v${S6_OVERLAY_VERSION}/s6-overlay-noarch.tar.xz" \
       | tar -C / -Jxpf - \
    && curl -fsSL "https://github.com/just-containers/s6-overlay/releases/download/v${S6_OVERLAY_VERSION}/s6-overlay-${S6_ARCH}.tar.xz" \
       | tar -C / -Jxpf -

# ── nvm + Node.js ────────────────────────────────────────────────
ENV NVM_DIR=/usr/local/nvm
RUN mkdir -p "$NVM_DIR" \
    && curl -fsSL https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.3/install.sh | bash \
    && . "$NVM_DIR/nvm.sh" \
    && nvm install ${NODE_DEFAULT} \
    && nvm install 22 \
    && nvm alias default ${NODE_DEFAULT} \
    && nvm use default \
    && npm install -g pnpm \
    && ln -sf "$NVM_DIR/versions/node/$(nvm version default)/bin/node" /usr/local/bin/node \
    && ln -sf "$NVM_DIR/versions/node/$(nvm version default)/bin/npm" /usr/local/bin/npm \
    && ln -sf "$NVM_DIR/versions/node/$(nvm version default)/bin/npx" /usr/local/bin/npx \
    && ln -sf "$NVM_DIR/versions/node/$(nvm version default)/bin/pnpm" /usr/local/bin/pnpm

# ── Core CLI tools ───────────────────────────────────────────────
RUN apt-get update && apt-get install -y --no-install-recommends \
      ripgrep fd-find bat fzf \
      imagemagick ffmpeg pandoc \
    && ln -sf /usr/bin/batcat /usr/local/bin/bat \
    && ln -sf /usr/bin/fdfind /usr/local/bin/fd \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# ── GitHub CLI ───────────────────────────────────────────────────
RUN curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
      -o /usr/share/keyrings/githubcli-archive-keyring.gpg \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
      > /etc/apt/sources.list.d/github-cli.list \
    && apt-get update && apt-get install -y --no-install-recommends gh \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# ── git-delta, scc, shoutrrr ─────────────────────────────────────
RUN case "${TARGETARCH}" in \
      amd64) DELTA_ARCH="x86_64-unknown-linux-musl"; SCC_ARCH="x86_64"; SHOUT_ARCH="amd64" ;; \
      arm64) DELTA_ARCH="aarch64-unknown-linux-gnu"; SCC_ARCH="arm64"; SHOUT_ARCH="arm64" ;; \
    esac \
    && curl -fsSL "https://github.com/dandavison/delta/releases/download/0.18.2/delta-0.18.2-${DELTA_ARCH}.tar.gz" \
      | tar -xz --strip-components=1 -C /usr/local/bin/ "delta-0.18.2-${DELTA_ARCH}/delta" \
    && curl -fsSL "https://github.com/boyter/scc/releases/download/v3.7.0/scc_Linux_${SCC_ARCH}.tar.gz" \
      | tar -xz -C /usr/local/bin/ scc \
    && curl -fsSL "https://github.com/containrrr/shoutrrr/releases/download/v0.8.0/shoutrrr_linux_${SHOUT_ARCH}.tar.gz" \
      | tar -xz -C /usr/local/bin/ shoutrrr \
    && chmod +x /usr/local/bin/delta /usr/local/bin/scc /usr/local/bin/shoutrrr

# ── Bun ──────────────────────────────────────────────────────────
RUN curl -fsSL https://bun.sh/install | BUN_INSTALL=/usr/local bash

# ── ttyd ─────────────────────────────────────────────────────────
RUN case "${TARGETARCH}" in \
      amd64) TTYD_ARCH="x86_64" ;; \
      arm64) TTYD_ARCH="aarch64" ;; \
    esac \
    && curl -fsSL -o /usr/local/bin/ttyd \
       "https://github.com/tsl0922/ttyd/releases/download/${TTYD_VERSION}/ttyd.${TTYD_ARCH}" \
    && chmod +x /usr/local/bin/ttyd

# ── noVNC ────────────────────────────────────────────────────────
RUN git clone --depth 1 https://github.com/novnc/noVNC.git /opt/novnc \
    && git clone --depth 1 https://github.com/novnc/websockify.git /opt/novnc/utils/websockify \
    && ln -sf /opt/novnc/vnc.html /opt/novnc/index.html \
    && rm -rf /opt/novnc/.git /opt/novnc/utils/websockify/.git

# ── Dev user ─────────────────────────────────────────────────────
RUN groupadd -g 1000 dev \
    && useradd -m -u 1000 -g dev -s /bin/zsh dev \
    && mkdir -p /workspace /skills \
       /home/dev/.claude /home/dev/.config /home/dev/.local/share \
       /home/dev/.cache/fontconfig \
       /home/dev/.local/share/chromium/Crashpad \
    && chown -R dev:dev /home/dev /workspace \
    && fc-cache -f 2>/dev/null || true

# ── SSH (key-only, disabled by default) ──────────────────────────
RUN mkdir -p /run/sshd \
    && sed -i 's/#PasswordAuthentication yes/PasswordAuthentication no/' /etc/ssh/sshd_config \
    && sed -i 's/#PubkeyAuthentication yes/PubkeyAuthentication yes/' /etc/ssh/sshd_config

# ── Tailscale + Cloudflared (tunneling, opt-in) ─────────────────
RUN curl -fsSL https://pkgs.tailscale.com/stable/debian/bookworm.noarmor.gpg \
      -o /usr/share/keyrings/tailscale-archive-keyring.gpg \
    && echo "deb [signed-by=/usr/share/keyrings/tailscale-archive-keyring.gpg] https://pkgs.tailscale.com/stable/debian bookworm main" \
      > /etc/apt/sources.list.d/tailscale.list \
    && apt-get update && apt-get install -y --no-install-recommends tailscale \
    && apt-get clean && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /var/run/tailscale /var/lib/tailscale
RUN case "${TARGETARCH}" in \
      amd64) CF_ARCH="amd64" ;; \
      arm64) CF_ARCH="arm64" ;; \
    esac \
    && curl -fsSL -o /usr/local/bin/cloudflared \
       "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-${CF_ARCH}" \
    && chmod +x /usr/local/bin/cloudflared

# ── AI CLIs + global tooling ─────────────────────────────────────
RUN . "$NVM_DIR/nvm.sh" \
    && npm install -g \
       @anthropic-ai/claude-code \
       @openai/codex \
       @google/gemini-cli \
       --ignore-scripts @earendil-works/pi-coding-agent \
       typescript tsx \
    && cd "$(npm root -g)/@anthropic-ai/claude-code" && node install.cjs

# ── Playwright Chromium (container-friendly build) ───────────────
RUN . "$NVM_DIR/nvm.sh" && npx -y playwright install chromium \
    && npx -y playwright install-deps chromium \
    && mv /root/.cache/ms-playwright /opt/playwright \
    && ln -sf "$(find /opt/playwright -name chrome -path '*/chrome-linux/*' -type f | head -1)" \
       /usr/local/bin/chrome-playwright \
    && chmod -R o+rx /opt/playwright \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# ── Python packages ──────────────────────────────────────────────
RUN pip3 install --break-system-packages httpx

# ── rootfs overlay (s6 services, entrypoint, bootstrap) ─────────
COPY rootfs/ /

RUN chmod +x /usr/local/bin/entrypoint.sh /usr/local/bin/bootstrap.sh \
       /usr/local/bin/cartridge-status /usr/local/bin/cartridge-config \
    && find /etc/s6-overlay -name "run" -exec chmod +x {} \;

WORKDIR /workspace
EXPOSE 7681 9222 6080 22

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
