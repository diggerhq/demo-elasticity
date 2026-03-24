# Implementation Design — Elasticity Demo

## Prerequisites & Assumptions

**Elasticity API**: Implemented and deployed on the `feat/qemu-backend-azure` branch of OpenComputer (the branch running at `app.opencomputer.dev`). Verified working:
- `POST /v1/scale` — live memory resize from inside the sandbox (**tested: 896→1920 MB**)
- `GET /v1/limits` — returns memLimit, memUsage, cpuPercent, pids, network stats
- `GET /v1/status` — returns sandboxId, uptime
- `GET /v1/metadata` — returns region, template
- CPU auto-scales with memory (1 vCPU per 4 GB based on source code)

**External scaling API**: `POST /api/sandboxes/:id/scale` with `{"memoryMB": N}` — also deployed and working.

**Memory cap**: OpenComputer currently enforces a 2048 MB ceiling on `Sandbox.create()`. This needs to be raised (or bypassed for this org) so the sandbox can scale to 8192 MB at runtime.

**Sandbox runs as root**: OC sandboxes run as root. Claude Code CLI refuses `--allow-dangerously-skip-permissions` as root for security. Agent uses `permissionMode: "acceptEdits"` instead.

**Rootfs is small (1.7 GB)**: The base sandbox rootfs is only 1.7 GB. The data disk (`/workspace`) is 20 GB. Rust toolchain (~1 GB) must install to `/workspace/.rustup` and `/workspace/.cargo`, not the default `/root/` paths.

**Declarative snapshots timeout**: `Snapshots.create()` with a full `Image` definition times out through Cloudflare (524) because the build takes too long before the first response byte. The working path is manual: create a sandbox → run setup commands step by step → checkpoint.

**Claude Code CLI required**: `@anthropic-ai/claude-agent-sdk` spawns the `claude` CLI as a subprocess. It must be installed globally (`npm install -g @anthropic-ai/claude-code`) in the snapshot.

---

## Architecture

```
┌─────────────┐    webhook     ┌─────────────┐  OC SDK     ┌──────────────────────────┐
│   GitHub     │──────────────▶│   api/       │────────────▶│  OpenComputer Sandbox     │
│   (issues)   │◀──────────────│   (Hono)     │  exec.start │  (2 GB → 8 GB → 2 GB)    │
│              │  gh comment   └─────────────┘             │                          │
│              │◀──────────────────────────────────────────│──┐                       │
└─────────────┘                                           │  │ agent/ (Node.js)       │
                                                          │  │ query() → Claude API   │
                                                          │  │   ↕ tool calls         │
                                                          │  │ bash, read, edit, ...  │
                                                          │  │   ↕                    │
                                                          │  │ curl 169.254.169.254   │
                                                          │  │   → /v1/scale          │
                                                          │  └────────────────────────│
                                                          └──────────────────────────┘
```

**Data flow**:
1. `api/` receives GitHub webhook, creates sandbox, runs agent as a process
2. Agent (real Node.js program using Claude Agent SDK) works autonomously — clone, fix, build, test, PR
3. Agent hits OOM → calls metadata service to scale up → retries → scales down
4. Agent posts status to GitHub via `gh` CLI (not through api/)
5. `api/` monitors exec session exit code; posts failure comment only if agent crashes silently

**Key design choices**:
- **Agent is real code**: A standalone Node.js program using `@anthropic-ai/claude-agent-sdk`'s `query()` API. Runs locally or in a sandbox. The sandbox is compute, not an agent framework. See AGENTS.md for full reasoning.
- **api/ uses `sandbox.exec`**: Not `sandbox.agent.start()`. The agent is deployed as code, not as a prompt config.
- **Agent owns its own deployment**: `agent/` has a deploy script that packages everything (code + deps) into a named snapshot. api/ just references the snapshot — zero knowledge of agent internals.

---

## Component 1: `ingest-rs/` — Data Ingestion Service

### What It Is

An HTTP service that normalizes events from multiple sources into a unified format and writes them to stdout as JSON Lines. Think "webhook receiver + transform pipeline" — a common thing to build in Rust when you care about throughput and type safety.

No database. The pipeline parses, validates, normalizes, and serializes — that's enough to exercise the full generic machinery. A DB would add setup complexity without contributing to the monomorphization pressure that's the whole point of this component.

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
6. `Emit<U>` — serialize batch to JSON and write (stdout in CLI mode, response body in HTTP mode)

