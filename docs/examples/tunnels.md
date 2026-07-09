# Remote Access with Tunnels

Make the container reachable from anywhere without port forwarding.

## Tailscale (private mesh)

Best for: dev team access, peer-to-peer, accessing from mobile.

```yaml
services:
  cartridge:
    build: .
    cap_add: [SYS_ADMIN, SYS_PTRACE, NET_ADMIN]
    environment:
      - TAILSCALE_AUTHKEY=tskey-auth-...
      - TAILSCALE_HOSTNAME=cartridge-dev
    ports:
      - "127.0.0.1:7681:7681"
    volumes:
      - ./workspace:/workspace
      - ./data/config:/home/dev/.claude
      - tailscale-state:/var/lib/tailscale

volumes:
  tailscale-state:
```

```bash
docker compose up -d

# From any device on your tailnet:
# http://cartridge-dev:7681   (web terminal)
# http://cartridge-dev:9222   (Chrome CDP)
# http://cartridge-dev:6080   (noVNC, if enabled)
```

Generate an auth key at https://login.tailscale.com/admin/settings/keys.
Use a reusable, ephemeral key for containers.

### With ACL tags

```toml
# cartridge.toml
[tunnels]
tailscale_authkey = "tskey-auth-..."
tailscale_hostname = "cartridge-dev"
tailscale_tags = "tag:dev,tag:container"
```

## Cloudflare Tunnel (public URLs)

Best for: stable public URLs, sharing with external collaborators, zero-trust.

```yaml
services:
  cartridge:
    build: .
    environment:
      - CF_TUNNEL_TOKEN=eyJ...
    ports:
      - "127.0.0.1:7681:7681"
    volumes:
      - ./workspace:/workspace
```

1. Create a tunnel at https://one.dash.cloudflare.com/
2. Add a public hostname routing to `http://localhost:7681`
3. Copy the tunnel token
4. Set `CF_TUNNEL_TOKEN` in your compose or TOML

The container gets a public URL like `cartridge.yourdomain.com`.

## Both at once

```toml
# cartridge.toml
[tunnels]
tailscale_authkey = "tskey-auth-..."
tailscale_hostname = "cartridge-dev"
cf_tunnel_token = "eyJ..."
```

Tailscale for your team, Cloudflare for external access.

## SSH over Tailscale

```yaml
services:
  cartridge:
    build: .
    environment:
      - TAILSCALE_AUTHKEY=tskey-auth-...
      - TAILSCALE_HOSTNAME=cartridge-dev
      - SSH_ENABLE=true
      - SSH_AUTHORIZED_KEYS=ssh-ed25519 AAAA... user@laptop
    volumes:
      - ./workspace:/workspace
```

```bash
# From your laptop (on the tailnet):
ssh dev@cartridge-dev
```

## Verify tunnel status

```bash
# Inside the container:
cartridge-status

# Expected:
# tunnels:
#   tailscale:  ok (100.x.y.z cartridge-dev)
#   cloudflared: ok (running)
```
