# Implementation Design — Elasticity Demo

## Prerequisites & Assumptions

**Elasticity API**: The internal scaling API (metadata service at `169.254.169.254`) is described in `elasticity.md` but **not yet implemented** in OpenComputer. This demo assumes it will ship per that spec. Specifically:
- `POST /v1/scale` — live memory resize from inside the sandbox
- `GET /v1/limits` — query current resource limits
- CPU auto-scales with memory (1 vCPU per 1 GB)
- No reboot on resize

**External scaling API** (`PUT /api/sandboxes/:id/limits`) is also not yet implemented. The demo doesn't use it directly — the agent scales itself via the internal API — but it may be useful for monitoring/override.

**Memory cap**: OpenComputer currently enforces a 2048 MB ceiling on `Sandbox.create()`. This needs to be raised (or bypassed for this org) so the sandbox can scale to 8192 MB at runtime.

**Snapshot SDK imports**: `Image` and `Snapshots` are Node.js-only exports:
```typescript
import { Image } from "@opencomputer/sdk/dist/image.js";
import { Snapshots } from "@opencomputer/sdk/dist/snapshot.js";
```

---

## Architecture

```
┌─────────────┐    webhook     ┌─────────────┐   OC SDK    ┌──────────────────────┐
│   GitHub     │──────────────▶│   api/       │────────────▶│  OpenComputer        │
│   (issues)   │◀──────────────│   (Hono)     │             │  Sandbox (2 GB)      │
│              │  gh issue     └─────────────┘             │                      │
│              │  comment                                   │  ┌──────────────┐    │
│              │◀───────────────────────────────────────────│──│  Agent        │    │
│              │                                            │  │  (Claude SDK) │    │
└─────────────┘                                            │  └──────┬───────┘    │
                                                           │         │            │
                                                           │   curl 169.254.169.254
                                                           │         │            │
                                                           │  ┌──────▼───────┐    │
                                                           │  │  Metadata    │    │
                                                           │  │  Service     │    │
                                                           │  │  /v1/scale   │    │
                                                           │  └──────────────┘    │
                                                           └──────────────────────┘
```

**Data flow**:
1. `api/` receives GitHub webhook, creates sandbox, starts agent
2. Agent works autonomously inside sandbox (clone, fix, build, test, PR)
3. Agent hits OOM → talks to metadata service to scale up → retries → scales down
4. Agent posts status to GitHub via `gh` CLI (not through api/)
5. `api/` monitors agent events; posts failure comment only if agent crashes silently

**Key design choice**: The agent is self-sufficient. `api/` is a thin launcher — it doesn't relay messages, stream events to a UI, or manage multi-turn conversation. This is simpler than the sessions-api/bolt-new-poc pattern because there's no interactive user.

---

## Component 1: `ingest-rs/` — Data Ingestion Service

### What It Is

An HTTP service that normalizes events from multiple sources into a unified format and writes them to Postgres. Think "webhook receiver + ETL lite" — a common thing to build in Rust when you care about throughput and type safety.

**Sources** (each has its own event struct):
- GitHub webhooks (push, PR, issue, release, deployment, check_run, ...)
- Stripe webhooks (payment, invoice, subscription, refund, dispute, ...)
- Custom HTTP payloads (generic JSON events with configurable schemas)
- CSV batch uploads (parsed into typed rows)

**Pipeline**: Each source event goes through a generic transform chain:
1. `Parse<S>` — deserialize raw payload into source-specific struct
2. `Validate<S>` — enforce business rules per source type
3. `Normalize<S, U>` — map source struct to unified event format
4. `Enrich<U>` — attach metadata (timestamps, dedup keys, org context)
5. `Batch<U>` — accumulate into write batches
6. `Persist<U>` — typed sqlx insert

Each layer is generic over the event type with `Serialize + Deserialize + FromRow` bounds. With ~20 source event structs × 5-6 generic pipeline stages, `rustc` monomorphizes a lot of code in a single crate. This is the natural compilation pressure — no tricks, just a wide type surface through a generic pipeline.

