#!/bin/bash
# Cartridge integration tests
# Runs the container with different configs and asserts behavior.
# Usage: ./test/integration.sh [test_name]

set -euo pipefail

IMAGE="cartridge-cartridge"
PASS=0
FAIL=0
ERRORS=""

# ── Helpers ──────────────────────────────────────────────────────

run_container() {
  local name="$1"; shift
  docker rm -f "$name" >/dev/null 2>&1 || true
  sleep 1
  local cid
  cid=$(docker run --rm -d --name "$name" \
    --shm-size 2g --cap-add SYS_ADMIN --security-opt seccomp=unconfined \
    "$@" "$IMAGE" 2>&1)
  if [ -z "$cid" ]; then
    echo "  FAILED to start container $name"
    return 1
  fi
  echo "$cid"
}

wait_ready() {
  local name="$1"
  local timeout="${2:-25}"
  local tmplog
  tmplog=$(mktemp)
  for _ in $(seq 1 "$timeout"); do
    docker logs "$name" > "$tmplog" 2>&1
    if grep -qF "[bootstrap] ready" "$tmplog"; then
      rm -f "$tmplog"
      sleep 2
      return 0
    fi
    sleep 1
  done
  echo "  TIMEOUT waiting for bootstrap ready ($timeout s)"
  grep bootstrap "$tmplog" || tail -3 "$tmplog"
  rm -f "$tmplog"
  cleanup "$name"
  return 1
}

cleanup() {
  local name="$1"
  docker stop "$name" >/dev/null 2>&1 || true
}

assert() {
  local desc="$1" result="$2"
  if [ -n "$result" ]; then
    echo "  ✓ $desc"
    PASS=$((PASS + 1))
  else
    echo "  ✗ $desc"
    FAIL=$((FAIL + 1))
    ERRORS="${ERRORS}\n  - $desc"
  fi
}

assert_eq() {
  local desc="$1" expected="$2" actual="$3"
  if [ "$expected" = "$actual" ]; then
    echo "  ✓ $desc"
    PASS=$((PASS + 1))
  else
    echo "  ✗ $desc (expected: $expected, got: $actual)"
    FAIL=$((FAIL + 1))
    ERRORS="${ERRORS}\n  - $desc (expected: $expected, got: $actual)"
  fi
}

assert_contains() {
  local desc="$1" haystack="$2" needle="$3"
  if echo "$haystack" | grep -q "$needle"; then
    echo "  ✓ $desc"
    PASS=$((PASS + 1))
  else
    echo "  ✗ $desc (missing: $needle)"
    FAIL=$((FAIL + 1))
    ERRORS="${ERRORS}\n  - $desc (missing: $needle)"
  fi
}

exec_dev() {
  local name="$1"; shift
  docker exec -u dev "$name" sh -c "export NVM_DIR=/usr/local/nvm; . \"\$NVM_DIR/nvm.sh\" 2>/dev/null; $*"
}

exec_root() {
  local name="$1"; shift
  docker exec "$name" sh -c "$*"
}

# ── Tests ────────────────────────────────────────────────────────

test_services() {
  echo "test: services"
  local c="cart-test-svc"
  run_container "$c"
  wait_ready "$c"

  # s6 services running
  assert "xvfb running" "$(exec_root "$c" 'pgrep -f Xvfb')"
  assert "ttyd running" "$(exec_root "$c" 'pgrep -f ttyd')"
  assert "state-sync running" "$(exec_root "$c" 'pgrep -f svc-state-sync')"

  # chrome CDP responding
  local cdp
  cdp=$(exec_root "$c" 'curl -sf http://localhost:9222/json/version 2>/dev/null | python3 -c "import sys,json;print(json.load(sys.stdin).get(\"Browser\",\"\"))" 2>/dev/null')
  assert "chrome CDP responds" "$cdp"

  # ttyd HTTP
  local ttyd_status
  ttyd_status=$(exec_root "$c" 'curl -sf -o /dev/null -w "%{http_code}" http://localhost:7681')
  assert_eq "ttyd returns 200" "200" "$ttyd_status"

  # optional services sleeping (disabled by default)
  assert "novnc disabled" "$(exec_root "$c" 'pgrep -f "sleep infinity" | head -1')"

  cleanup "$c"
}

