# Cartridge - AI Dev Harness Container

Codename: Cartridge (slot it in, everything's loaded)

## What it is

A portable, provider-agnostic container for AI-assisted development and security
engagements. Ships as a Docker image. Works standalone, with PromptKiddie, or in K8s.
Provides the infrastructure agents need: terminals, browser, display, persistence,
and telemetry.

## Architecture

```
s6-overlay (PID 1)
  ├── ttyd              :7681   web terminal (MIT, single binary)
  ├── xvfb              :99     virtual display for Chromium
  ├── chrome-mcp        :9222   browser MCP server (CDP)
  ├── state-sync                persist session state to volume
  ├── novnc             :6080   optional live browser viewport
  └── sshd              :22     optional, key-only, off by default

Volumes
  ├── /workspace                project files (bind mount)
  ├── /home/dev/.claude         claude config, skills, plugins (bind mount)
  ├── /home/dev/.config         provider configs (codex, gemini, pi, etc.)
  └── /skills                   custom skills mount (optional, read-only)
```

## Design Decisions

### Base Image: debian bookworm-slim
Not a Node-specific base. Install Node 22 LTS + Python 3 + Bun on top.
Provider-agnostic (Pi, future providers may use different runtimes).
Bookworm-slim over Alpine because Chromium, node-pty, and native npm packages need glibc.

### Process Supervisor: s6-overlay v3
Proven in HolyClaude. Handles zombie reaping, signal forwarding, crash restart.
Works in both Docker and K8s. Conditional service registration via marker files.

### Web UI: ttyd + Eye
ttyd (MIT, 2 MB) as the fallback web terminal. Eye (the agent board) as
the primary interface when available. Eye runs either inside the container
as an s6 service or as a separate container/service.

### Browser: Chromium + Xvfb + Chrome MCP
Chrome DevTools MCP server on port 9222 for AI agent browser control.
Xvfb at :99 for headless rendering. Playwright pre-installed.
Optional noVNC for live browser viewport viewing.

### HUSK Integration
Claude Code's built-in OTel exporter sends traces to HUSK_ENDPOINT.
HUSK runs as a sidecar (same pod in K8s, same compose stack in Docker).
HUSK MCP server registered in Claude Code config for mid-session queries.
If HUSK_ENDPOINT is unset, OTel is not configured. No hard dependency.

### Skills and Plugins
Mount at /skills (read-only). Bootstrap symlinks into ~/.claude/skills/.
Controlled by SKILLS_INIT env var. Supports the Saturate/agents repo.

### PromptKiddie Integration
Opt-in via PK_MODE=true. Installs pk CLI, configures DATABASE_URL,
merges PK's CLAUDE.md and skills. Default is off.

### Multi-Provider Support
Claude Code, Codex, Pi, Gemini CLI installed at build time.
Config directories symlinked into persistent volume.
Provider API keys flow through env vars.

## Boot Sequence

1. entrypoint.sh: UID/GID remap, workspace ownership, state restore
2. bootstrap.sh (first boot only): copy configs, wire HUSK, init skills
3. s6-overlay takes over: starts all registered services

## What Gets Installed

| Category     | Packages                                          |
|-------------|---------------------------------------------------|
| AI CLIs     | claude, codex, pi, gemini-cli                     |
| Browser     | chromium, xvfb, playwright                         |
| Terminal    | ttyd, tmux, zsh                                    |
| Core CLI    | git, curl, jq, ripgrep, fd, fzf, bat              |
| Node.js     | node 22, pnpm, typescript, tsx                     |
| Python      | python3, pip, httpx, playwright                    |
| DB CLIs     | psql, sqlite3, redis-cli                           |
| Network     | openssh, mosh                                      |
| VNC         | x11vnc, noVNC (optional)                           |

## Docker Compose

Standard: cartridge alone with ttyd
With HUSK: cartridge + husk sidecar
With Eye: cartridge + eye + husk

## K8s / K3s

Same image. HUSK as sidecar in pod. PVCs for state.
One pod = one developer session. StatefulSet for stable DNS.

## Relationship to Other Projects

- **Eye**: The board that manages agents inside Cartridge. Can run inside or outside.
- **HUSK**: OTel telemetry sidecar. Optional dependency.
- **PromptKiddie**: Security engagement workspace. Opt-in via PK_MODE.
- **Saturate/agents**: Skills and agent definitions. Mounted at /skills.
