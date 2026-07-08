#!/bin/bash
set -euo pipefail

HOME_DIR="/home/dev"

# ── Default shell config ─────────────────────────────────────────
if [ ! -f "$HOME_DIR/.zshrc" ]; then
  cat > "$HOME_DIR/.zshrc" << 'EOF'
export LANG=en_US.UTF-8
export EDITOR=vim
export PATH="$HOME/.local/bin:$PATH"

PROMPT='%F{blue}cartridge%f %~ %# '

alias ll='ls -la'
alias gs='git status'
alias gd='git diff'
alias gl='git log --oneline -20'

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
EOF
fi

# ── State directory ──────────────────────────────────────────────
mkdir -p "$HOME_DIR/.claude/state"

# ── HUSK OTel wiring ─────────────────────────────────────────────
if [ -n "${HUSK_ENDPOINT:-}" ]; then
  mkdir -p "$HOME_DIR/.claude"
  CLAUDE_SETTINGS="$HOME_DIR/.claude/settings.json"
  if [ ! -f "$CLAUDE_SETTINGS" ]; then
    echo '{}' > "$CLAUDE_SETTINGS"
  fi
fi

# ── Skills init ──────────────────────────────────────────────────
if [ "${SKILLS_INIT:-}" = "true" ] && [ -d /skills ]; then
  SKILLS_TARGET="$HOME_DIR/.claude/skills"
  mkdir -p "$SKILLS_TARGET"
  for skill_dir in /skills/*/; do
    [ -d "$skill_dir" ] || continue
    skill_name=$(basename "$skill_dir")
    if [ ! -e "$SKILLS_TARGET/$skill_name" ]; then
      ln -sf "$skill_dir" "$SKILLS_TARGET/$skill_name"
    fi
  done
fi

# ── PromptKiddie integration ─────────────────────────────────────
if [ "${PK_MODE:-}" = "true" ]; then
  if command -v pk &>/dev/null; then
    echo "[bootstrap] PromptKiddie mode active"
  fi
fi

# Fix ownership
chown -R dev:dev "$HOME_DIR"

echo "[bootstrap] Complete"
