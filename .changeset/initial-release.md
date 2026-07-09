---
"cartridge": minor
---

Initial release of Cartridge - AI dev harness container.

- Four AI harnesses: Claude Code, Pi, OpenAI Codex, Gemini CLI
- s6-overlay v3 with 8 managed services (xvfb, ttyd, chrome, state-sync, novnc, sshd, tailscale, cloudflared)
- Provider auto-wiring via env vars or TOML config
- nvm with Node 24 + 22, Bun, Python 3
- CLI tools: rg, fd, bat, fzf, jq, gh, delta, scc, pandoc, imagemagick, ffmpeg, shoutrrr
- Workspace git auto-setup (clone, worktree, or init)
- Tailscale and Cloudflare tunnel support
- SSH with public key provisioning
- Helm chart for K8s deployment
- Integration test suite (62 assertions)
