<p align="center">
  <img src="assets/cartridge-icon.png" alt="Cartridge" width="128" height="128">
</p>

<h1 align="center">Cartridge</h1>

<p align="center">
  <em>Slot it in, everything's loaded.</em>
</p>

<p align="center">
  A portable, provider-agnostic container for AI-assisted development.<br>
  Ships as a Docker image. Works in K8s, docker compose or whereever you like your images.
</p>

---

## What's inside

Four AI harnesses, a programmatic API, a headless browser, a web terminal, and everything a coding agent needs to work autonomously.

| Category | Included |
|----------|----------|
| **AI Harnesses** | [Claude Code](https://github.com/anthropics/claude-code), [Pi](https://pi.dev), [OpenAI Codex](https://github.com/openai/codex), [Gemini CLI](https://github.com/google-gemini/gemini-cli) |
| **API** | HTTP + WebSocket on :4500 with interactive docs, browser terminal, inter-agent messaging |
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
```

Open http://localhost:7681 for the web terminal, or use the API:

```bash
# Start an agent
curl -X POST http://localhost:4500/api/agents \
  -H "Content-Type: application/json" \
  -d '{"provider":"claude","prompt":"fix the failing test"}'

# Open the browser terminal for that agent
# http://localhost:4500/api/agents/<id>/terminal

# Interactive API docs
# http://localhost:4500/api/docs
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

| Harness | Method | How |
|---------|--------|-----|
| **Claude Code** | API key | `ANTHROPIC_API_KEY` env var (onboarding auto-completed) |
| | Setup token | `CLAUDE_CODE_OAUTH_TOKEN` env var ([generate once](docs/examples/claude-code.md#headless-k8s--ci), onboarding auto-completed) |
| | OAuth login | `claude auth login` in terminal |
| **Pi** | Ollama | `OLLAMA_HOST` env var (auto-discovers models on boot) |
| | Any cloud provider | `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, or `GEMINI_API_KEY` |
| | Subscription | `/login` inside Pi (ChatGPT, Claude, GitHub Copilot) |
| **Codex** | API key | `OPENAI_API_KEY` env var |
| | Subscription | `/login` inside Codex (ChatGPT Plus/Pro) |
| **Gemini CLI** | API key | `GEMINI_API_KEY` env var |
| | Google OAuth | `gemini` then `/login` in terminal |

All env vars can also be set via [TOML config](docs/configuration.md).

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

## API

Cartridge includes an HTTP/WebSocket API on port 4500 for programmatic agent management. Orchestrators, dashboards, or other agents can start sessions, stream output, send follow-up prompts, and receive structured hook events.

| Feature | How |
|---------|-----|
| Start an agent | `POST /api/agents` |
| Stream output | `GET /api/agents/:id/ws` (WebSocket) |
| Send follow-up prompt | `POST /api/agents/:id/input` |
| Watch in browser | `GET /api/agents/:id/terminal` (xterm.js) |
| Agent-to-agent messaging | `POST /api/agents/:id/messages` |
| One-shot command | `POST /api/run` |
| Interactive docs | `GET /api/docs` (Scalar) |

Agents run in interactive mode with full TUI. Subscription auth works (no API key required). Hook plugins for Claude Code, Pi, and OpenCode stream structured events alongside the terminal output.

See **[docs/api.md](docs/api.md)** for the full reference.

## Ecosystem

- **[Barracks](https://github.com/Saturate/barracks)** - Orchestrator that runs Cartridge pods

## Docs

- **[API](docs/api.md)** - HTTP/WebSocket API reference
- **[Configuration](docs/configuration.md)** - Full env var + TOML reference with examples
- **[Tools](docs/tools.md)** - Everything installed in the image
- **[cartridge.example.toml](cartridge.example.toml)** - Example config file

### Provider examples

- **[Claude Code](docs/examples/claude-code.md)** - API key, subscription, headless, skills
- **[Ollama](docs/examples/ollama.md)** - Local/remote models, auto-discovery, compose stack
- **[OpenAI](docs/examples/openai.md)** - Codex CLI, Pi with GPT models
- **[Google Gemini](docs/examples/google.md)** - Gemini CLI, Gemma via Ollama
- **[Multi-Provider](docs/examples/multi-provider.md)** - All providers at once
- **[Kubernetes](docs/examples/kubernetes.md)** - Helm chart, production values, subscription auth
- **[Tunnels](docs/examples/tunnels.md)** - Tailscale, Cloudflare, SSH

## License

MIT
