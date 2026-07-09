# Ollama + Cartridge

Run any Ollama model inside Cartridge via Pi. No API keys needed.

## Local Ollama (same machine)

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
# Make sure Ollama is running on your host
ollama serve

docker compose up -d
# Open http://localhost:7681
# Run: pi --model ollama/llama3.2 "write a hello world"
```

## Remote Ollama server

```yaml
services:
  cartridge:
    build: .
    environment:
      - OLLAMA_HOST=http://10.106.20.134:11434
    ports:
      - "127.0.0.1:7681:7681"
    volumes:
      - ./workspace:/workspace
```

Bootstrap auto-discovers all models on the server and writes Pi's config.

## TOML config

```toml
# cartridge.toml
[providers]
ollama_host = "http://10.106.20.134:11434"

# Optional: limit to specific models (otherwise all are discovered)
ollama_models = ["gemma4:31b-it-bf16", "qwen2.5:14b", "llama3.2"]
```

```bash
docker run --rm -d \
  -v ./cartridge.toml:/etc/cartridge/config.toml:ro \
  -v ./workspace:/workspace \
  -p 127.0.0.1:7681:7681 \
  cartridge
```

## Specific models

```bash
# Inside the container:

# Use Gemma 4
pi --model ollama/gemma4:31b-it-bf16 "refactor this function"

# Use Llama
pi --model ollama/llama3.2 "explain this code"

# Use Qwen
pi --model ollama/qwen2.5:14b "write tests for this module"

# Non-interactive
pi --model ollama/gemma4:31b-it-bf16 --print "what does this project do"
```

## Verify the connection

```bash
# Inside the container:
cartridge-status
# Should show:
#   ollama: ok (http://10.106.20.134:11434, 10 models)
#   pi:     ok (ollama)

# List available models:
pi --list-models ollama
```

## Ollama in the same compose stack

```yaml
services:
  cartridge:
    build: .
    environment:
      - OLLAMA_HOST=http://ollama:11434
    ports:
      - "127.0.0.1:7681:7681"
    volumes:
      - ./workspace:/workspace
    depends_on: [ollama]

  ollama:
    image: ollama/ollama
    volumes:
      - ollama-data:/root/.ollama
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: all
              capabilities: [gpu]

volumes:
  ollama-data:
```

```bash
docker compose up -d
# Pull a model:
docker exec ollama ollama pull gemma4:12b
# Open http://localhost:7681, start coding with Pi
```
