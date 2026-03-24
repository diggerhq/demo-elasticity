# demo-elasticity

An AI agent that resolves GitHub issues in an [OpenComputer](https://opencomputer.dev) sandbox with **live resource scaling**. The sandbox starts small, scales up for heavy compilation, and scales back down — paying for peak resources only when needed.

## What it does

1. You comment `@myagent resolve this` on a GitHub issue
2. A webhook triggers the API, which spins up an OpenComputer sandbox
3. An AI agent (Claude) clones the repo, reads the issue, investigates, and makes a fix
4. The agent detects the sandbox has limited memory, **scales up to 8 GB** via the metadata service
5. Builds the Rust project, **scales back down** to baseline
6. Runs tests, commits, pushes a branch, opens a PR, and comments on the issue

The whole flow takes ~2 minutes and costs ~$0.30 in API calls.

## Architecture

```
GitHub Issue → Webhook → API (Hono) → OpenComputer Sandbox
                                         ├── Agent (Claude Agent SDK)
                                         ├── scales 896 MB → 8 GB → 2 GB
                                         └── clone → fix → build → test → PR
```

**Three components:**

| Component | What | Tech |
|-----------|------|------|
| `ingest-rs/` | Rust data pipeline — the project the agent fixes | axum, serde, tokio, reqwest |
| `agent/` | AI agent that resolves issues | @anthropic-ai/claude-agent-sdk |
| `api/` | Webhook handler that launches sandboxes | Hono, @opencomputer/sdk, @octokit |

## Setup

### Prerequisites

- Node.js 20+
- An [OpenComputer](https://opencomputer.dev) API key
- An [Anthropic](https://console.anthropic.com) API key
- A GitHub PAT with `repo` scope
- [ngrok](https://ngrok.com) or similar for webhook delivery

### 1. Install dependencies

```bash
cd agent && npm install
cd ../api && npm install
```

### 2. Configure environment

```bash
# agent/.env
ANTHROPIC_API_KEY=sk-ant-...
GITHUB_TOKEN=ghp_...
OPENCOMPUTER_API_KEY=osb_...
OPENCOMPUTER_API_URL=https://app.opencomputer.dev

# api/.env
OPENCOMPUTER_API_KEY=osb_...
OPENCOMPUTER_API_URL=https://app.opencomputer.dev
GITHUB_TOKEN=ghp_...
GITHUB_WEBHOOK_SECRET=your-webhook-secret
ANTHROPIC_API_KEY=sk-ant-...
CHECKPOINT_ID=           # filled in after deploy
PORT=3000
```

### 3. Deploy the agent snapshot

This creates an OpenComputer checkpoint with Rust, Node.js, gh CLI, Claude Code, and the agent code:

```bash
cd agent
npx tsx scripts/deploy-manual.ts
```

Copy the checkpoint ID from the output and set it as `CHECKPOINT_ID` in `api/.env`.

### 4. Configure GitHub webhook

On your repo, add a webhook:
- **URL**: `https://<your-ngrok-url>/webhooks/github`
- **Content type**: `application/json`
- **Secret**: same as `GITHUB_WEBHOOK_SECRET`
- **Events**: Issue comments only

### 5. Run

```bash
# Terminal 1 — API server
cd api && npm run dev

# Terminal 2 — expose to internet
ngrok http 3000
```

Then comment `@myagent resolve this` on an issue.

## How the elasticity works

The sandbox starts with ~896 MB of memory. The agent:

1. Checks current resources: `curl http://169.254.169.254/v1/limits`
2. Scales up: `curl -X POST http://169.254.169.254/v1/scale -d '{"memoryMB": 8192}'`
3. Builds the Rust project (needs the extra memory for compilation)
4. Scales back down: `curl -X POST http://169.254.169.254/v1/scale -d '{"memoryMB": 2048}'`
5. Runs tests at lower memory (no recompilation needed)

The scaling happens live — no reboot, no checkpoint-restore. The VM's memory limit changes in-place via cgroup updates.

## Redeploying

When you change agent code or prompt:

```bash
cd agent
npx tsx scripts/deploy-manual.ts
# Update CHECKPOINT_ID in api/.env with the new ID
# Restart api/
```

## Testing locally

The agent can run outside a sandbox:

```bash
cd agent
npm run dev -- --repo diggerhq/demo-elasticity --issue 1
```

The elasticity API calls will 404 (no metadata service) but the build will work if your machine has enough RAM.
