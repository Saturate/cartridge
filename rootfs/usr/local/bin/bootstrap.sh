#!/bin/bash
set -euo pipefail

HOME_DIR="/home/dev"
NVM_DIR="/usr/local/nvm"

log() { echo "[bootstrap] $*"; }

# ── Default shell config ─────────────────────────────────────────
if [ ! -f "$HOME_DIR/.zshrc" ]; then
  cat > "$HOME_DIR/.zshrc" << 'EOF'
export LANG=en_US.UTF-8
export EDITOR=vim
export NVM_DIR=/usr/local/nvm
export PATH="$HOME/.local/bin:$PATH"
[ -s "$NVM_DIR/nvm.sh" ] && . "$NVM_DIR/nvm.sh"

PROMPT='%F{blue}cartridge%f %~ %# '

alias ll='ls -la'
alias gs='git status'
alias gd='git diff'
alias gl='git log --oneline -20'
alias status='cartridge-status'

HISTFILE="$HOME/.zsh_history"
HISTSIZE=10000
SAVEHIST=10000
setopt SHARE_HISTORY HIST_IGNORE_DUPS HIST_IGNORE_SPACE
EOF
fi

# ── Default tmux config ──────────────────────────────────────────
if [ ! -f "$HOME_DIR/.tmux.conf" ]; then
  cat > "$HOME_DIR/.tmux.conf" << 'EOF'
set -g default-terminal "tmux-256color"
set -g history-limit 50000
set -g mouse on
set -g base-index 1
setw -g pane-base-index 1
set -g status-style 'bg=colour235 fg=colour250'
set -g status-left '#[fg=colour39]cartridge '
set -g status-right '%H:%M'
EOF
fi

# ── Default git config ───────────────────────────────────────────
if [ ! -f "$HOME_DIR/.gitconfig" ]; then
  cat > "$HOME_DIR/.gitconfig" << 'EOF'
[init]
  defaultBranch = main
[pull]
  rebase = true
[core]
  autocrlf = input
  pager = delta
[interactive]
  diffFilter = delta --color-only
[delta]
  navigate = true
  side-by-side = true
EOF
fi

# ── State directory ──────────────────────────────────────────────
mkdir -p "$HOME_DIR/.claude/state"

# ── Provider auto-wiring ─────────────────────────────────────────
# Claude Code: ANTHROPIC_API_KEY is picked up automatically.
# For subscription auth, the user runs `claude auth login` or
# `claude setup-token` interactively, or mounts credentials at
# ~/.claude/ via a volume or K8s secret.

# GitHub CLI
if [ -n "${GH_TOKEN:-}${GITHUB_TOKEN:-}" ]; then
  log "gh: token detected"
fi

# Pi + Ollama: write models.json if OLLAMA_HOST is set
if [ -n "${OLLAMA_HOST:-}" ]; then
  PI_DIR="$HOME_DIR/.pi/agent"
  mkdir -p "$PI_DIR"
  if [ ! -f "$PI_DIR/models.json" ]; then
    OLLAMA_MODELS="${OLLAMA_MODELS:-}"
    MODELS_JSON="["
    if [ -n "$OLLAMA_MODELS" ]; then
      IFS=',' read -ra MODEL_LIST <<< "$OLLAMA_MODELS"
      first=true
      for model in "${MODEL_LIST[@]}"; do
        model=$(echo "$model" | xargs)
        [ -z "$model" ] && continue
        $first || MODELS_JSON+=","
        MODELS_JSON+="{\"id\":\"$model\"}"
        first=false
      done
    else
      # Try to discover models from the Ollama server
      discovered=$(curl -sf "${OLLAMA_HOST}/api/tags" 2>/dev/null \
        | python3 -c "
import sys,json
try:
    models = json.load(sys.stdin).get('models',[])
    print(','.join(m['name'] for m in models if 'embed' not in m['name']))
except: pass
" 2>/dev/null || true)
      if [ -n "$discovered" ]; then
        IFS=',' read -ra MODEL_LIST <<< "$discovered"
        first=true
        for model in "${MODEL_LIST[@]}"; do
          $first || MODELS_JSON+=","
          MODELS_JSON+="{\"id\":\"$model\"}"
          first=false
        done
        log "ollama: discovered models: $discovered"
      fi
    fi
    MODELS_JSON+="]"

    cat > "$PI_DIR/models.json" << PIEOF
{
  "providers": {
    "ollama": {
      "baseUrl": "${OLLAMA_HOST}/v1",
      "api": "openai-completions",
      "apiKey": "ollama",
      "compat": {
        "supportsDeveloperRole": false,
        "supportsReasoningEffort": false
      },
      "models": $MODELS_JSON
    }
  }
}
PIEOF
    log "pi: configured ollama at $OLLAMA_HOST"
  fi
fi

# Notification URL for shoutrrr
if [ -n "${SHOUTRRR_URL:-}" ]; then
  log "shoutrrr: notification URL configured"
fi

# ── HUSK OTel wiring ─────────────────────────────────────────────
if [ -n "${HUSK_ENDPOINT:-}" ]; then
  mkdir -p "$HOME_DIR/.claude"
  CLAUDE_SETTINGS="$HOME_DIR/.claude/settings.json"
  if [ ! -f "$CLAUDE_SETTINGS" ]; then
    echo '{}' > "$CLAUDE_SETTINGS"
  fi
  log "husk: endpoint $HUSK_ENDPOINT"
fi

# ── Skills init ──────────────────────────────────────────────────
if [ "${SKILLS_INIT:-}" = "true" ] && [ -d /skills ]; then
  SKILLS_TARGET="$HOME_DIR/.claude/skills"
  mkdir -p "$SKILLS_TARGET"
  count=0
  for skill_dir in /skills/*/; do
    [ -d "$skill_dir" ] || continue
    skill_name=$(basename "$skill_dir")
    if [ ! -e "$SKILLS_TARGET/$skill_name" ]; then
      ln -sf "$skill_dir" "$SKILLS_TARGET/$skill_name"
      count=$((count + 1))
    fi
  done
  [ $count -gt 0 ] && log "skills: linked $count from /skills"
fi

# ── PromptKiddie integration ─────────────────────────────────────
if [ "${PK_MODE:-}" = "true" ]; then
  if command -v pk &>/dev/null; then
    log "promptkiddie: active"
  fi
fi

# Fix ownership
chown -R dev:dev "$HOME_DIR"

log "ready"
