#!/bin/bash
set -euo pipefail

PUID=${PUID:-1000}
PGID=${PGID:-1000}

# Remap dev user UID/GID if different from defaults
if [ "$PGID" != "1000" ]; then
  groupmod -g "$PGID" dev 2>/dev/null || true
fi
if [ "$PUID" != "1000" ]; then
  usermod -u "$PUID" -o dev 2>/dev/null || true
fi

# Fix ownership
chown -R dev:dev /home/dev
chown dev:dev /workspace 2>/dev/null || true

# State restore from persistent volume
STATE_DIR="/home/dev/.claude/state"
if [ -d "$STATE_DIR" ]; then
  for f in .zsh_history .gitconfig .tmux.conf; do
    [ -f "$STATE_DIR/$f" ] && cp "$STATE_DIR/$f" "/home/dev/$f"
  done
  chown dev:dev /home/dev/.zsh_history /home/dev/.gitconfig /home/dev/.tmux.conf 2>/dev/null || true
fi

# Bootstrap runs every boot (idempotent, only writes missing configs)
/usr/local/bin/bootstrap.sh

# Hand off to s6-overlay
exec /init
