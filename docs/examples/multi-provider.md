# Multi-Provider Setup

Run all four harnesses and switch between providers per task.

## Full stack compose

```yaml
services:
  cartridge:
    build: .
    shm_size: 2g
    cap_add: [SYS_ADMIN, SYS_PTRACE]
    security_opt: [seccomp=unconfined]
    ports:
      - "127.0.0.1:7681:7681"
    environment:
      - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
      - OPENAI_API_KEY=${OPENAI_API_KEY}
      - GEMINI_API_KEY=${GEMINI_API_KEY}
      - OLLAMA_HOST=http://host.docker.internal:11434
      - GH_TOKEN=${GH_TOKEN}
      - SHOUTRRR_URL=${SHOUTRRR_URL}
    volumes:
      - ./workspace:/workspace
      - ./data/config:/home/dev/.claude
```

## Full stack TOML

```toml
# cartridge.toml
[providers]
anthropic_api_key = "sk-ant-..."
openai_api_key = "sk-..."
gemini_api_key = "AI..."
ollama_host = "http://10.106.20.134:11434"

[github]
token = "ghp_..."

[notifications]
shoutrrr_url = "slack://token-a/token-b/token-c"
```

## Choosing the right tool per task

```bash
# Complex refactoring, long-horizon work
claude

# Quick tasks with local models, no API cost
pi --model ollama/gemma4:31b-it-bf16 "fix this bug"

# OpenAI reasoning models
pi --model openai/o3 "design the architecture for this system"

# Fast iteration with Gemini
gemini

# Codex for OpenAI-native workflows
codex exec "add tests for the auth module"
```

## Per-task provider selection with Pi

Pi is the universal harness; it talks to every provider:

```bash
# Same tool, different backends:
pi --model anthropic/claude-sonnet-4-6 "review this PR"
pi --model openai/gpt-4o "suggest improvements"
pi --model google/gemini-2.5-pro "analyze performance"
pi --model ollama/qwen2.5:14b "write the docs"
```

## Verify everything is connected

```bash
cartridge-status

# Expected output:
# providers:
#   claude: ok (API key)
#   pi:     ok (ollama)
#   ollama: ok (http://10.106.20.134:11434, 10 models)
#   github: ok (token)
#
# notifications:
#   shoutrrr: ok (slack://...)
```

## Notify when done

```bash
# Run a task and notify on completion:
claude -p "refactor the auth module" && \
  shoutrrr send -u "$SHOUTRRR_URL" "Auth refactor complete"
```