Each layer is generic over the event type with `Serialize + Deserialize` bounds. With ~20 source event structs × 6 generic pipeline stages, `rustc` monomorphizes a lot of code in a single crate. This is the natural compilation pressure — no tricks, just a wide type surface through a generic pipeline.

### Dependencies

- `axum` — HTTP server, routing
- `serde` / `serde_json` — (de)serialization, derives on every struct
- `tokio` — async runtime
- `clap` — CLI config
- `tracing` — structured logging
- `chrono` — timestamps in event structs

No database dependency.

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
│   │   └── emit.rs          # Emit<U> — serialize to JSON
│   ├── unified.rs           # Unified event type (output of normalize)
│   └── handlers.rs          # HTTP handlers — one per source, wires source through pipeline
├── tests/
│   └── pipeline_test.rs     # End-to-end pipeline tests (no external deps)
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

A standalone Node.js application using `@anthropic-ai/claude-agent-sdk`. Resolves a GitHub issue by cloning the repo, investigating, fixing, building, testing, and opening a PR.

Can run locally (`.env` auto-loaded by dotenv):
```bash
cd agent
npm run dev -- --repo diggerhq/demo-elasticity --issue 1
```

Or inside an OpenComputer sandbox (deployed as a snapshot via `npm run deploy`).

### Dependencies

```json
{
  "dependencies": {
    "@anthropic-ai/claude-agent-sdk": "^0.2.71",
    "dotenv": "^17"
  },
  "devDependencies": {
    "@opencomputer/sdk": "file:../../opencomputer/sdks/typescript",
    "tsx": "^4",
    "typescript": "^5"
  }
}
```

Two runtime deps: agent SDK + dotenv. `@opencomputer/sdk` is a devDep installed from local source (published npm version lags behind — missing `snapshot`, `exec`, `secretStore`).

### Environment Variables

**Runtime** (inside sandbox, from SecretStore):
```bash
ANTHROPIC_API_KEY=           # Claude API — injected by SecretStore
GITHUB_TOKEN=                # gh CLI auth — injected by SecretStore
```

**Deploy script** (local):
```bash
OPENCOMPUTER_API_KEY=        # OC API key for snapshot creation
OPENCOMPUTER_API_URL=        # OC API endpoint
```

**Local testing** (`.env`, auto-loaded by dotenv):
```bash
ANTHROPIC_API_KEY=           # Claude API
GITHUB_TOKEN=                # gh CLI auth
```

### Structure

```
agent/
├── package.json
├── tsconfig.json
├── .env.example
├── src/
│   └── index.ts           # Entry point — parses args, runs query(), handles result
├── prompt.md              # System prompt — loaded by index.ts at runtime
└── scripts/
    ├── deploy.ts          # Declarative deploy (doesn't work — Cloudflare timeout)
    └── deploy-manual.ts   # Working deploy: sandbox → steps → checkpoint
```

### Entry Point (`src/index.ts`)

See `agent/src/index.ts` for actual code. Key points:
- `import "dotenv/config"` at top — auto-loads `.env`
- `mkdtempSync()` creates a fresh temp dir for `cwd` (avoids cloning into source tree)
- `query()` with `permissionMode: "acceptEdits"` — sandbox runs as root, `bypassPermissions` is rejected by Claude Code CLI
- Event loop logs `[agent]` text and `[tool]` calls with inputs for visibility
- Prints duration + cost on completion

### System Prompt (`prompt.md`)

See `agent/prompt.md` for actual content. Covers: workflow (clone → investigate → fix → build → test → PR), elasticity scaling instructions (OOM detection patterns, metadata service curl commands), and rules (CARGO_BUILD_JOBS=1, branch naming). Tells the agent that `ingest-rs/` is a subdirectory of the repo.

### Running Locally vs. In Sandbox

The same code runs in both environments. Locally, the agent creates a temp dir via `mkdtempSync()` and works there. In the sandbox, `AGENT_WORKDIR=/workspace` overrides this. The elasticity `curl` commands will 404 locally (no metadata service) but the build will just work if your machine has enough RAM.

**Note**: Running locally the agent uses `acceptEdits` mode — it will execute bash commands and edit files without asking. For safer local testing, use a sandbox (test level 2).

### Deployment (`scripts/deploy-manual.ts`)

The agent owns its own packaging. `deploy-manual.ts` creates a sandbox, installs tooling step by step, uploads agent code, and checkpoints. The declarative `deploy.ts` (using `Snapshots.create()`) exists but doesn't work — Cloudflare times out before the build completes.

