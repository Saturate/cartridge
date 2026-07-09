<p align="center">
  <img src="assets/cartridge-icon.png" alt="Cartridge" width="128" height="128" style="image-rendering: pixelated;">
</p>

<h1 align="center">Cartridge</h1>

<p align="center">
  <em>Slot it in, everything's loaded.</em>
</p>

<p align="center">
  A portable, provider-agnostic container for AI-assisted development.<br>
  Ships as a Docker image. Works standalone, with Eye, or in K8s.
</p>

---

## What's inside

Four AI harnesses, a headless browser, a web terminal, and everything a coding agent needs to work autonomously.

| Category | Included |
|----------|----------|
| **AI Harnesses** | Claude Code, Pi, OpenAI Codex, Google Gemini CLI |
| **Browser** | Playwright Chromium + Xvfb + CDP on :9222, optional noVNC |
| **Terminal** | ttyd on :7681, tmux, zsh |
| **Node.js** | nvm with Node 24 + 22, pnpm, Bun, TypeScript |
| **Python** | Python 3, pip, httpx |
| **CLI Tools** | rg, fd, bat, fzf, jq, gh, delta, scc, git |
| **Media** | pandoc, imagemagick, ffmpeg |
| **DB Clients** | psql, sqlite3, redis-cli |
| **Notifications** | shoutrrr (Slack, Discord, email, webhooks) |
| **Tunnels** | Tailscale, Cloudflare Tunnels |
| **Supervisor** | s6-overlay v3 |

## Quick start

```bash
git clone https://github.com/Saturate/cartridge.git
cd cartridge
docker compose up -d
# Open http://localhost:7681
```

Run `cartridge-status` inside the container to see what's configured.

## Configuration

Two ways, combinable. Env vars always override TOML values.

**Env vars** (docker-compose.yaml):
```yaml
environment:
  - ANTHROPIC_API_KEY=sk-ant-...
  - OLLAMA_HOST=http://host.docker.internal:11434
```

**TOML file** (mount at `/etc/cartridge/config.toml`):
```toml
[providers]
ollama_host = "http://10.106.20.134:11434"

[tunnels]
tailscale_authkey = "tskey-auth-..."
tailscale_hostname = "cartridge-dev"

[notifications]
shoutrrr_url = "slack://token-a/token-b/token-c"
```

See `cartridge.example.toml` for all options.

## Provider auth

| Method | How | Best for |
|--------|-----|----------|
| API key | `ANTHROPIC_API_KEY` env var | CI, fleet, org billing |
| Setup token | `claude setup-token` in terminal | Headless K8s pods |
| OAuth login | `claude auth login` in terminal | Local dev |
| Ollama | `OLLAMA_HOST` env var | Local/remote models, no key needed |

Pi auto-configures for Ollama on boot (discovers models, writes config).

## Deploy

**Docker Compose** (standalone):
```bash
docker compose up -d
```

**Docker Compose** (with HUSK telemetry + Eye):
```bash
docker compose -f docker-compose.husk.yaml up -d
```

**Helm** (K8s, one pod per session):
```bash
helm install dev helm/cartridge/ \
  --set providers.ollamaHost=http://ollama:11434 \
  --set tunnels.tailscale.authkey=tskey-auth-...
```

## Architecture

```
s6-overlay (PID 1)
  ├── ttyd              :7681   web terminal
  ├── xvfb              :99     virtual display
  ├── chrome            :9222   browser CDP
  ├── state-sync                dotfile persistence
  ├── novnc             :6080   browser viewport (opt-in)
  ├── sshd              :22     SSH access (opt-in)
  ├── tailscale                 mesh networking (opt-in)
  └── cloudflared               tunnel (opt-in)
```

## Ecosystem

- **[Eye](https://github.com/Saturate/eye)** - Agent board UI that manages sessions inside Cartridge
- **[HUSK](https://github.com/Saturate/husk)** - OTel telemetry sidecar
- **[Barracks](https://github.com/Saturate/barracks)** - Orchestrator that runs Cartridge pods

## Docs

- **[USAGE.md](USAGE.md)** - Full configuration reference
- **[cartridge.example.toml](cartridge.example.toml)** - All config options
- **[test/integration.sh](test/integration.sh)** - Integration test suite (62 assertions)

## License

MIT
