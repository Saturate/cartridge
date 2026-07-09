# OpenAI + Cartridge

Run OpenAI's Codex CLI or use Pi with OpenAI models.

## Codex CLI

```yaml
services:
  cartridge:
    build: .
    environment:
      - OPENAI_API_KEY=${OPENAI_API_KEY}
    ports:
      - "127.0.0.1:7681:7681"
    volumes:
      - ./workspace:/workspace
```

```bash
docker compose up -d
# Open http://localhost:7681

# Interactive mode
codex

# Non-interactive
codex exec "add error handling to this function"
```

## Pi with OpenAI

Pi supports OpenAI models without Codex CLI:

```bash
# Inside the container:
pi --model openai/gpt-4o "review this code"
pi --model openai/o3 "solve this complex problem"
pi --model openai/gpt-4o-mini --print "quick summary of this file"
```

## TOML config

```toml
# cartridge.toml
[providers]
openai_api_key = "sk-..."
```

## Codex with ChatGPT subscription

```bash
# Inside the container:
codex
# Use /login to authenticate with ChatGPT Plus/Pro
```

## Both providers at once

```yaml
services:
  cartridge:
    build: .
    environment:
      - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
      - OPENAI_API_KEY=${OPENAI_API_KEY}
    ports:
      - "127.0.0.1:7681:7681"
    volumes:
      - ./workspace:/workspace
```

```bash
# Switch between providers inside the container:
claude "implement the feature"
codex exec "write tests for it"
pi --model openai/gpt-4o "review the changes"
```
