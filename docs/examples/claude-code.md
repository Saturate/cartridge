# Claude Code + Cartridge

Run Claude Code inside Cartridge with full browser, terminal, and tool access.

## API key

```yaml
# docker-compose.yaml
services:
  cartridge:
    build: .
    environment:
      - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
    ports:
      - "127.0.0.1:7681:7681"
    volumes:
      - ./workspace:/workspace
      - ./data/config:/home/dev/.claude
```

```bash
docker compose up -d
# Open http://localhost:7681
# Run: claude
```

## Subscription (Claude Pro/Max)

```bash
docker compose up -d
# Open http://localhost:7681
# Run: claude auth login
# Follow the URL, authenticate, done.
# Token persists in ./data/config/
```

## Headless (K8s / CI)

When `ANTHROPIC_API_KEY` or `CLAUDE_CODE_OAUTH_TOKEN` is set, Cartridge auto-completes Claude Code's first-run onboarding so there are no interactive prompts. Just set the credential and go.

```yaml
services:
  cartridge:
    build: .
    environment:
      - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
```

For subscription auth, generate a long-lived token once and pass it as an env var:

```bash
# Generate a token (valid for 1 year):
docker run --rm -it cartridge claude setup-token

# Use it:
docker run -e CLAUDE_CODE_OAUTH_TOKEN=sk-ant-oat01-... cartridge
```

## Non-interactive (piped)

```bash
docker run --rm \
  -e ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY} \
  -v $(pwd):/workspace \
  cartridge \
  bash -c 'source /usr/local/nvm/nvm.sh && claude -p "Explain what this project does"'
```

## Custom Claude Code config

For teams that want to pre-seed specific settings (theme, feature flags, trusted workspaces), mount a `.claude.json` template:

```yaml
volumes:
  - ./claude.json:/etc/cartridge/claude.json:ro
```

Or set the path via env var or TOML:

```yaml
environment:
  - CLAUDE_CONFIG_TEMPLATE=/etc/cartridge/claude.json
```

```toml
[claude]
config_template = "/etc/cartridge/claude.json"
```

The template is copied to `~/.claude.json` on first boot only. If `.claude.json` already exists (from a volume mount at `~/.claude/`), it's left untouched.

**Priority order:**
1. Existing `~/.claude.json` (volume mount) - used as-is
2. Template file (`CLAUDE_CONFIG_TEMPLATE` or `/etc/cartridge/claude.json`)
3. Auto-onboarding when auth credentials are present

## With skills and plugins

```yaml
services:
  cartridge:
    build: .
    environment:
      - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
      - SKILLS_INIT=true
    volumes:
      - ./workspace:/workspace
      - ./data/config:/home/dev/.claude
      - ./skills:/skills:ro
```

Claude Code picks up skills from `~/.claude/skills/` (symlinked from `/skills` on boot).

## Checking onboarding status

The API's status endpoint shows whether Claude Code is ready:

```bash
curl -s http://localhost:4500/api/status | jq .providers.claude
```

```json
{
  "available": true,
  "auth": "api_key"
}
```

## With HUSK telemetry

```yaml
# docker-compose.husk.yaml ships this pre-configured
docker compose -f docker-compose.husk.yaml up -d
```

Claude Code's OTel exporter sends traces to the HUSK sidecar.
Open HUSK at http://localhost:3000 to see session traces.