### Dependencies

- `axum` — HTTP server, routing
- `serde` / `serde_json` — (de)serialization, derives on every struct
- `sqlx` — typed Postgres queries, `FromRow` derives
- `tokio` — async runtime
- `clap` — CLI config
- `tracing` — structured logging
- `chrono` — timestamps in event structs

### Structure

```
ingest-rs/
├── Cargo.toml
├── src/
│   ├── main.rs              # axum server setup, routes
│   ├── sources/
│   │   ├── mod.rs
│   │   ├── github.rs        # GitHub webhook event structs (push, pr, issue, ...)
│   │   ├── stripe.rs        # Stripe webhook event structs
│   │   ├── custom.rs        # Generic configurable event struct
│   │   └── csv.rs           # CSV row types
│   ├── pipeline/
│   │   ├── mod.rs
│   │   ├── parse.rs         # Parse<S>
│   │   ├── validate.rs      # Validate<S>
│   │   ├── normalize.rs     # Normalize<S, U>
│   │   ├── enrich.rs        # Enrich<U>
│   │   ├── batch.rs         # Batch<U>
│   │   └── persist.rs       # Persist<U> (sqlx)
│   ├── unified.rs           # Unified event type (output of normalize)
│   ├── handlers.rs          # HTTP handlers — one per source, wires source type through pipeline
│   └── db.rs                # connection pool, migrations
├── migrations/
│   └── 001_events.sql
└── README.md
```

### The Demo Issue

"Batch endpoint response is missing `processed_at` timestamp" — a simple fix: add a `processed_at: DateTime<Utc>` field to the batch response struct in `handlers.rs`, populate it from the pipeline output. The code change is small, but `cargo build` has to recompile the whole pipeline to verify it.

### Calibration

Build with `CARGO_BUILD_JOBS=1` (single-threaded, predictable memory):
- 2 GB → OOM
- 4 GB → gray zone
- 8 GB → succeeds

Tuning lever: number of source event structs. More structs = more monomorphization = more memory. Start with ~20, adjust empirically.

---

## Component 2: Agent (`agent/`)

Claude Agent SDK, runs inside an OpenComputer sandbox via `sandbox.agent.start()`. The agent wrapper (`claude-agent-wrapper`) is pre-installed in the base image — we don't ship any code for this, just a system prompt.

### How It's Loaded

The system prompt lives at `agent/prompt.md` in this repo. At runtime, `api/` reads it and passes it as the `systemPrompt` parameter to `sandbox.agent.start()`. No files are synced into the sandbox for agent config — the SDK handles it.

The issue-specific context (repo, issue number, title, body) is passed as the `prompt` parameter — the first user message that kicks off the agent.

### System Prompt (`agent/prompt.md`)

Full content — this is what the agent sees:

```markdown
You resolve GitHub issues for the `ingest-rs` project — a Rust data ingestion service.

## Workflow

1. Clone the repo: `gh repo clone <owner>/<repo>`
2. Read the issue to understand what needs to change
3. Investigate the codebase — find the relevant files, understand the structure
4. Make the fix
5. Build: `CARGO_BUILD_JOBS=1 cargo build 2>&1`
6. If the build succeeds, run tests: `cargo test 2>&1`
7. Commit, push a branch, and open a PR: `gh pr create --title "..." --body "..."`
8. Post a status comment on the original issue: `gh issue comment <number> --body "..."`

## Resource Scaling

The sandbox starts with limited memory. If a build or test fails due to insufficient memory, you can scale up.

**Detection**: Look for exit code 137, "Killed" in output, or LLVM allocation errors like:
- `LLVM ERROR: out of memory`
- `error: could not compile` with signal 9
- `fatal runtime error: out of memory`

**Scaling**:
- Check current limits: `curl -s http://169.254.169.254/v1/limits`
- Scale up: `curl -s -X POST http://169.254.169.254/v1/scale -H "Content-Type: application/json" -d '{"memoryMB": 8192}'`
- After the memory-intensive step completes, scale back down: `curl -s -X POST http://169.254.169.254/v1/scale -H "Content-Type: application/json" -d '{"memoryMB": 2048}'`