See `agent/scripts/deploy-manual.ts` for actual code. Steps:
1. Create base sandbox (1h timeout)
2. Install Rust to `/workspace/.rustup` + `/workspace/.cargo` (rootfs too small)
3. Install gh CLI via apt
4. Install Claude Code CLI globally (`npm install -g @anthropic-ai/claude-code`)
5. Upload agent source files via `sandbox.files.write()`
6. Run `npm install && npm run build` inside sandbox
7. Set env vars in `.bashrc` (PATH, RUSTUP_HOME, CARGO_HOME, AGENT_WORKDIR)
8. Create checkpoint, poll until status = "ready"
9. Kill sandbox

```bash
cd agent
npx tsx scripts/deploy-manual.ts
```

**Snapshot contents** (checkpoint `rust-agent`):

| Layer | What |
|-------|------|
| OC base image | Ubuntu, build-essential, git, curl, libssl-dev, pkg-config, python3, Node.js 20 |
| Rust | `/workspace/.rustup` + `/workspace/.cargo` (stable 1.94.0) |
| gh CLI | Via apt (2.88.1) |
| Claude Code CLI | `@anthropic-ai/claude-code` global npm package |
| Agent | `/workspace/agent/` — source, node_modules, built dist |

**Sandbox creation**: `Sandbox.createFromCheckpoint(checkpointId)` — uses the checkpoint ID, not a snapshot name. The checkpoint ID is stored in api/'s `.env` as `CHECKPOINT_ID`.

**When to redeploy**: Any change to agent source, prompt, or deps. Rerun `deploy-manual.ts` → get new checkpoint ID → update `CHECKPOINT_ID` in api/.env.

---

## Component 3: Event Handler / API (`api/`)

Thin webhook handler + sandbox launcher. Receives a GitHub webhook, creates a sandbox from the agent's pre-built snapshot, runs the agent, monitors exit. Has zero knowledge of agent internals.

### Dependencies

- `hono` — HTTP framework
- `@opencomputer/sdk` — sandbox creation + exec
- `@hono/node-server` — run Hono on Node.js
- `@octokit/webhooks` — GitHub webhook signature verification
- `@octokit/rest` — GitHub API (post comments)

### Structure

```
api/
├── package.json
├── tsconfig.json
├── .env.example
└── src/
    ├── index.ts            # Hono app, bind routes, start server
    ├── webhook.ts          # POST /webhooks/github — verify, parse, dispatch
    ├── sandbox.ts          # Sandbox.create() + exec.start()
    └── github.ts           # postComment(), verifySignature()
```

### API Surface

```
POST /webhooks/github
  Headers: X-Hub-Signature-256, X-GitHub-Event
  Body: GitHub webhook payload (issue_comment.created)
  Response: 200 OK (immediate, async processing)

GET /health
  Response: 200 OK
```

### Environment Variables

```bash
OPENCOMPUTER_API_KEY=       # OC API key for sandbox creation
OPENCOMPUTER_API_URL=       # OC API endpoint
GITHUB_TOKEN=               # PAT with repo scope — for posting comments + passed to sandbox
GITHUB_WEBHOOK_SECRET=      # Shared secret for webhook HMAC verification
ANTHROPIC_API_KEY=          # Passed to sandbox for Claude agent
CHECKPOINT_ID=              # Checkpoint ID from deploy-manual.ts output
PORT=3000                   # Server port (default 3000)
```

Secrets are passed to the sandbox via env vars in `exec.start()`. SecretStore is the intended long-term solution but isn't set up yet — env vars work for the demo.

### Entry Point (`index.ts`)

```typescript
import { serve } from "@hono/node-server";
import { Hono } from "hono";
import { webhook } from "./webhook";

const app = new Hono();
app.route("/", webhook);
app.get("/health", (c) => c.text("ok"));

const port = parseInt(process.env.PORT ?? "3000");
serve({ fetch: app.fetch, port });
console.log(`Listening on :${port}`);
```

### Webhook Handler (`webhook.ts`)

