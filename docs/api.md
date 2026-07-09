# API

Cartridge includes an HTTP/WebSocket API on port 4500 for programmatic agent management. The API runs as an s6 service (`svc-api`) and starts automatically.

## Quick start

```bash
# Health check
curl http://localhost:4500/api/health

# Run a one-shot command
curl -X POST http://localhost:4500/api/run \
  -H "Content-Type: application/json" \
  -d '{"command":["echo","hello"]}'

# Start an agent
curl -X POST http://localhost:4500/api/agents \
  -H "Content-Type: application/json" \
  -d '{"provider":"claude","prompt":"fix the failing test"}'

# Interactive docs
open http://localhost:4500/api/docs
```

## Authentication

No auth by default (internal network). Set `CARTRIDGE_API_TOKEN` to require a bearer token:

```yaml
environment:
  - CARTRIDGE_API_TOKEN=your-secret
```

All requests (except `/api/health`, `/api/docs`, `/api/openapi.json`, `/api/logs`) then require:

```
Authorization: Bearer your-secret
```

WebSocket connections pass the token as a query param: `ws://host:4500/api/agents/:id/ws?token=your-secret`

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/health` | Liveness probe (`{"ok":true}`) |
| GET | `/api/status` | Full container status (services, CDP, providers, runtime, agents) |
| GET | `/api/logs` | API server log entries (`?limit=100&level=info`) |
| GET | `/api/openapi.json` | OpenAPI 3.1 spec |
| GET | `/api/docs` | Scalar interactive docs |
| POST | `/api/run` | Run a one-shot command |
| POST | `/api/agents` | Start an agent |
| GET | `/api/agents` | List agents (`?status=running&provider=claude`) |
| GET | `/api/agents/:id` | Agent details |
| GET | `/api/agents/:id/output` | Ring buffer contents (`?since=0&format=raw`) |
| POST | `/api/agents/:id/input` | Send keystrokes to agent PTY |
| POST | `/api/agents/:id/resize` | Resize agent PTY |
| POST | `/api/agents/:id/stop` | Stop an agent |
| POST | `/api/agents/:id/messages` | Send message to agent (inter-agent communication) |
| GET | `/api/agents/:id/messages` | List agent's message history |
| GET | `/api/agents/:id/ws` | WebSocket (terminal + events + status) |

## Starting agents

```bash
curl -X POST http://localhost:4500/api/agents \
  -H "Content-Type: application/json" \
  -d '{
    "provider": "claude",
    "prompt": "fix the failing test in src/auth.ts",
    "options": {
      "model": "opus",
      "maxTurns": 50,
      "allowedTools": ["Read", "Edit", "Bash"]
    },
    "env": {
      "ANTHROPIC_API_KEY": "sk-..."
    },
    "timeout": 3600
  }'
```

**Providers:** `claude`, `pi`, `opencode`, `codex`, `gemini`, `custom`

For `custom`, pass a `command` array instead of relying on provider mapping:

```json
{"provider": "custom", "command": ["node", "my-agent.js", "--flag"]}
```

### Provider options

Each provider maps `options` to CLI flags:

**Claude Code:** `model`, `maxTurns`, `permissionMode`, `allowedTools`, `appendSystemPrompt`, `mcpServers`, `resume`

**Pi:** `model`, `approve`, `mode`, `tools`, `excludeTools`, `noSession`, `noSkills`, `noExtensions`

**OpenCode:** `autoApprove`, `quiet`

**Codex:** `model`, `fullAuto`

**Gemini:** `model`, `autoApprove`

### Permissions

By default, all agents run with auto-approve (the container is the sandbox). Override per-agent or globally:

| Scope | How | Effect |
|-------|-----|--------|
| Container | `CARTRIDGE_API_SAFE_MODE=true` | All agents require confirmation |
| Per-agent | `"options": {"permissionMode": "plan"}` (Claude) or `"options": {"approve": false}` (Pi) | This agent requires confirmation |

When an agent pauses at a prompt, send input via `POST /api/agents/:id/input` or the WebSocket `input` frame.

## Inter-agent messaging

Agents can communicate with each other through the API. Messages are stored in the recipient's inbox and injected into their PTY as formatted input.

```bash
# Agent B sends a question to Agent A
curl -X POST http://localhost:4500/api/agents/$AGENT_A/messages \
  -H "Content-Type: application/json" \
  -d '{"from":"'$AGENT_B'","content":"can you check the auth module?"}'

# Agent A sees in their terminal:
# [message from ag_xyz: can you check the auth module?]

# Agent A responds
curl -X POST http://localhost:4500/api/agents/$AGENT_B/messages \
  -H "Content-Type: application/json" \
  -d '{"from":"'$AGENT_A'","content":"auth module looks fine","msg_type":"response"}'

# View message history
curl http://localhost:4500/api/agents/$AGENT_A/messages
```

Message types: `request` (default), `response`, `info`. The `from` field can be an agent ID or any identifier (e.g., `barracks` for orchestrator messages).

Since agents have `CARTRIDGE_API_URL` in their environment, they can send messages to other agents directly through the API without orchestrator involvement.

## WebSocket

Connect to `ws://host:4500/api/agents/:id/ws` for real-time streaming. The connection is multiplexed with three frame types:

