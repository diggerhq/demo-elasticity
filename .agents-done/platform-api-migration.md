# Platform API Migration

How demo-elasticity benefits from the sessions-api Agent/Instance/Session model. Design doc: `../../sessions-api/.agents-wip/agents-instances-sessions.md`.

## Current Architecture

Three components, all custom:

```
GitHub webhook
  → api/  (custom Hono server, ~200 LOC)
      OC SDK: create sandbox from checkpoint, exec.start(), post status to GitHub
  → agent/  (Claude Agent SDK, standalone Node.js)
      clones repo, investigates, fixes, scales up/down, opens PR
  → ingest-rs/  (the heavy Rust project)
```

`api/` does too much: receives webhooks, manages sandbox lifecycle, posts GitHub comments, handles cleanup. It's tightly coupled to OC SDK internals (checkpoint IDs, exec.start options, sandbox teardown).

## After Migration

```
GitHub webhook
  → handler  (~50 LOC, no OC SDK dependency)
      POST /v1/agents/issue-resolver/sessions  { input: {...} }
      post "Agent started..." comment to GitHub
  → agent/  (unchanged)
      runs inside session sandbox, posts results to GitHub directly
  → ingest-rs/  (unchanged)
```

### Agent Definition (created once)

```
POST /v1/agents
{
  "id": "issue-resolver",
  "display_name": "Issue Resolver",
  "config": {
    "snapshot": "rust-agent-v1",
    "entrypoint": "node /workspace/agent/dist/index.js",
    "env": {
      "CARGO_BUILD_JOBS": "1"
    },
    "elasticity": {
      "baseline_mb": 2048,
      "max_mb": 8192
    },
    "idle_timeout_s": 1800
  }
}

PUT /v1/agents/issue-resolver/secrets/ANTHROPIC_API_KEY
  { "value": "sk-ant-...", "allowed_hosts": ["api.anthropic.com"] }

PUT /v1/agents/issue-resolver/secrets/GITHUB_TOKEN
  { "value": "ghp_...", "allowed_hosts": ["api.github.com"] }
```

### Session Creation (per webhook)

```
POST /v1/agents/issue-resolver/sessions
{
  "input": {
    "repo": "acme/ingest-rs",
    "issue_number": 42,
    "comment_author": "alice",
    "comment_body": "@myagent resolve this"
  }
}

201 Created
{
  "id": "sess_01jz...",
  "agent_id": "issue-resolver",
  "status": "creating"
}
```

The platform:
1. Creates sandbox from `rust-agent-v1` snapshot at 2048 MB
2. Injects secrets via OC secret store (sealed tokens)
3. Runs entrypoint
4. Agent reads input from `AGENT_INPUT_PATH` (or however we wire input — TBD)
5. Agent does work, scales up/down, posts to GitHub, opens PR
6. Agent exits → session status becomes `completed` or `failed`
7. Sandbox destroyed

### Webhook Handler (replaces api/)

```typescript
import { Hono } from "hono"
import { Webhooks } from "@octokit/webhooks"
import { Octokit } from "@octokit/rest"

const app = new Hono()
const webhooks = new Webhooks({ secret: process.env.GITHUB_WEBHOOK_SECRET })
const octokit = new Octokit({ auth: process.env.GITHUB_TOKEN })

app.post("/webhook/github", async (c) => {
  const event = await webhooks.verify(/* ... */)

  // Filter: only issue comments mentioning @myagent
  if (!event.comment.body.includes("@myagent")) return c.json({ skip: true })

  // Post "starting" comment
  await octokit.issues.createComment({
    owner, repo, issue_number,
    body: "Agent starting..."
  })

  // Create session — fire and forget
  await fetch(`${PLATFORM_API}/v1/agents/issue-resolver/sessions`, {
    method: "POST",
    headers: { "X-API-Key": PLATFORM_API_KEY },
    body: JSON.stringify({
      input: { repo: `${owner}/${repo}`, issue_number, ... }
    })
  })

  return c.json({ ok: true })
})
```

That's it. ~50 lines. No OC SDK, no sandbox lifecycle, no checkpoint IDs, no cleanup logic.

## What Changes

| Concern | Before | After |
|---------|--------|-------|
| Sandbox creation | `api/` calls OC SDK directly | Platform handles via session creation |
| Checkpoint management | Hardcoded `CHECKPOINT_ID` env var in `api/` | `snapshot` on Agent config |
| Secret injection | `api/` passes env vars to `sandbox.exec.start()` | Secrets API + OC secret store (sealed tokens) |
| Elasticity config | Implicit (agent just calls metadata service) | Declared on Agent (`baseline_mb`, `max_mb`) |
| Cleanup on crash | `api/` must handle (and currently doesn't well) | Platform manages session lifecycle |
| Observability | None (fire and forget) | SSE event stream with scale events, status |
| GitHub interaction | Split: `api/` posts "starting", agent posts results | Handler posts "starting", agent posts everything else |

## What Doesn't Change

- `agent/` — unchanged. Still a standalone Claude Agent SDK program. Still calls the metadata service for scaling. Still posts to GitHub with `gh` CLI.
- `ingest-rs/` — unchanged. Still the compilation target.
- Elasticity flow — unchanged. Agent detects OOM, queries limits, scales up, retries, scales down. The metadata service API is the same.

## Input Wiring

Resolved: platform writes `session.input` to `/tmp/agent_input.json` and sets `AGENT_INPUT_PATH` env var. Agent reads the file. Same convention as agents-api.

```
Platform writes:  /tmp/agent_input.json
Platform sets:    AGENT_INPUT_PATH=/tmp/agent_input.json
Agent reads:      JSON.parse(fs.readFileSync(process.env.AGENT_INPUT_PATH))
```

The agent entrypoint changes from CLI args to reading the input file:
```json
// Before: "entrypoint": "node /workspace/agent/dist/index.js --repo acme/backend --issue 42"
// After:  "entrypoint": "node /workspace/agent/dist/index.js"
// Agent reads repo + issue_number from AGENT_INPUT_PATH
```

Agent writes results to `AGENT_RESULT_PATH=/tmp/agent_result.json` before exiting. Platform reads it and stores as `session.result`.

## Migration Steps

1. Register Agent definition and secrets via platform API (manual or script)
2. Rewrite `api/` as thin webhook handler (no OC SDK)
3. Modify `agent/` to read input from `AGENT_INPUT_PATH` instead of CLI args (small change)
4. Update deploy script to build snapshot instead of checkpoint
5. Delete old `api/` OC SDK integration code

Steps 1-2 can happen first. Step 3 is a minor agent change. Steps 4-5 are cleanup.

## Relationship to oc-demo-slack

Both demos validate the same platform API from different angles:

- **demo-elasticity**: Session (process mode), fire-and-forget, GitHub trigger
- **oc-demo-slack**: Instance (persistent), interactive, Slack trigger

Same Agent config shape. Same secrets mechanism. Same elasticity API. Different runtime models — which is exactly why both Instance and Session exist as distinct concepts.
