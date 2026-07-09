# Included Tools

Everything installed in the Cartridge image.

## AI Harnesses

| Tool | Provider | Install |
|------|----------|---------|
| [Claude Code](https://github.com/anthropics/claude-code) | Anthropic | `claude` |
| [Pi](https://pi.dev) | 15+ (Ollama, Anthropic, OpenAI, Google, ...) | `pi` |
| [OpenAI Codex](https://github.com/openai/codex) | OpenAI | `codex` |
| [Gemini CLI](https://github.com/google-gemini/gemini-cli) | Google | `gemini` |

## Runtimes

| Tool | Version | Notes |
|------|---------|-------|
| Node.js | 24 (default), 22 | Managed via nvm. `nvm use 22`, `nvm install 20` |
| Bun | latest | |
| pnpm | latest | |
| Python 3 | system | pip, httpx pre-installed |
| TypeScript | latest | `tsx` for direct execution, `tsc` for type checking |

## CLI Tools

| Tool | Command | What it does |
|------|---------|-------------|
| ripgrep | `rg` | Fast text search |
| fd | `fd` | Fast file finder |
| bat | `bat` | Cat with syntax highlighting |
| fzf | `fzf` | Fuzzy finder |
| jq | `jq` | JSON processor |
| git | `git` | Version control |
| GitHub CLI | `gh` | PRs, issues, actions from the terminal |
| git-delta | `delta` | Better diffs (configured as git pager) |
| scc | `scc` | Code stats: lines, complexity, languages |
| curl / wget | `curl`, `wget` | HTTP clients |
| tmux | `tmux` | Terminal multiplexer |
| zsh | `zsh` | Default shell |

## Database Clients

| Tool | Command |
|------|---------|
| PostgreSQL | `psql` |
| SQLite | `sqlite3` |
| Redis | `redis-cli` |

## Media & Documents

| Tool | Command | What it does |
|------|---------|-------------|
| pandoc | `pandoc` | Convert between document formats |
| ImageMagick | `convert` | Image manipulation |
| ffmpeg | `ffmpeg` | Audio/video processing |

## Notifications

| Tool | Command | What it does |
|------|---------|-------------|
| shoutrrr | `shoutrrr send -u URL "msg"` | Slack, Discord, Teams, email, webhooks |

## Browser

| Component | Details |
|-----------|---------|
| Chromium | Playwright build, headless, CDP on :9222 |
| Xvfb | Virtual display at :99 (1920x1080) |
| x11vnc | VNC bridge (used by noVNC) |
| noVNC | Browser viewport at :6080 (opt-in) |

## Networking

| Tool | Details |
|------|---------|
| Tailscale | Mesh VPN (opt-in via `TAILSCALE_AUTHKEY`) |
| Cloudflare Tunnel | Public URLs (opt-in via `CF_TUNNEL_TOKEN`) |
| OpenSSH | SSH server (opt-in via `SSH_ENABLE`) |

## Services (s6-overlay)

| Service | Port | Default | Description |
|---------|------|---------|-------------|
| svc-xvfb | :99 | On | Virtual display for Chromium |
| svc-ttyd | :7681 | On | Web terminal (tmux session) |
| svc-chrome | :9222 | On | Headless Chromium with CDP |
| svc-state-sync | - | On | Periodic dotfile backup |
| svc-novnc | :6080 | Off | Live browser viewport |
| svc-sshd | :22 | Off | SSH access |
| svc-tailscale | - | Off | Tailscale mesh networking |
| svc-cloudflared | - | Off | Cloudflare tunnel |
