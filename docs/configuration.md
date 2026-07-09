# Configuration

Cartridge reads configuration from two sources. Env vars always override TOML values.

| Source | How | Best for |
|--------|-----|----------|
| **Env vars** | `docker-compose.yaml` or `docker run -e` | CI, compose, K8s |
| **TOML file** | Mount at `/etc/cartridge/config.toml` or place in `/workspace/cartridge.toml` | Humans, version control |

```bash
# Env var
docker run -e OLLAMA_HOST=http://host:11434 ...

# TOML mount
docker run -v ./cartridge.toml:/etc/cartridge/config.toml:ro ...
```

## Full reference

Every configuration option, its env var, TOML key, and what it does.

### Providers

| Env var | TOML key | Example | Description |
|---------|----------|---------|-------------|
| `ANTHROPIC_API_KEY` | `providers.anthropic_api_key` | `sk-ant-api03-...` | Anthropic API key for Claude Code and Pi |
| `OPENAI_API_KEY` | `providers.openai_api_key` | `sk-proj-...` | OpenAI API key for Codex and Pi |
| `GEMINI_API_KEY` | `providers.gemini_api_key` | `AIza...` | Google Gemini API key for Gemini CLI and Pi |
| `OLLAMA_HOST` | `providers.ollama_host` | `http://10.0.0.5:11434` | Ollama server URL. Bootstrap auto-discovers models and configures Pi |
| `OLLAMA_MODELS` | `providers.ollama_models` | `gemma4:31b-it-bf16,llama3.2` | Comma-separated model list. Omit to auto-discover from server |

### GitHub

| Env var | TOML key | Example | Description |
|---------|----------|---------|-------------|
| `GH_TOKEN` | `github.token` | `ghp_xxxx...` | GitHub personal access token for `gh` CLI |
| `GITHUB_TOKEN` | *(no TOML)* | `ghp_xxxx...` | Alternative GitHub token (env var only) |

### Workspace

| Env var | TOML key | Example | Description |
|---------|----------|---------|-------------|
| `GIT_REPO_URL` | `workspace.repo_url` | `https://github.com/org/repo` | Clone this repo into `/workspace` on first boot |
| `GIT_BRANCH` | `workspace.branch` | `main` | Branch to clone. Default: repo default branch |
| `GIT_WORKTREE_REPO` | `workspace.worktree_repo` | `https://github.com/org/repo` | Clone for git worktree use |
| `GIT_WORKTREE_BRANCH` | `workspace.worktree_branch` | `feature-x` | Worktree branch. Default: `main` |

If no workspace config is set and `/workspace` is empty, bootstrap runs `git init`.

### SSH

| Env var | TOML key | Example | Description |
|---------|----------|---------|-------------|
| `SSH_ENABLE` | `ssh.enable` | `true` | Enable sshd on port 22 |
| `SSH_AUTHORIZED_KEYS` | `ssh.authorized_keys` | `ssh-ed25519 AAAA... user@host` | Public key(s) for the dev user. Newline-separated for multiple keys |

### Tunnels

| Env var | TOML key | Example | Description |
|---------|----------|---------|-------------|
| `TAILSCALE_AUTHKEY` | `tunnels.tailscale_authkey` | `tskey-auth-k...` | Tailscale auth key. Enables the tailscale service |
| `TAILSCALE_HOSTNAME` | `tunnels.tailscale_hostname` | `cartridge-dev` | Hostname on the tailnet |
| `TAILSCALE_TAGS` | `tunnels.tailscale_tags` | `tag:dev` | ACL tags for the node |
| `CF_TUNNEL_TOKEN` | `tunnels.cf_tunnel_token` | `eyJhIjoiNmQ...` | Cloudflare Tunnel token. Enables the cloudflared service |

### Notifications

| Env var | TOML key | Example | Description |
|---------|----------|---------|-------------|
| `SHOUTRRR_URL` | `notifications.shoutrrr_url` | `slack://token/channel` | Notification URL. Supports Slack, Discord, Teams, email, webhooks |

