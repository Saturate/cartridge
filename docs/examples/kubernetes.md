# Kubernetes Deployment

One pod per developer session via the Helm chart.

## Basic install

```bash
helm install dev helm/cartridge/ \
  --set providers.anthropicApiKey=sk-ant-...
```

## With Ollama (in-cluster)

```bash
helm install dev helm/cartridge/ \
  --set providers.ollamaHost=http://ollama.default.svc:11434
```

## With Tailscale (access from anywhere)

```bash
helm install dev helm/cartridge/ \
  --set providers.anthropicApiKey=sk-ant-... \
  --set tunnels.tailscale.authkey=tskey-auth-... \
  --set tunnels.tailscale.hostname=cartridge-dev
```

Access at `http://cartridge-dev:7681` from any device on your tailnet.

## With ingress

```bash
helm install dev helm/cartridge/ \
  --set providers.anthropicApiKey=sk-ant-... \
  --set ingress.enabled=true \
  --set ingress.className=nginx \
  --set 'ingress.hosts[0].host=cartridge.example.com' \
  --set 'ingress.hosts[0].paths[0].path=/' \
  --set 'ingress.hosts[0].paths[0].port=ttyd'
```

## Full production values

```yaml
# values-prod.yaml
image:
  repository: ghcr.io/saturate/cartridge
  tag: latest

replicaCount: 1

resources:
  requests:
    cpu: "2"
    memory: 4Gi
  limits:
    cpu: "4"
    memory: 8Gi

persistence:
  config:
    size: 2Gi
    storageClass: fast-ssd
  workspace:
    size: 50Gi
    storageClass: fast-ssd

providers:
  anthropicApiKey: ""  # set via --set or external secret
  ollamaHost: http://ollama.ai-infra.svc:11434
  ghToken: ""

workspace:
  repoUrl: https://github.com/org/project
  branch: main

tunnels:
  tailscale:
    authkey: ""
    hostname: cartridge-prod
    tags: "tag:dev"

husk:
  enabled: true
  apiKey: ""

ssh:
  enabled: true
  authorizedKeys: |
    ssh-ed25519 AAAA... dev@laptop

config: |
  [notifications]
  shoutrrr_url = "slack://token/channel"
```

```bash
helm install prod helm/cartridge/ -f values-prod.yaml \
  --set providers.anthropicApiKey=$ANTHROPIC_API_KEY \
  --set providers.ghToken=$GH_TOKEN \
  --set tunnels.tailscale.authkey=$TAILSCALE_AUTHKEY \
  --set husk.apiKey=$HUSK_API_KEY
```

## Subscription auth in K8s

For Claude Pro/Max (not API key):

```bash
# 1. Generate a token on your laptop
claude setup-token

# 2. Create a K8s secret from the auth files
kubectl create secret generic claude-auth \
  --from-file=auth.json=$HOME/.claude/auth.json

# 3. Mount it in the Helm chart
helm install dev helm/cartridge/ \
  --set providers.authSecretName=claude-auth
```

## Multiple sessions

```bash
# Each install is a separate session with its own PVCs
helm install session-alice helm/cartridge/ \
  --set providers.anthropicApiKey=$ALICE_KEY \
  --set tunnels.tailscale.hostname=cartridge-alice

helm install session-bob helm/cartridge/ \
  --set providers.anthropicApiKey=$BOB_KEY \
  --set tunnels.tailscale.hostname=cartridge-bob
```