test_tools() {
  echo "test: tools"
  local c="cart-test-tools"
  run_container "$c"
  wait_ready "$c"

  # AI harnesses
  local claude_ver pi_ver
  claude_ver=$(exec_dev "$c" 'claude --version 2>&1 | head -1')
  assert_contains "claude installed" "$claude_ver" "Claude Code"

  pi_ver=$(exec_dev "$c" 'pi --version 2>&1')
  assert "pi installed" "$pi_ver"

  # Node/nvm
  local node_ver
  node_ver=$(exec_dev "$c" 'node --version')
  assert_contains "node 24 default" "$node_ver" "v24"

  local node22
  node22=$(exec_dev "$c" 'nvm run 22 --version 2>&1 | tail -1')
  assert_contains "node 22 available" "$node22" "v22"

  # Package managers
  assert "pnpm installed" "$(exec_dev "$c" 'pnpm --version 2>/dev/null')"
  assert "bun installed" "$(exec_dev "$c" 'bun --version 2>/dev/null')"

  # Core CLI tools
  for tool in rg fd bat fzf jq git gh delta scc tmux zsh python3 curl; do
    assert "$tool on PATH" "$(exec_dev "$c" "which $tool 2>/dev/null")"
  done

  # Media/doc tools
  for tool in pandoc ffmpeg convert shoutrrr; do
    assert "$tool on PATH" "$(exec_dev "$c" "which $tool 2>/dev/null")"
  done

  # DB clients
  for tool in sqlite3 psql redis-cli; do
    assert "$tool on PATH" "$(exec_dev "$c" "which $tool 2>/dev/null")"
  done

  cleanup "$c"
}

test_workspace_empty() {
  echo "test: workspace (empty -> git init)"
  local c="cart-test-ws-empty"
  run_container "$c"
  wait_ready "$c"

  local logs
  logs=$(docker logs "$c" 2>&1)
  assert_contains "bootstrap inits git" "$logs" "initialized empty git repo"

  local is_repo
  is_repo=$(exec_dev "$c" 'git -C /workspace rev-parse --git-dir 2>&1')
  assert_eq "workspace is git repo" ".git" "$is_repo"

  # dev user can use git without permission errors
  local git_status
  git_status=$(exec_dev "$c" 'git -C /workspace status 2>&1 | head -1')
  assert_contains "dev user can git status" "$git_status" "branch"

  cleanup "$c"
}

test_workspace_clone() {
  echo "test: workspace (auto-clone)"
  local c="cart-test-ws-clone"
  run_container "$c" \
    -e GIT_REPO_URL=https://github.com/containrrr/shoutrrr \
    -e GIT_BRANCH=main
  wait_ready "$c" 45

  local logs
  logs=$(docker logs "$c" 2>&1)
  assert_contains "bootstrap cloned repo" "$logs" "cloned.*shoutrrr"

  local git_log
  git_log=$(exec_dev "$c" 'git -C /workspace log --oneline -1 2>&1')
  assert "clone has commits" "$git_log"

  local branch
  branch=$(exec_dev "$c" 'git -C /workspace branch --show-current 2>&1')
  assert_eq "on correct branch" "main" "$branch"

  cleanup "$c"
}

test_workspace_existing() {
  echo "test: workspace (existing repo, leave alone)"
  local c="cart-test-ws-exist"
  # Mount this repo as workspace (read-only)
  local repo_dir
  repo_dir=$(cd "$(dirname "$0")/.." && pwd)
  run_container "$c" -v "${repo_dir}:/workspace"
  wait_ready "$c"

  local logs
  logs=$(docker logs "$c" 2>&1)
  assert_contains "detects existing repo" "$logs" "existing git repo"

  cleanup "$c"
}

test_ollama_wiring() {
  echo "test: ollama auto-wiring"
  local c="cart-test-ollama"

  # Check if the test Ollama server is reachable
  if ! curl -sf http://10.106.20.134:11434/api/tags >/dev/null 2>&1; then
    echo "  SKIP (ollama server at 10.106.20.134 not reachable)"
    return
  fi

  run_container "$c" -e OLLAMA_HOST=http://10.106.20.134:11434
  wait_ready "$c"

  local logs
  logs=$(docker logs "$c" 2>&1)
  assert_contains "bootstrap discovers models" "$logs" "ollama: discovered models"
  assert_contains "bootstrap configures pi" "$logs" "pi: configured ollama"

  local models_json
  models_json=$(exec_dev "$c" 'cat ~/.pi/agent/models.json 2>/dev/null')
  assert_contains "models.json has baseUrl" "$models_json" "10.106.20.134"
  assert_contains "models.json has provider" "$models_json" "ollama"

  cleanup "$c"
}