```typescript
import { Hono } from "hono";
import { webhooks, postComment } from "./github";
import { runAgent } from "./sandbox";

const TRIGGER = "@myagent";

export const webhook = new Hono();

webhook.post("/webhooks/github", async (c) => {
  const body = await c.req.text();
  const sig = c.req.header("x-hub-signature-256") ?? "";

  if (!(await webhooks.verify(body, sig))) return c.text("bad signature", 401);

  const event = c.req.header("x-github-event");
  if (event !== "issue_comment") return c.text("ignored", 200);

  const payload = JSON.parse(body);
  if (payload.action !== "created") return c.text("ignored", 200);
  if (!payload.comment.body.includes(TRIGGER)) return c.text("ignored", 200);

  const ctx = {
    repo: payload.repository.full_name,
    issueNumber: payload.issue.number,
  };

  await postComment(ctx.repo, ctx.issueNumber, "⏳ Working on it — sandbox starting...");

  runAgent(ctx).catch((err) => {
    console.error("Agent failed:", err);
    postComment(ctx.repo, ctx.issueNumber, `❌ Agent failed: ${err.message}`).catch(() => {});
  });

  return c.text("ok", 200);
});
```

### GitHub Helpers (`github.ts`)

```typescript
import { Webhooks } from "@octokit/webhooks";
import { Octokit } from "@octokit/rest";

export const webhooks = new Webhooks({ secret: process.env.GITHUB_WEBHOOK_SECRET! });

export const octokit = new Octokit({ auth: process.env.GITHUB_TOKEN });

export async function postComment(repo: string, issue: number, body: string): Promise<void> {
  const [owner, name] = repo.split("/");
  await octokit.issues.createComment({ owner, repo: name, issue_number: issue, body });
}
```

### Sandbox Orchestration (`sandbox.ts`)

See `api/src/sandbox.ts` for actual code. Key points:
- Uses `Sandbox.createFromCheckpoint(CHECKPOINT_ID)` — not `Sandbox.create({ snapshot })`. Checkpoint ID from deploy output.
- Passes Rust env vars (`RUSTUP_HOME`, `CARGO_HOME`, `PATH`) + secrets (`ANTHROPIC_API_KEY`, `GITHUB_TOKEN`) as `env` to `exec.start()`. The `.bashrc` approach doesn't work in non-interactive exec.
- Streams stdout/stderr from the agent process.
- Posts failure comment on non-zero exit, kills sandbox in `finally`.

api/ knows: the checkpoint ID, the entry point path, the CLI args, and the env vars to pass. That's it.

### Concurrency

For the demo, one agent at a time is fine. Each webhook spawns an independent sandbox — no coordination needed.

### Deployment

- **Dev**: Run locally + ngrok/cloudflare tunnel for webhook delivery
- **Demo**: Fly.io (single machine, same pattern as agents-control)

---

## Setup & Operations

### Bootstrap (once)

Fill in `.env` files for both `agent/` and `api/` with the required credentials:
- `OPENCOMPUTER_API_KEY` + `OPENCOMPUTER_API_URL` — OC access
- `ANTHROPIC_API_KEY` — Claude API key
- `GITHUB_TOKEN` — GitHub PAT with repo scope

Long-term, secrets should move to an OC SecretStore for egress control. For now, env vars work.

### Deploy (on agent code change)

```bash
cd agent
npm install                        # only needed once locally
npx tsx scripts/deploy-manual.ts   # creates sandbox, installs tools, checkpoints
```

Output includes the checkpoint ID. Update `CHECKPOINT_ID` in `api/.env` with the new value. Agent code change → redeploy → update checkpoint ID → next sandbox picks it up.

### Run

```bash
cd api
cp .env.example .env   # OPENCOMPUTER_API_KEY, GITHUB_TOKEN (api/'s own), GITHUB_WEBHOOK_SECRET
npm install
npm run dev             # starts on :3000
```

### Testing

Three levels, from inner to outer. You can test each independently.

#### 1. Agent locally (no sandbox, no OC)

Run the agent on your machine against the demo repo + issue:

```bash
cd agent
npm run dev -- --repo diggerhq/demo-elasticity --issue 1
```

The agent will clone, investigate, fix, build, test, and PR — same as in a sandbox. Elasticity `curl` commands will 404 (no metadata service) but that's fine — your machine has enough RAM so the build won't OOM. This tests agent logic, prompt quality, and the full workflow without any OC dependency.

**Caveat**: The agent runs with `acceptEdits` — it executes bash commands and edits files without prompting. For safer testing, use level 2 (sandbox).

#### 2. Agent in sandbox (no webhook, no elasticity)

Test the sandbox deployment path without waiting for the elasticity API. Use a small script (`api/scripts/test-sandbox.ts`) that calls `runAgent()` directly:

```typescript
import { runAgent } from "../src/sandbox";

await runAgent({
  repo: "diggerhq/demo-elasticity",
  issueNumber: 1,
});
```

```bash
cd api
npx tsx scripts/test-sandbox.ts
```