**Server to client:**

```json
{"type": "terminal", "data": "<base64 PTY bytes>"}
{"type": "event", "event": "tool:post", "payload": {"tool": "Edit", "file": "src/auth.ts"}}
{"type": "status", "status": "completed", "exit_code": 0, "duration_ms": 42000}
```

**Client to server:**

```json
{"type": "input", "data": "y\n"}
{"type": "resize", "cols": 120, "rows": 40}
```

On connect, the server replays the current ring buffer contents, then switches to live streaming.

## Polling

For clients that don't need WebSocket:

```bash
# Start agent
ID=$(curl -s -X POST .../api/agents -d '...' | jq -r .id)

# Poll status
curl -s http://localhost:4500/api/agents/$ID | jq .status

# Incremental output
curl -s "http://localhost:4500/api/agents/$ID/output?since=0"
# Use next_offset from response for subsequent calls
curl -s "http://localhost:4500/api/agents/$ID/output?since=4096"
```

## Hooks

CLI hooks are pre-installed as plugins in the container image. They send structured events (tool calls, results, errors) to the API server via a Unix socket. Events are delivered over the WebSocket `event` frames and counted in the agent detail response.

| Provider | Hook mechanism | Installed at |
|----------|---------------|--------------|
| Claude Code | Plugin with `hooks.json` | `~/.claude/plugins/cartridge-api` (symlink) |
| Pi | TypeScript extension | `~/.pi/agent/extensions/cartridge-hook.ts` (symlink) |
| OpenCode | JS plugin | `~/.config/opencode/plugins/cartridge-hook.js` (symlink) |
| Codex, Gemini | Tier 1 only (lifecycle events) | N/A |

Disable hooks globally with `CARTRIDGE_HOOKS=false` or per-agent with `"hooks": false` in the request body.

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `CARTRIDGE_API_PORT` | `4500` | Listen port |
| `CARTRIDGE_API_TOKEN` | (none) | Bearer token. No auth when unset. |
| `CARTRIDGE_API_SAFE_MODE` | `false` | Disable auto-approve for all providers |
| `CARTRIDGE_API_BUFFER_SIZE` | `2097152` | Ring buffer per agent (bytes) |
| `CARTRIDGE_API_MAX_AGENTS` | `100` | Max agents in registry |
| `CARTRIDGE_API_MAX_CONCURRENT` | `10` | Max simultaneously running agents |
| `CARTRIDGE_API_MAX_TIMEOUT` | `7200` | Max agent timeout (seconds) |
| `CARTRIDGE_API_RETAIN_SECONDS` | `3600` | Keep completed agents for (seconds) |
| `CARTRIDGE_API_EXEC_TIMEOUT` | `30` | Default `/api/run` timeout |
| `CARTRIDGE_API_EXEC_MAX_TIMEOUT` | `300` | Max `/api/run` timeout |
| `CARTRIDGE_API_CORS_ORIGIN` | (none) | CORS origin. Disabled by default. |
| `CARTRIDGE_API_MAX_EVENTS` | `10000` | Max hook events per agent |
| `CARTRIDGE_API_LOG_LEVEL` | `info` | Log level (error, warn, info, debug) |
| `CARTRIDGE_HOOKS` | `true` | Global hook enable/disable |

## Paths

| Path | Purpose |
|------|---------|
| `/tmp/cartridge-api.sock` | Unix socket for hook events |
| `/etc/cartridge/plugins/` | Read-only plugin sources (image layer) |
| `~/.claude/plugins/cartridge-api` | Claude Code hook plugin (symlink) |
| `~/.claude/settings.local.json` | bypassPermissions default (copied on first boot) |
| `~/.pi/agent/extensions/cartridge-hook.ts` | Pi extension (symlink) |
| `~/.config/opencode/plugins/cartridge-hook.js` | OpenCode plugin (symlink) |

Plugin symlinks point to `/etc/cartridge/plugins/` inside the image. They survive volume mounts without polluting the mounted volume.

## Error responses

All errors return a consistent format:

```json
{
  "error": {
    "code": "agent_not_found",
    "message": "Agent ag_abc123 does not exist",
    "status": 404
  }
}
```

| HTTP | Code | When |
|------|------|------|
| 400 | `invalid_request` | Bad JSON, missing fields, invalid provider |
| 400 | `invalid_command` | Empty command array |
| 401 | `unauthorized` | Missing/invalid token |
| 404 | `agent_not_found` | Unknown agent ID |
| 409 | `agent_not_running` | Stop/input on exited agent |
| 429 | `too_many_agents` | Concurrent limit reached |
| 502 | `spawn_failed` | CLI binary not found or crashed |

## Architecture

The API is a single Rust binary (`cartridge-api`) with four modes:

```
cartridge-api serve    # HTTP/WS server (s6 service)
cartridge-api hook     # Forward hook event via Unix socket
cartridge-api status   # Print container status as JSON
cartridge-api health   # Exit 0 if server running, 1 otherwise
```

Agents are spawned in real PTYs via `portable-pty`. The full CLI TUI renders normally. Output is captured to a per-agent ring buffer and broadcast to WebSocket subscribers. Hook events arrive via a Unix socket from baked-in CLI plugins.

The API is stateless across restarts. The caller owns persistence.
