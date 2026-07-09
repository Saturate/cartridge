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

```bash
# Generate a long-lived token once:
docker run --rm -it cartridge claude setup-token

# Then use it in your pod spec or compose:
# Mount the token file as a K8s Secret at /home/dev/.claude/
```

## Non-interactive (piped)

```bash
docker run --rm \
  -e ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY} \
  -v $(pwd):/workspace \
  cartridge \
  bash -c 'source /usr/local/nvm/nvm.sh && claude -p "Explain what this project does"'
```

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

## With HUSK telemetry

```yaml
# docker-compose.husk.yaml ships this pre-configured
docker compose -f docker-compose.husk.yaml up -d
```

Claude Code's OTel exporter sends traces to the HUSK sidecar.
Open HUSK at http://localhost:3000 to see session traces.
