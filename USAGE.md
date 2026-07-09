# Cartridge - Usage Guide

## Quick Start

```bash
docker compose up -d
# Open http://localhost:7681 for the web terminal
```

Run `cartridge-status` inside the container to see what's configured.

## Authentication

Cartridge supports multiple auth paths. Set env vars in `docker-compose.yaml`
or pass them at runtime. Bootstrap auto-wires providers on every boot.

### Claude Code

Three options, from simplest to most flexible:

| Method | How | Best for |
|--------|-----|----------|
| API key | `ANTHROPIC_API_KEY=sk-ant-...` | CI, fleet, org billing |
| Setup token | Run `claude setup-token` in the terminal | Headless pods, K8s |
| OAuth login | Run `claude auth login` in the terminal | Local dev, interactive |

API keys work immediately via env var. The other two store credentials in
`~/.claude/` which is persisted via the volume mount at `./data/config`.

For K8s: mount a Secret containing the auth files at `/home/dev/.claude/`.

### Pi (provider-agnostic)

Pi works with 15+ providers. The most common setups:

**Ollama (local/remote, no API key needed):**
```yaml
environment:
  - OLLAMA_HOST=http://10.106.20.134:11434
  # Optional: specify models (otherwise auto-discovered)
  - OLLAMA_MODELS=gemma4:31b-it-bf16,llama3.2
```

Bootstrap writes `~/.pi/agent/models.json` automatically.

**Cloud providers (API key):**
```yaml
environment:
  - ANTHROPIC_API_KEY=sk-ant-...   # Pi uses this too
  - OPENAI_API_KEY=sk-...
  - GEMINI_API_KEY=...
```

**Subscription (interactive):**
Run `pi` in the terminal and use `/login` to authenticate via OAuth.

### GitHub CLI

```yaml
environment:
  - GH_TOKEN=ghp_...
```

Or run `gh auth login` interactively.

### Notifications (shoutrrr)

```yaml
environment:
  - SHOUTRRR_URL=slack://token-a/token-b/token-c
```

Send notifications from inside the container:
```bash
shoutrrr send -u "$SHOUTRRR_URL" "Build complete"
```

Supported: Slack, Discord, Teams, email (SMTP), Gotify, Pushover, webhooks.

### Workspace

The workspace (`/workspace`) is auto-configured on boot:

**Auto-clone a repo:**
```yaml
environment:
  - GIT_REPO_URL=https://github.com/org/repo
  - GIT_BRANCH=main  # optional, defaults to repo default
```

**Git worktree (work on a branch without affecting the main checkout):**
```yaml
environment:
  - GIT_WORKTREE_REPO=https://github.com/org/repo
  - GIT_WORKTREE_BRANCH=feature-branch
```

**No config:** if the workspace is empty and no repo is configured,
bootstrap runs `git init` so agents have git available from the start.

If the workspace already contains files or a `.git` directory, bootstrap
leaves it alone.

### Tunnels

Make the container reachable from anywhere without port forwarding.

**Tailscale (private mesh):**
```yaml
environment:
  - TAILSCALE_AUTHKEY=tskey-auth-...
  - TAILSCALE_HOSTNAME=cartridge-dev  # optional
  - TAILSCALE_TAGS=tag:dev            # optional ACL tags
```

Generate an auth key at https://login.tailscale.com/admin/settings/keys.
The container joins your tailnet and gets a stable IP. Access ttyd at
`http://cartridge-dev:7681` from any device on the tailnet.

**Cloudflare Tunnel (public URLs):**
```yaml
environment:
  - CF_TUNNEL_TOKEN=eyJ...
```

Create a tunnel at https://one.dash.cloudflare.com/ and configure it to
route to `http://localhost:7681` (ttyd) or other container ports. The
container gets a public `*.cfargotunnel.com` URL or your custom domain.

**Neither is required.** Without tunnel config, services are only
accessible via the mapped Docker ports.

## Environment Variables Reference

### Provider Auth

| Variable | Used by | Description |
|----------|---------|-------------|
| `ANTHROPIC_API_KEY` | Claude Code, Pi | Anthropic API key |
| `OPENAI_API_KEY` | Pi | OpenAI API key |
| `GEMINI_API_KEY` | Pi | Google Gemini API key |
| `OLLAMA_HOST` | Pi (via bootstrap) | Ollama server URL (e.g. `http://host:11434`) |
| `OLLAMA_MODELS` | Pi (via bootstrap) | Comma-separated model list (auto-discovered if empty) |
| `GH_TOKEN` | gh CLI | GitHub personal access token |
| `GITHUB_TOKEN` | gh CLI | Alternative GitHub token variable |