Scale down after compilation succeeds but before running tests — tests don't trigger recompilation so they need much less memory.

## Rules

- Always use `CARGO_BUILD_JOBS=1` to keep memory usage predictable
- Don't install additional Rust toolchains or components — the sandbox has what you need
- Commit on a new branch named `fix/<issue-number>-<short-description>`
- PR body should reference the issue: `Fixes #<number>`
```

### Agent SDK Configuration

```typescript
const session = await sandbox.agent.start({
  prompt: `Resolve this GitHub issue:\n\nRepo: ${repo}\nIssue #${issueNumber}: ${issueTitle}\n\n${issueBody}`,
  systemPrompt: agentPrompt,       // loaded from agent/prompt.md
  allowedTools: ["bash", "read", "write", "edit", "glob", "grep"],
  permissionMode: "bypassPermissions",
  cwd: "/workspace",
  onEvent: (event) => handleAgentEvent(event, context),
  onError: (data) => console.error("[agent stderr]", data),
  onExit: (code) => handleAgentExit(code, context),
});
```

### Agent Events We Care About

From the SDK, `AgentEvent.type` values:
- `"result"` — agent finished (success or failure). `event.subtype` is `"success"` or `"error"`.
- `"error"` — agent SDK error (not a tool error — an infrastructure failure).
- `"assistant"` — agent message (for logging/debugging, not surfaced to GitHub).
- `"tool_use_summary"` — tool invocation summary (useful for logs).

We don't need to handle `"turn_complete"` or `"interrupted"` — this is a single-shot run, not interactive.

### Sandbox Template: `rust-agent` Snapshot

The base OC template already includes `build-essential`, `git`, `curl`, `libssl-dev`, `pkg-config`. We layer Rust + gh CLI on top.

```typescript
import { Image } from "@opencomputer/sdk/dist/image.js";
import { Snapshots } from "@opencomputer/sdk/dist/snapshot.js";

const rustAgentImage = Image.base()
  .runCommands(
    'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y',
  )
  .aptInstall(["gh"])
  .env({
    PATH: "/root/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
    RUST_BACKTRACE: "1",
  })
  .workdir("/workspace");

const snapshots = new Snapshots({ apiKey, apiUrl });
await snapshots.create({
  name: "rust-agent",
  image: rustAgentImage,
  onBuildLogs: (log) => console.log(log),
});
```

This is run once via `scripts/create-snapshot.ts`. Sandboxes then boot instantly from the snapshot.

---

## Component 3: Event Handler / API (`api/`)

Thin webhook handler + sandbox launcher. No database, no event persistence, no UI. Receives a GitHub webhook, spins up a sandbox, starts the agent, monitors for completion.

### Dependencies

- `hono` — HTTP framework (consistent with agents-api pattern)
- `@opencomputer/sdk` — sandbox creation + agent sessions
- `@hono/node-server` — run Hono on Node.js
- Node.js `crypto` — HMAC-SHA256 webhook signature verification
- `fetch` (built-in) — GitHub API calls for posting comments

No @octokit — we only make 2-3 GitHub API calls (post comment), raw `fetch` is simpler.

### Structure

```
api/
├── package.json
├── tsconfig.json
├── .env.example
├── src/
│   ├── index.ts            # Hono app, bind routes, start server
│   ├── webhook.ts          # POST /webhooks/github — verify, parse, dispatch
│   ├── sandbox.ts          # createSandbox(), startAgent(), killSandbox()
│   └── github.ts           # postComment(), verifySignature() — thin wrappers
└── scripts/
    └── create-snapshot.ts  # One-time: build rust-agent snapshot