test_bootstrap_configs() {
  echo "test: bootstrap config files"
  local c="cart-test-configs"
  run_container "$c"
  wait_ready "$c"

  assert "zshrc exists" "$(exec_dev "$c" 'test -f ~/.zshrc && echo yes')"
  assert "tmux.conf exists" "$(exec_dev "$c" 'test -f ~/.tmux.conf && echo yes')"
  assert "gitconfig exists" "$(exec_dev "$c" 'test -f ~/.gitconfig && echo yes')"

  # gitconfig has delta as pager
  local pager
  pager=$(exec_dev "$c" 'git config --get core.pager 2>/dev/null')
  assert_eq "git pager is delta" "delta" "$pager"

  # zshrc has nvm
  local zshrc
  zshrc=$(exec_dev "$c" 'cat ~/.zshrc')
  assert_contains "zshrc sources nvm" "$zshrc" "NVM_DIR"

  # dev user owns home
  local owner
  owner=$(exec_root "$c" 'stat -c "%U" /home/dev/.zshrc')
  assert_eq "dev owns config files" "dev" "$owner"

  cleanup "$c"
}

test_status_script() {
  echo "test: cartridge-status"
  local c="cart-test-status"
  run_container "$c"
  wait_ready "$c"

  local exit_code
  exec_dev "$c" 'cartridge-status >/dev/null 2>&1'
  exit_code=$?
  assert_eq "cartridge-status exits 0" "0" "$exit_code"

  local output
  output=$(exec_dev "$c" 'cartridge-status 2>&1')
  assert_contains "shows services section" "$output" "services:"
  assert_contains "shows providers section" "$output" "providers:"
  assert_contains "shows runtime section" "$output" "runtime:"
  assert_contains "shows xvfb running" "$output" "running"

  cleanup "$c"
}

test_user_permissions() {
  echo "test: dev user permissions"
  local c="cart-test-perms"
  run_container "$c"
  wait_ready "$c"

  local uid gid
  uid=$(exec_dev "$c" 'id -u')
  gid=$(exec_dev "$c" 'id -g')
  assert_eq "dev uid is 1000" "1000" "$uid"
  assert_eq "dev gid is 1000" "1000" "$gid"

  # Can write to workspace
  exec_dev "$c" 'touch /workspace/test-write && rm /workspace/test-write'
  assert_eq "dev can write to /workspace" "0" "$?"

  # Shell is zsh
  local shell
  shell=$(exec_root "$c" 'getent passwd dev | cut -d: -f7')
  assert_eq "dev shell is zsh" "/bin/zsh" "$shell"

  cleanup "$c"
}

test_uid_remap() {
  echo "test: UID/GID remap"
  local c="cart-test-remap"
  run_container "$c" -e PUID=1500 -e PGID=1500
  wait_ready "$c"

  local uid gid
  uid=$(exec_root "$c" 'id -u dev')
  gid=$(exec_root "$c" 'id -g dev')
  assert_eq "dev uid remapped to 1500" "1500" "$uid"
  assert_eq "dev gid remapped to 1500" "1500" "$gid"

  cleanup "$c"
}

test_optional_services() {
  echo "test: optional services (noVNC, SSH)"
  local c="cart-test-optional"
  run_container "$c" \
    -e NOVNC_ENABLE=true \
    -e SSH_ENABLE=true
  wait_ready "$c" 25

  # noVNC should be running (not sleeping)
  local novnc_sleep
  novnc_sleep=$(exec_root "$c" 'pgrep -f "svc-novnc" -o 2>/dev/null && pgrep -f "sleep infinity" -P $(pgrep -f "svc-novnc" -o) 2>/dev/null || echo ""')
  # If sleep infinity is NOT a child of novnc supervisor, it's active
  local novnc_port
  novnc_port=$(exec_root "$c" 'curl -sf -o /dev/null -w "%{http_code}" http://localhost:6080 2>/dev/null || echo "000"')
  assert "novnc responds" "$([ "$novnc_port" != "000" ] && echo yes)"

  # SSH should have host keys
  assert "ssh host key exists" "$(exec_root "$c" 'test -f /etc/ssh/ssh_host_ed25519_key && echo yes')"

  cleanup "$c"
}

