# cartridge

## 0.2.0

### Minor Changes

- [#7](https://github.com/Saturate/cartridge/pull/7) [`201793b`](https://github.com/Saturate/cartridge/commit/201793b9d4cd2e2b2dab41751cdd9d39613a1888) Thanks [@Saturate](https://github.com/Saturate)! - Auto-complete Claude Code onboarding when auth credentials are present, with support for custom config templates via CLAUDE_CONFIG_TEMPLATE or /etc/cartridge/claude.json

### Patch Changes

- [#5](https://github.com/Saturate/cartridge/pull/5) [`a20c322`](https://github.com/Saturate/cartridge/commit/a20c3228887b85f40777206a47896269fe21f58f) Thanks [@Saturate](https://github.com/Saturate)! - Suppress Chromium crashpad handler errors in container by adding --disable-crashpad, --crash-dumps-dir, and clearing CHROME_CRASHPAD_PIPE_NAME

## 0.1.0

### Minor Changes

- [`dae3299`](https://github.com/Saturate/cartridge/commit/dae3299e5d1f0c22476f30af2cb9d5beb05f8499) Thanks [@Saturate](https://github.com/Saturate)! - Initial release of Cartridge - AI dev harness container.

  - Four AI harnesses: Claude Code, Pi, OpenAI Codex, Gemini CLI
  - HTTP/WebSocket API for agent lifecycle management
  - CLI commands: spawn, list, show, stop, logs
  - Inter-agent messaging
  - Browser terminal viewer
  - s6-overlay v3 with managed services (xvfb, ttyd, chrome, state-sync, novnc, sshd, tailscale, cloudflared)
  - Provider auto-wiring via env vars or TOML config
  - nvm with Node 24 + 22, Bun, Python 3
  - CLI tools: rg, fd, bat, fzf, jq, gh, delta, scc, pandoc, imagemagick, ffmpeg, shoutrrr
  - Workspace git auto-setup (clone, worktree, or init)
  - Tailscale and Cloudflare tunnel support
  - SSH with public key provisioning
  - Helm chart for K8s deployment
  - Integration test suite (62 assertions)