```

### API Surface

Single endpoint:

```
POST /webhooks/github
  Headers: X-Hub-Signature-256, X-GitHub-Event
  Body: GitHub webhook payload (issue_comment.created)
  Response: 200 OK (immediate, async processing)
```

Plus a health check:

```
GET /health
  Response: 200 OK
```

### Environment Variables

```bash
# api/ server
OPENCOMPUTER_API_KEY=       # OC API key for sandbox creation
OPENCOMPUTER_API_URL=       # OC API endpoint (e.g. https://api.opencomputer.dev)
GITHUB_TOKEN=               # PAT with repo scope — for posting comments + passed to sandbox
GITHUB_WEBHOOK_SECRET=      # Shared secret for webhook HMAC verification
ANTHROPIC_API_KEY=          # Passed through to sandbox for Claude agent
PORT=3000                   # Server port (default 3000)
```

### Webhook Handler (`webhook.ts`)

```typescript
import { Hono } from "hono";
import { verifySignature, postComment } from "./github";
import { launchAgent } from "./sandbox";

const TRIGGER = "@myagent";

export const webhook = new Hono();

webhook.post("/webhooks/github", async (c) => {
  // 1. Verify HMAC-SHA256 signature
  const body = await c.req.text();
  const sig = c.req.header("x-hub-signature-256") ?? "";
  if (!verifySignature(body, sig)) return c.text("bad signature", 401);

  const event = c.req.header("x-github-event");
  if (event !== "issue_comment") return c.text("ignored", 200);

  const payload = JSON.parse(body);
  if (payload.action !== "created") return c.text("ignored", 200);

  const comment = payload.comment.body as string;
  if (!comment.includes(TRIGGER)) return c.text("ignored", 200);

  // 2. Extract issue context
  const issue = payload.issue;
  const repo = payload.repository.full_name;  // "owner/repo"
  const context = {
    repo,
    issueNumber: issue.number as number,
    issueTitle: issue.title as string,
    issueBody: issue.body as string,
    commentUrl: payload.comment.html_url as string,
  };

  // 3. Acknowledge immediately, process async
  //    Post "On it..." comment before returning
  await postComment(repo, context.issueNumber,
    `⏳ Working on it — sandbox starting...`
  );

  // 4. Launch agent in background (don't await in request handler)
  launchAgent(context).catch((err) => {
    console.error("Agent launch failed:", err);
    postComment(repo, context.issueNumber,
      `❌ Agent failed to start: ${err.message}`
    ).catch(() => {});
  });

  return c.text("ok", 200);
});
```

### GitHub Helpers (`github.ts`)

```typescript
import { createHmac, timingSafeEqual } from "node:crypto";

const WEBHOOK_SECRET = process.env.GITHUB_WEBHOOK_SECRET!;
const GITHUB_TOKEN = process.env.GITHUB_TOKEN!;

export function verifySignature(payload: string, signature: string): boolean {
  const expected = "sha256=" + createHmac("sha256", WEBHOOK_SECRET)
    .update(payload)
    .digest("hex");
  if (expected.length !== signature.length) return false;
  return timingSafeEqual(Buffer.from(expected), Buffer.from(signature));
}

export async function postComment(
  repo: string, issueNumber: number, body: string,
): Promise<void> {
  await fetch(`https://api.github.com/repos/${repo}/issues/${issueNumber}/comments`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${GITHUB_TOKEN}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ body }),
  });
}
```

### Sandbox Orchestration (`sandbox.ts`)

```typescript
import { Sandbox } from "@opencomputer/sdk";
import { readFileSync } from "node:fs";
import { postComment } from "./github";

const agentPrompt = readFileSync("../agent/prompt.md", "utf-8");

interface IssueContext {
  repo: string;
  issueNumber: number;
  issueTitle: string;
  issueBody: string;
}

