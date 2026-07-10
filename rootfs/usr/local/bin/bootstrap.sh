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

# ── Load config file (env vars take precedence) ─────────────────
eval "$(cartridge-config)" || true

# ── State directory ──────────────────────────────────────────────
mkdir -p "$HOME_DIR/.claude/state"

# ── Workspace git setup ──────────────────────────────────────────
WORKSPACE="/workspace"
if [ -d "$WORKSPACE" ]; then
  ws_empty() { [ ! -d "$WORKSPACE/.git" ] && [ -z "$(ls -A "$WORKSPACE" 2>/dev/null)" ]; }

  if [ -n "${GIT_WORKTREE_REPO:-}" ] && ws_empty; then
    BRANCH="${GIT_WORKTREE_BRANCH:-main}"
    if git clone --branch "$BRANCH" "$GIT_WORKTREE_REPO" "$WORKSPACE" 2>&1; then
      chown -R dev:dev "$WORKSPACE"
      log "workspace: cloned $GIT_WORKTREE_REPO ($BRANCH) for worktree"
    else
      log "workspace: failed to clone $GIT_WORKTREE_REPO"
    fi

  elif [ -n "${GIT_REPO_URL:-}" ] && ws_empty; then
    BRANCH="${GIT_BRANCH:-}"
    if git clone ${BRANCH:+--branch "$BRANCH"} "$GIT_REPO_URL" "$WORKSPACE" 2>&1; then
      chown -R dev:dev "$WORKSPACE"
      log "workspace: cloned $GIT_REPO_URL${BRANCH:+ ($BRANCH)}"
    else
      log "workspace: failed to clone $GIT_REPO_URL"
    fi

  elif ws_empty; then
    git init -q "$WORKSPACE"
    log "workspace: initialized empty git repo"

  elif [ -d "$WORKSPACE/.git" ]; then
    log "workspace: existing git repo"
  fi

  # safe.directory so dev user can use git regardless of ownership
  su -s /bin/sh dev -c "git config --global --add safe.directory $WORKSPACE" 2>/dev/null || true
fi

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
      discovered=$(curl -sf --connect-timeout 5 --max-time 10 "${OLLAMA_HOST}/api/tags" 2>/dev/null \
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

# ── SSH authorized keys ──────────────────────────────────────────
if [ -n "${SSH_AUTHORIZED_KEYS:-}" ]; then
  SSH_DIR="$HOME_DIR/.ssh"
  mkdir -p "$SSH_DIR"
  chmod 700 "$SSH_DIR"
  echo "$SSH_AUTHORIZED_KEYS" > "$SSH_DIR/authorized_keys"
  chmod 600 "$SSH_DIR/authorized_keys"
  chown -R dev:dev "$SSH_DIR"
  log "ssh: authorized keys installed"
fi

# ── Tunneling ────────────────────────────────────────────────────
if [ -n "${TAILSCALE_AUTHKEY:-}" ]; then
  log "tailscale: authkey set, will connect on service start"
fi
if [ -n "${CF_TUNNEL_TOKEN:-}" ]; then
  log "cloudflared: tunnel token set, will connect on service start"
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

# ── API hook plugin installation ─────────────────────────────────
PLUGIN_SRC="/etc/cartridge/plugins"

if [ -d "$PLUGIN_SRC" ]; then
  # Claude Code
  CLAUDE_PLUGIN_DIR="$HOME_DIR/.claude/plugins"
  mkdir -p "$CLAUDE_PLUGIN_DIR"
  if [ ! -e "$CLAUDE_PLUGIN_DIR/cartridge-api" ]; then
    ln -sf "$PLUGIN_SRC/claude/cartridge-api" "$CLAUDE_PLUGIN_DIR/cartridge-api"
    log "api: linked Claude Code hook plugin"
  fi

  # Pi
  PI_EXT_DIR="$HOME_DIR/.pi/agent/extensions"
  mkdir -p "$PI_EXT_DIR"
  if [ ! -e "$PI_EXT_DIR/cartridge-hook.ts" ]; then
    ln -sf "$PLUGIN_SRC/pi/cartridge-hook.ts" "$PI_EXT_DIR/cartridge-hook.ts"
    log "api: linked Pi extension"
  fi

  # OpenCode
  OC_PLUGIN_DIR="$HOME_DIR/.config/opencode/plugins"
  mkdir -p "$OC_PLUGIN_DIR"
  if [ ! -e "$OC_PLUGIN_DIR/cartridge-hook.js" ]; then
    ln -sf "$PLUGIN_SRC/opencode/cartridge-hook.js" "$OC_PLUGIN_DIR/cartridge-hook.js"
    log "api: linked OpenCode plugin"
  fi

  # Safe mode
  if [ "${CARTRIDGE_API_SAFE_MODE:-}" = "true" ]; then
    rm -f "$HOME_DIR/.claude/settings.local.json"
    log "api: safe mode enabled, removed bypassPermissions override"
  else
    if [ ! -f "$HOME_DIR/.claude/settings.local.json" ]; then
      cp "$PLUGIN_SRC/claude/settings.local.json" "$HOME_DIR/.claude/settings.local.json"
      log "api: set bypassPermissions default"
    fi
  fi

  # Hook opt-out
  if [ "${CARTRIDGE_HOOKS:-true}" = "false" ]; then
    rm -f "$CLAUDE_PLUGIN_DIR/cartridge-api"
    rm -f "$PI_EXT_DIR/cartridge-hook.ts"
    rm -f "$OC_PLUGIN_DIR/cartridge-hook.js"
    log "api: hooks disabled globally (removed symlinks)"
  fi
fi

# Fix ownership
chown -R dev:dev "$HOME_DIR"

log "ready"