Usage: `shoutrrr send -u "$SHOUTRRR_URL" "message"`

### Integrations

| Env var | TOML key | Example | Description |
|---------|----------|---------|-------------|
| `HUSK_ENDPOINT` | `integrations.husk_endpoint` | `http://husk:3000` | HUSK OTel telemetry endpoint |
| `HUSK_API_KEY` | `integrations.husk_api_key` | `husk-key-...` | HUSK API key |
| `SKILLS_INIT` | `integrations.skills_init` | `true` | Symlink `/skills` into Claude Code config |
| `PK_MODE` | `integrations.pk_mode` | `true` | Enable PromptKiddie integration |

### Container

| Env var | TOML key | Default | Description |
|---------|----------|---------|-------------|
| `PUID` | *(no TOML)* | `1000` | UID for the dev user |
| `PGID` | *(no TOML)* | `1000` | GID for the dev user |
| `TZ` | `container.tz` | `UTC` | Timezone |
| `NOVNC_ENABLE` | `container.novnc_enable` | unset | Set `true` to enable noVNC on :6080 |

## Ports

| Port | Service | Default |
|------|---------|---------|
| 7681 | ttyd (web terminal) | Always on |
| 9222 | Chrome CDP (browser control) | Always on |
| 6080 | noVNC (live browser viewport) | Off, `NOVNC_ENABLE=true` |
| 22 | SSH | Off, `SSH_ENABLE=true` |

## Volumes

| Container path | Purpose |
|----------------|---------|
| `/home/dev/.claude` | Claude Code config, auth tokens, session state |
| `/workspace` | Project files |
| `/skills` | Skills and plugins (read-only mount) |
| `/etc/cartridge/config.toml` | TOML config file (read-only mount) |

## TOML example

Full example with all sections:

```toml
[providers]
anthropic_api_key = "sk-ant-api03-..."
ollama_host = "http://10.106.20.134:11434"
# ollama_models = ["gemma4:31b-it-bf16", "llama3.2"]

[github]
token = "ghp_..."

[workspace]
repo_url = "https://github.com/org/repo"
branch = "main"

[ssh]
enable = true
authorized_keys = """
ssh-ed25519 AAAA... dev@laptop
ssh-rsa AAAA... other@desktop
"""

[tunnels]
tailscale_authkey = "tskey-auth-..."
tailscale_hostname = "cartridge-dev"
# cf_tunnel_token = "eyJ..."

[notifications]
shoutrrr_url = "slack://token-a/token-b/token-c"

[integrations]
husk_endpoint = "http://husk:3000"
skills_init = true

[container]
tz = "Europe/Copenhagen"
# novnc_enable = true
```

## Compose example

Equivalent in `docker-compose.yaml`:

```yaml
services:
  cartridge:
    build: .
    shm_size: 2g
    cap_add: [SYS_ADMIN, SYS_PTRACE]
    security_opt: [seccomp=unconfined]
    ports:
      - "127.0.0.1:7681:7681"
    volumes:
      - ./data/config:/home/dev/.claude
      - ./workspace:/workspace
    environment:
      - TZ=Europe/Copenhagen
      - PUID=1000
      - PGID=1000
      - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
      - OLLAMA_HOST=http://host.docker.internal:11434
      - GH_TOKEN=${GH_TOKEN}
      - GIT_REPO_URL=https://github.com/org/repo
      - GIT_BRANCH=main
      - SSH_ENABLE=true
      - SSH_AUTHORIZED_KEYS=ssh-ed25519 AAAA... dev@laptop
      - TAILSCALE_AUTHKEY=${TAILSCALE_AUTHKEY}
      - TAILSCALE_HOSTNAME=cartridge-dev
      - SHOUTRRR_URL=${SHOUTRRR_URL}
      - SKILLS_INIT=true
```