# ── API Tests ───────────────────────────────────────────────────

test_api_health() {
  echo "test: api health"
  local c="cart-test-api"
  run_container "$c"
  wait_ready "$c"

  local health
  health=$(exec_root "$c" 'curl -sf http://localhost:4500/api/health')
  assert_contains "health returns ok" "$health" '"ok":true'

  cleanup "$c"
}

test_api_status() {
  echo "test: api status"
  local c="cart-test-api-status"
  run_container "$c"
  wait_ready "$c"

  sleep 2

  local status
  status=$(exec_root "$c" 'curl -sf http://localhost:4500/api/status')
  assert_contains "status has services" "$status" '"services"'
  assert_contains "status has runtime" "$status" '"runtime"'
  assert_contains "status has agents" "$status" '"agents"'

  cleanup "$c"
}

test_api_run() {
  echo "test: api run"
  local c="cart-test-api-run"
  run_container "$c"
  wait_ready "$c"

  sleep 2

  local result
  result=$(exec_root "$c" 'curl -sf -X POST http://localhost:4500/api/run \
    -H "Content-Type: application/json" \
    -d "{\"command\":[\"echo\",\"hello\"],\"timeout\":5}"')
  assert_contains "run returns stdout" "$result" "hello"
  assert_contains "run returns exit code 0" "$result" '"exit_code":0'

  cleanup "$c"
}

test_api_agent_lifecycle() {
  echo "test: api agent lifecycle"
  local c="cart-test-api-agent"
  run_container "$c"
  wait_ready "$c"

  sleep 2

  # Start a custom agent (simple script that runs for a few seconds)
  local start_resp
  start_resp=$(exec_root "$c" 'curl -sf -X POST http://localhost:4500/api/agents \
    -H "Content-Type: application/json" \
    -d "{\"provider\":\"custom\",\"prompt\":\"\",\"command\":[\"sh\",\"-c\",\"echo started; sleep 5; echo done\"],\"timeout\":10}"')
  local agent_id
  agent_id=$(echo "$start_resp" | python3 -c "import sys,json;print(json.load(sys.stdin)['id'])")
  assert "agent started" "$agent_id"

  # Check it's running
  sleep 1
  local status
  status=$(exec_root "$c" "curl -sf http://localhost:4500/api/agents/$agent_id")
  assert_contains "agent is running" "$status" '"running"'

  # Check output
  local output
  output=$(exec_root "$c" "curl -sf http://localhost:4500/api/agents/$agent_id/output")
  assert_contains "output has started" "$output" "started"

  # Wait for completion
  sleep 6
  status=$(exec_root "$c" "curl -sf http://localhost:4500/api/agents/$agent_id")
  assert_contains "agent completed" "$status" '"completed"'

  # List agents
  local list
  list=$(exec_root "$c" 'curl -sf http://localhost:4500/api/agents')
  assert_contains "list includes agent" "$list" "$agent_id"

  cleanup "$c"
}

test_api_agent_stop() {
  echo "test: api agent stop"
  local c="cart-test-api-stop"
  run_container "$c"
  wait_ready "$c"

  sleep 2

  # Start a long-running agent
  local start_resp
  start_resp=$(exec_root "$c" 'curl -sf -X POST http://localhost:4500/api/agents \
    -H "Content-Type: application/json" \
    -d "{\"provider\":\"custom\",\"prompt\":\"\",\"command\":[\"sleep\",\"300\"],\"timeout\":600}"')
  local agent_id
  agent_id=$(echo "$start_resp" | python3 -c "import sys,json;print(json.load(sys.stdin)['id'])")

  sleep 1

  # Stop it
  local stop_resp
  stop_resp=$(exec_root "$c" "curl -sf -X POST http://localhost:4500/api/agents/$agent_id/stop")
  assert_contains "agent stopped" "$stop_resp" '"stopped"'

  cleanup "$c"
}