### Workspace

| Variable | Description |
|----------|-------------|
| `GIT_REPO_URL` | Clone this repo into /workspace on first boot |
| `GIT_BRANCH` | Branch to clone (default: repo default) |
| `GIT_WORKTREE_REPO` | Create a worktree from this repo |
| `GIT_WORKTREE_BRANCH` | Branch for the worktree (default: `main`) |

### Tunnels

| Variable | Description |
|----------|-------------|
| `TAILSCALE_AUTHKEY` | Tailscale auth key (enables tailscale service) |
| `TAILSCALE_HOSTNAME` | Hostname on the tailnet (optional) |
| `TAILSCALE_TAGS` | ACL tags, e.g. `tag:dev` (optional) |
| `CF_TUNNEL_TOKEN` | Cloudflare Tunnel token (enables cloudflared service) |

### Container Config

| Variable | Default | Description |
|----------|---------|-------------|
| `PUID` | `1000` | UID for the dev user |
| `PGID` | `1000` | GID for the dev user |
| `TZ` | `UTC` | Timezone |
| `NOVNC_ENABLE` | unset | Set to `true` to enable noVNC on :6080 |
| `SSH_ENABLE` | unset | Set to `true` to enable sshd on :22 |
| `SSH_AUTHORIZED_KEYS` | unset | Public key(s) to install for the dev user |
| `SKILLS_INIT` | unset | Set to `true` to symlink /skills into Claude config |
| `PK_MODE` | unset | Set to `true` for PromptKiddie integration |

### Telemetry & Notifications

| Variable | Description |
|----------|-------------|
| `HUSK_ENDPOINT` | HUSK OTel endpoint (e.g. `http://husk:3000`) |
| `HUSK_API_KEY` | HUSK API key |
| `SHOUTRRR_URL` | Notification URL (see shoutrrr docs) |

## Ports

| Port | Service | Always on? |
|------|---------|------------|
| 7681 | ttyd (web terminal) | Yes |
| 9222 | Chrome CDP | Yes |
| 6080 | noVNC (browser viewport) | `NOVNC_ENABLE=true` |
| 22 | SSH | `SSH_ENABLE=true` |

## Volumes

| Container path | Purpose |
|----------------|---------|
| `/home/dev/.claude` | Claude Code config, auth, session state |
| `/workspace` | Project files |
| `/skills` | Skills/plugins (read-only mount) |

## Node.js (nvm)

Node 24 (Krypton LTS) is the default. Node 22 (Jod LTS) is also installed.

```bash
nvm use 22          # switch to Node 22
nvm use 24          # switch back to Node 24
nvm install 20      # install another version
```

## Tools

### AI Harnesses
- `claude` - Claude Code (Anthropic)
- `pi` - Pi coding agent (15+ providers including Ollama)

### Development
- `node`, `npm`, `pnpm`, `bun`, `tsx`, `tsc` - JavaScript/TypeScript
- `python3`, `pip3` - Python
- `git`, `gh` - Version control
- `delta` - Better git diffs (configured as default pager)
- `tmux`, `zsh` - Terminal

### CLI Tools
- `rg` (ripgrep), `fd`, `bat`, `fzf`, `jq` - Search and filter
- `scc` - Code stats (lines, complexity, languages)
- `curl`, `wget` - HTTP
- `sqlite3`, `psql`, `redis-cli` - Database clients

### Media & Documents
- `pandoc` - Document conversion
- `imagemagick` (`convert`) - Image manipulation
- `ffmpeg` - Media processing

### Notifications
- `shoutrrr` - Send to Slack, Discord, email, webhooks

## Docker Compose Variants

**Standalone** (`docker-compose.yaml`):
```bash
docker compose up -d
```

**With HUSK telemetry + Eye** (`docker-compose.husk.yaml`):
```bash
docker compose -f docker-compose.husk.yaml up -d
```

## K8s Deployment

The same image works in K8s. Key considerations:

- One pod per developer session (StatefulSet for stable DNS)
- PVC for `/home/dev/.claude` (auth + state persistence)
- PVC for `/workspace` (project files)
- HUSK as a sidecar container in the pod
- Auth via K8s Secrets mounted as volumes or env vars

For subscription-based auth in K8s, use `claude setup-token` to generate a
long-lived token that doesn't require browser redirect.

## Status Check

Run inside the container:
```bash
cartridge-status
```

Shows: running services, authenticated providers, notification config, and
runtime versions.