This creates a real sandbox from the snapshot, runs the agent inside it, and streams output. Skips the webhook path entirely. To test without the elasticity API, temporarily start the sandbox at 8 GB (`memoryMB: 8192` in the test script) so the build succeeds without scaling — verifies the full sandbox → agent → GitHub flow.

#### 3. End-to-end with webhook

Start api/, expose it via ngrok, configure the webhook on the demo repo, and comment `@myagent resolve this` on the issue. This is the full demo flow.

```bash
# Terminal 1
cd api && npm run dev

# Terminal 2
ngrok http 3000
# → copy the https URL, set it as the webhook URL on the GitHub repo
```

#### What you can test before the elasticity API ships

Everything except the actual scaling. Levels 1 and 2 work today. For level 2, start at 8 GB to skip the OOM/scale-up cycle. Once the elasticity API lands, switch back to 2 GB and test the full OOM → scale → retry → scale-down sequence.

#### Calibrating ingest-rs memory profile

Once ingest-rs is built, verify it actually OOMs at 2 GB. Use level 2 (sandbox) with different `memoryMB` values:

```typescript
// In test-sandbox.ts, try different sizes:
const sandbox = await Sandbox.create({ snapshot: "rust-agent", memoryMB: 2048, ... });
// → agent should hit OOM on cargo build

const sandbox = await Sandbox.create({ snapshot: "rust-agent", memoryMB: 8192, ... });
// → agent should succeed
```

If 2 GB doesn't OOM, add more event source structs to ingest-rs (the tuning lever). If 8 GB isn't enough, reduce the struct count or allow more parallelism.

---

## Full Sequence

```
Time  What
─────────────────────────────────────────────────────
0:00  User comments "@myagent resolve this" on issue #42
0:01  GitHub POSTs webhook to api/
0:01  api/ verifies signature, posts "Working on it..." comment
0:02  api/ → Sandbox.createFromCheckpoint(CHECKPOINT_ID)
0:03  api/ → sandbox.exec.start("node /workspace/agent/dist/index.js --repo ... --issue 42")

      ── Agent process running inside sandbox ──
      ── (query() loop: Claude API ↔ tool calls) ──

0:10  Agent: gh issue view 42 → reads issue body
0:15  Agent: gh repo clone diggerhq/demo-elasticity
0:30  Agent: investigates codebase, makes the fix
1:30  Agent: CARGO_BUILD_JOBS=1 cargo build 2>&1
3:00  Build killed (exit 137) — OOM at 2 GB
3:05  Agent: curl -s http://169.254.169.254/v1/limits → 2048 MB
3:10  Agent: curl -s -X POST http://169.254.169.254/v1/scale -d '{"memoryMB": 8192}'
3:15  Agent: CARGO_BUILD_JOBS=1 cargo build 2>&1
5:00  Build succeeds (at 8 GB)
5:05  Agent: curl -s -X POST http://169.254.169.254/v1/scale -d '{"memoryMB": 2048}'
5:10  Agent: cargo test → pass
5:30  Agent: git checkout -b fix/42-add-processed-at, commit, push
5:40  Agent: gh pr create --title "Add processed_at ..." --body "Fixes #42"
5:45  Agent: gh issue comment 42 --body "PR submitted: <link>"

      ── Agent process exits (code 0) ──

5:50  api/ sees exit code 0 via session.done
5:50  api/ kills sandbox

Total session: ~6 min
Time at 8 GB: ~2 min (3:10 → 5:05)
Time at 2 GB: ~4 min (everything else)
```

---

## Repo Layout (final)

```
demo-elasticity/
├── AGENTS.md                      # Stable reference + design decisions
├── elasticity.md                  # Scaling API spec (assumed contract)
├── .agents-wip/
│   └── design.md                  # This file
│
├── ingest-rs/                     # Rust data ingestion service (the target repo)
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs
│   │   ├── sources/{github,stripe,custom,csv}.rs
│   │   ├── pipeline/{parse,validate,normalize,enrich,batch,emit}.rs
│   │   ├── unified.rs
│   │   └── handlers.rs
│   ├── tests/
│   │   └── pipeline_test.rs
│   └── README.md
│
├── agent/                         # The agent — standalone Node.js program
│   ├── package.json               # @anthropic-ai/claude-agent-sdk + tsx
│   ├── tsconfig.json
│   ├── src/
│   │   └── index.ts               # Entry: parse args, query(), handle result
│   ├── prompt.md                  # System prompt (loaded by index.ts)
│   └── scripts/
│       ├── deploy.ts              # Declarative deploy (doesn't work — Cloudflare timeout)
│       └── deploy-manual.ts       # Working deploy: sandbox → steps → checkpoint
│
└── api/                           # Webhook handler + sandbox launcher
    ├── package.json               # hono, @opencomputer/sdk, @octokit/*
    ├── tsconfig.json
    ├── .env.example
    ├── src/
    │   ├── index.ts               # Hono app, server
    │   ├── webhook.ts             # Webhook handler
    │   ├── sandbox.ts             # createFromCheckpoint() + exec.start()
    │   └── github.ts              # Octokit helpers
    └── scripts/
        ├── test-sandbox.ts        # Direct agent test (no webhook)
        └── test-debug.ts          # Debug test with full output capture
```