test_api_auth() {
  echo "test: api auth"
  local c="cart-test-api-auth"
  run_container "$c" -e CARTRIDGE_API_TOKEN=test-secret-123
  wait_ready "$c"

  sleep 2

  # Without token: 401
  local no_auth
  no_auth=$(exec_root "$c" 'curl -s -o /dev/null -w "%{http_code}" http://localhost:4500/api/agents')
  assert_eq "rejects without token" "401" "$no_auth"

  # With token: 200
  local with_auth
  with_auth=$(exec_root "$c" 'curl -sf -o /dev/null -w "%{http_code}" -H "Authorization: Bearer test-secret-123" http://localhost:4500/api/agents')
  assert_eq "accepts with token" "200" "$with_auth"

  # Health bypasses auth
  local health
  health=$(exec_root "$c" 'curl -sf -o /dev/null -w "%{http_code}" http://localhost:4500/api/health')
  assert_eq "health needs no auth" "200" "$health"

  cleanup "$c"
}

test_api_hooks_installed() {
  echo "test: api hook plugins installed"
  local c="cart-test-api-hooks"
  run_container "$c"
  wait_ready "$c"

  # Claude Code plugin (symlink to /etc/cartridge/plugins/claude/cartridge-api)
  assert "claude hook plugin linked" \
    "$(exec_dev "$c" 'test -L ~/.claude/plugins/cartridge-api && test -f ~/.claude/plugins/cartridge-api/hooks/hooks.json && echo yes')"

  # Pi extension (symlink to /etc/cartridge/plugins/pi/cartridge-hook.ts)
  assert "pi extension linked" \
    "$(exec_dev "$c" 'test -L ~/.pi/agent/extensions/cartridge-hook.ts && echo yes')"

  # OpenCode plugin (symlink to /etc/cartridge/plugins/opencode/cartridge-hook.js)
  assert "opencode plugin linked" \
    "$(exec_dev "$c" 'test -L ~/.config/opencode/plugins/cartridge-hook.js && echo yes')"

  # bypassPermissions default
  local settings
  settings=$(exec_dev "$c" 'cat ~/.claude/settings.local.json 2>/dev/null')
  assert_contains "bypassPermissions set" "$settings" "bypassPermissions"

  cleanup "$c"
}

test_api_safe_mode() {
  echo "test: api safe mode"
  local c="cart-test-api-safe"
  run_container "$c" -e CARTRIDGE_API_SAFE_MODE=true
  wait_ready "$c"

  # bypassPermissions should NOT be set
  assert "settings.local.json removed" \
    "$(exec_dev "$c" 'test ! -f ~/.claude/settings.local.json && echo yes')"

  cleanup "$c"
}

test_api_hooks_disabled() {
  echo "test: api hooks disabled"
  local c="cart-test-api-nohooks"
  run_container "$c" -e CARTRIDGE_HOOKS=false
  wait_ready "$c"

  # Symlinks should be removed (not renamed to .disabled)
  assert "claude plugin removed" \
    "$(exec_dev "$c" 'test ! -e ~/.claude/plugins/cartridge-api && echo yes')"
  assert "pi extension removed" \
    "$(exec_dev "$c" 'test ! -e ~/.pi/agent/extensions/cartridge-hook.ts && echo yes')"
  assert "opencode plugin removed" \
    "$(exec_dev "$c" 'test ! -e ~/.config/opencode/plugins/cartridge-hook.js && echo yes')"

  cleanup "$c"
}

# ── Runner ───────────────────────────────────────────────────────

ALL_TESTS=(
  test_services
  test_tools
  test_workspace_empty
  test_workspace_clone
  test_workspace_existing
  test_ollama_wiring
  test_bootstrap_configs
  test_status_script
  test_user_permissions
  test_uid_remap
  test_optional_services
  test_api_health
  test_api_status
  test_api_run
  test_api_agent_lifecycle
  test_api_agent_stop
  test_api_auth
  test_api_hooks_installed
  test_api_safe_mode
  test_api_hooks_disabled
)

echo "cartridge integration tests"
echo "image: $IMAGE"
echo ""

# Run specific test or all
if [ $# -gt 0 ]; then
  "test_$1"
else
  for t in "${ALL_TESTS[@]}"; do
    $t
    echo ""
  done
fi

echo "────────────────────────"
echo "passed: $PASS  failed: $FAIL"
if [ $FAIL -gt 0 ]; then
  echo -e "\nfailures:$ERRORS"
  exit 1
fi
