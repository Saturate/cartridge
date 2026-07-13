# cartridge

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