---

## Implementation Notes

**Source is authority**: Code sketches in this doc may lag the actual source files. When in doubt, read `agent/src/index.ts`, `api/src/sandbox.ts`, and `agent/scripts/deploy-manual.ts` directly.

**OC SDK**: Installed from local source (`file:../../opencomputer/sdks/typescript`), not npm. The published version lags behind — missing `snapshot`, `exec`, `secretStore`. SDK source at `../opencomputer/sdks/typescript/src/`. We added `"./dist/*": "./dist/*"` to the SDK's package.json exports to enable subpath imports for `Image` and `Snapshots`.

**ingest-rs is a subdirectory**: `ingest-rs/` lives inside this repo (`diggerhq/demo-elasticity`). The agent clones the whole repo and works in the `ingest-rs/` subdirectory. The system prompt tells it this.

**Agent SDK `query()` returns an async generator**: The stream yields `SDKMessage` objects. The code handles `"assistant"` and `"result"` types. For the full type union, check `../base360-checkin-agent/agent/node_modules/@anthropic-ai/claude-agent-sdk/sdk.d.ts`.

**Sandbox env vars for Rust**: The `.bashrc` approach doesn't work for `exec.start()` commands (non-interactive shell). Rust env vars (`RUSTUP_HOME`, `CARGO_HOME`, `PATH`) must be passed explicitly in the `env` option of `exec.start()`.

---

## Open Questions

- **Calibration**: Need to empirically verify the memory profile by building `ingest-rs` under constrained memory (2 GB sandbox). Number of source event structs is the tuning lever. The current ~20 structs may not be enough — hasn't been tested under memory constraint yet.
- **OOM detection reliability**: Exit 137 is clear. `rustc` LLVM errors may look different. System prompt covers both patterns but needs testing under actual OOM conditions.
- **Declarative snapshots**: `Snapshots.create()` with `Image` definitions times out through Cloudflare. Either OC needs to stream build logs faster (keep Cloudflare alive) or we keep using the manual checkpoint approach.
- **SecretStore migration**: Currently passing secrets as env vars. Should move to OC SecretStore for egress control, but works for demo.

## Resolved

- **Agent is real code**: Standalone Node.js program using `query()` from `@anthropic-ai/claude-agent-sdk`. Not prompt-only via `sandbox.agent.start()`. See AGENTS.md for reasoning.
- **No database in ingest-rs**: Pipeline writes JSON to stdout/response. Monomorphization pressure from generics, not storage.
- **Agent owns deployment**: `deploy-manual.ts` builds the checkpoint. api/ references a checkpoint ID.
- **Status reporting**: Agent posts its own GitHub comments via `gh` CLI. api/ only posts initial ack and failure fallback.
- **Framework**: Hono + @octokit/rest + @octokit/webhooks for GitHub interaction.
- **Elasticity API contract**: Per `elasticity.md`. Not yet implemented in OC — demo assumes it ships.
- **Permission mode**: `acceptEdits` not `bypassPermissions` — sandbox runs as root, Claude Code CLI rejects `--allow-dangerously-skip-permissions` as root.
- **Rust install path**: `/workspace/.rustup` + `/workspace/.cargo` — rootfs is only 1.7 GB, data disk (`/workspace`) has 20 GB.
- **Base image**: Already has Node.js 20, build-essential, git, curl. Need to add: Rust, gh CLI, Claude Code CLI.
- **Checkpoint-based deploy**: Manual approach (create sandbox → run steps → checkpoint) works around Cloudflare timeout on declarative snapshots.
- **E2E verified**: Agent completed successfully in sandbox — ~3 min, $0.46 cost. Created PR #2 on diggerhq/demo-elasticity.