export async function launchAgent(ctx: IssueContext): Promise<void> {
  const sandbox = await Sandbox.create({
    snapshot: "rust-agent",
    timeout: 1800,                        // 30 min max
    memoryMB: 2048,                       // deliberately undersized
    envs: {
      GITHUB_TOKEN: process.env.GITHUB_TOKEN!,
      ANTHROPIC_API_KEY: process.env.ANTHROPIC_API_KEY!,
      CARGO_BUILD_JOBS: "1",              // predictable memory usage
    },
  });

  console.log(`Sandbox created: ${sandbox.sandboxId} for issue #${ctx.issueNumber}`);

  try {
    const session = await sandbox.agent.start({
      prompt: [
        `Resolve this GitHub issue:`,
        ``,
        `Repo: ${ctx.repo}`,
        `Issue #${ctx.issueNumber}: ${ctx.issueTitle}`,
        ``,
        ctx.issueBody,
      ].join("\n"),
      systemPrompt: agentPrompt,
      allowedTools: ["bash", "read", "write", "edit", "glob", "grep"],
      permissionMode: "bypassPermissions",
      cwd: "/workspace",
      onEvent: (event) => {
        // Log all events for observability
        console.log(`[agent:${sandbox.sandboxId}] ${event.type}`,
          event.type === "tool_use_summary" ? event.tool : "");
      },
      onError: (data) => {
        console.error(`[agent:${sandbox.sandboxId}] error:`, data);
      },
    });

    // Wait for agent to finish
    const exitCode = await session.done;
    console.log(`Agent exited with code ${exitCode}`);

    if (exitCode !== 0) {
      await postComment(ctx.repo, ctx.issueNumber,
        `❌ Agent exited with code ${exitCode}. Check logs for details.`
      );
    }
    // On success: the agent already posted its own comment with the PR link
  } finally {
    await sandbox.kill();
    console.log(`Sandbox ${sandbox.sandboxId} killed`);
  }
}
```

### Concurrency

For the demo, one agent at a time is fine. If a second webhook arrives while an agent is running, `launchAgent` spawns a second sandbox — no coordination needed. For production use this would need a queue, but this is a demo.

### Deployment

- **Dev**: Run locally + ngrok/cloudflare tunnel for webhook delivery
- **Demo**: Fly.io (single machine, same pattern as agents-control)
- **Config**: GitHub webhook pointing at `https://<host>/webhooks/github` with `issue_comment` events enabled

---

## Setup & Bootstrap

### One-Time: Create Snapshot

```bash
cd api/
OPENCOMPUTER_API_KEY=... OPENCOMPUTER_API_URL=... npx tsx scripts/create-snapshot.ts
```

This builds the `rust-agent` snapshot (Rust toolchain + gh CLI on base image). Takes a few minutes. After that, sandboxes boot from it instantly.

### One-Time: Create Demo Repo

Push `ingest-rs/` to a GitHub repo (e.g. `demo-org/ingest-rs`). Create the demo issue: "Batch endpoint response is missing `processed_at` timestamp". Leave it open.

### One-Time: Configure GitHub Webhook

On the repo, add a webhook:
- URL: `https://<api-host>/webhooks/github`
- Content type: `application/json`
- Secret: same as `GITHUB_WEBHOOK_SECRET`
- Events: select "Issue comments" only

### Run

```bash
cd api/
cp .env.example .env   # fill in values
npm install
npm run dev             # starts on :3000
```

Then comment `@myagent resolve this` on the demo issue.

---

## Full Sequence (detail)

