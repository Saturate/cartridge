# Google Gemini + Cartridge

Run Google's Gemini CLI or use Pi with Gemini models.

## Gemini CLI

```yaml
services:
  cartridge:
    build: .
    environment:
      - GEMINI_API_KEY=${GEMINI_API_KEY}
    ports:
      - "127.0.0.1:7681:7681"
    volumes:
      - ./workspace:/workspace
```

```bash
docker compose up -d
# Open http://localhost:7681

# Interactive mode
gemini

# Gemini CLI also supports Google OAuth:
gemini
# Use /login inside the CLI
```

## Pi with Google models

```bash
# Inside the container:
pi --model google/gemini-2.5-pro "analyze this codebase"
pi --model google/gemini-2.5-flash --print "quick summary"
```

## TOML config

```toml
# cartridge.toml
[providers]
gemini_api_key = "AI..."
```

## With Ollama (Gemma models)

Gemma models run locally via Ollama, no API key needed:

```yaml
services:
  cartridge:
    build: .
    environment:
      - OLLAMA_HOST=http://host.docker.internal:11434
    ports:
      - "127.0.0.1:7681:7681"
    volumes:
      - ./workspace:/workspace
```

```bash
# Pull Gemma 4 on your host:
ollama pull gemma4:12b

# Inside the container, Pi auto-discovers it:
pi --model ollama/gemma4:12b "refactor this"
```

See [ollama.md](ollama.md) for more Ollama setups.