```
Time  What
─────────────────────────────────────────────────────
0:00  User comments "@myagent resolve this" on issue #42
0:01  GitHub sends POST /webhooks/github to api/
0:01  api/ verifies signature, parses payload
0:01  api/ posts "Working on it..." comment via GitHub API
0:02  api/ calls Sandbox.create({ snapshot: "rust-agent", memoryMB: 2048 })
0:05  api/ calls sandbox.agent.start({ prompt: issue context, systemPrompt })

      ── Agent running inside sandbox ──

0:10  Agent: gh repo clone demo-org/ingest-rs
0:15  Agent: reads issue #42, investigates codebase
1:00  Agent: makes the fix (adds processed_at field)
1:30  Agent: CARGO_BUILD_JOBS=1 cargo build 2>&1
3:00  Agent: build killed (exit 137) — OOM at 2 GB
3:05  Agent: curl -s http://169.254.169.254/v1/limits → {"memLimit": 2048, ...}
3:10  Agent: curl -s -X POST http://169.254.169.254/v1/scale -d '{"memoryMB": 8192}'
3:15  Agent: CARGO_BUILD_JOBS=1 cargo build 2>&1
5:00  Agent: build succeeds (at 8 GB)
5:05  Agent: curl -s -X POST http://169.254.169.254/v1/scale -d '{"memoryMB": 2048}'
5:10  Agent: cargo test 2>&1 → pass
5:30  Agent: git checkout -b fix/42-add-processed-at
5:35  Agent: git commit, git push
5:40  Agent: gh pr create --title "Add processed_at to batch response" --body "Fixes #42"
5:45  Agent: gh issue comment 42 --body "PR submitted: <link>"

      ── Agent exits ──

5:50  api/ receives exit code 0 via session.done
5:50  api/ kills sandbox

Total session: ~6 min
Time at 8 GB: ~2 min (3:10 → 5:05)
Time at 2 GB: ~4 min (everything else)
```

---

## Repo Layout (final)

```
demo-elasticity/
├── AGENTS.md                      # Stable reference
├── elasticity.md                  # Scaling API spec (assumed contract)
├── .agents-wip/
│   └── design.md                  # This file
├── ingest-rs/                     # Rust data ingestion service
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs
│   │   ├── sources/{github,stripe,custom,csv}.rs
│   │   ├── pipeline/{parse,validate,normalize,enrich,batch,persist}.rs
│   │   ├── unified.rs
│   │   ├── handlers.rs
│   │   └── db.rs
│   └── migrations/
│       └── 001_events.sql
├── agent/
│   └── prompt.md                  # System prompt (read by api/ at runtime)
└── api/
    ├── package.json
    ├── tsconfig.json
    ├── .env.example
    ├── src/
    │   ├── index.ts               # Hono app, server
    │   ├── webhook.ts             # Webhook handler
    │   ├── sandbox.ts             # OC SDK orchestration
    │   └── github.ts              # Signature verification, comment posting
    └── scripts/
        └── create-snapshot.ts     # One-time snapshot builder
```

---

## Open Questions

- **Calibration**: Need to empirically verify the memory profile by building `ingest-rs` under constrained memory. Number of source event structs is the tuning lever.
- **OOM detection reliability**: Exit 137 is clear. `rustc` LLVM errors may look different. System prompt covers both patterns but needs testing.
- **Real repo vs. demo org**: Real public repo is more convincing but needs cleanup between demo runs. Dedicated demo org is safer.
- **Snapshot durability**: How long do OC snapshots persist? Need to confirm they survive across days/weeks or have a re-creation mechanism.

## Resolved

- **Sandbox template**: Declarative snapshot via `Image.base().runCommands(rustup).aptInstall([gh])`. Default base already has build-essential, git, curl, libssl-dev. Snapshot persists org-wide, boots instantly.
- **Agent config delivery**: System prompt passed via SDK `systemPrompt` parameter, not synced as files. Simpler.
- **Status reporting**: Agent posts its own GitHub comments via `gh` CLI. api/ only posts initial ack and failure fallback.
- **Framework**: Hono + raw fetch. No @octokit — only 2-3 GitHub API calls needed.
- **Elasticity API contract**: Per `elasticity.md`. Not yet implemented in OC — demo assumes it ships. See Prerequisites.
