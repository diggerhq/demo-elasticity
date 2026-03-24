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
1. `api/` receives GitHub webhook, creates sandbox, syncs agent source, runs it as a process
2. Agent (real Node.js program using Claude Agent SDK) works autonomously — clone, fix, build, test, PR
3. Agent hits OOM → calls metadata service to scale up → retries → scales down
4. Agent posts status to GitHub via `gh` CLI (not through api/)
5. `api/` monitors exec session exit code; posts failure comment only if agent crashes silently

**Key design choices**:
- **Agent is real code**: A standalone Node.js program using `@anthropic-ai/claude-agent-sdk`'s `query()` API. Runs locally or in a sandbox. The sandbox is compute, not an agent framework. See AGENTS.md for full reasoning.
- **api/ uses `sandbox.exec`**: Not `sandbox.agent.start()`. The agent is deployed as code, not as a prompt config.
- **Deps in snapshot, source synced at runtime**: Agent npm dependencies (slow to install) are pre-installed in the snapshot. Agent source code (2 small files) is synced at launch time via `sandbox.files.write()`. This means agent code changes don't require a snapshot rebuild.

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

No database dependency. No sqlx, no migrations, no connection pool.

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

Can run locally:
```bash
cd agent
ANTHROPIC_API_KEY=... GITHUB_TOKEN=... npx tsx src/index.ts --repo owner/repo --issue 42
```

Or inside an OpenComputer sandbox (source synced by api/, deps pre-installed in snapshot).

### Dependencies

```json
{
  "dependencies": {
    "@anthropic-ai/claude-agent-sdk": "^0.2.71"
  },
  "devDependencies": {
    "tsx": "^4",
    "typescript": "^5"
  }
}
```

Single meaningful dependency. The SDK handles tool execution (bash, file ops), Claude API calls, and the agentic loop.

### Structure

```
agent/
├── package.json
├── tsconfig.json
├── src/
│   └── index.ts           # Entry point — parses args, runs query(), handles result
└── prompt.md              # System prompt — loaded by index.ts at runtime
```

### Entry Point (`src/index.ts`)

```typescript
import { query } from "@anthropic-ai/claude-agent-sdk";
import { readFileSync } from "node:fs";
import { parseArgs } from "node:util";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));

const { values } = parseArgs({
  options: {
    repo:  { type: "string" },
    issue: { type: "string" },
  },
  strict: true,
});

if (!values.repo || !values.issue) {
  console.error("Usage: index.ts --repo owner/repo --issue 42");
  process.exit(1);
}

const systemPrompt = readFileSync(join(__dirname, "../prompt.md"), "utf-8");

const stream = query({
  prompt: [
    `Resolve this GitHub issue.`,
    ``,
    `Repository: ${values.repo}`,
    `Issue number: ${values.issue}`,
    ``,
    `Start by running: gh issue view ${values.issue} --repo ${values.repo}`,
  ].join("\n"),
  options: {
    model: "claude-sonnet-4-20250514",
    systemPrompt,
    tools: ["Bash", "Read", "Write", "Edit", "Glob", "Grep"],
    allowedTools: ["Bash", "Read", "Write", "Edit", "Glob", "Grep"],
    permissionMode: "bypassPermissions",
    allowDangerouslySkipPermissions: true,
    cwd: "/workspace",
    maxTurns: 50,
  },
});

let exitCode = 0;

for await (const message of stream) {
  if (message.type === "assistant" && message.message?.content) {
    for (const block of message.message.content) {
      if (block.type === "text") {
        console.log("[agent]", block.text?.slice(0, 200));
      }
    }
  }

  if (message.type === "result") {
    if (message.subtype === "success") {
      console.log("Agent completed successfully.");
    } else {
      console.error("Agent failed:", message.result ?? "unknown error");
      exitCode = 1;
    }
  }
}

process.exit(exitCode);
```

### System Prompt (`prompt.md`)

```markdown
You resolve GitHub issues for the `ingest-rs` project — a Rust data ingestion service.

## Workflow

1. Read the issue with `gh issue view` to understand what needs to change
2. Clone the repo: `gh repo clone <owner>/<repo>`
3. Investigate the codebase — find the relevant files, understand the structure
4. Make the fix
5. Build: `CARGO_BUILD_JOBS=1 cargo build 2>&1`
6. If the build succeeds, run tests: `cargo test 2>&1`
7. Commit, push a branch, and open a PR: `gh pr create --title "..." --body "..."`
8. Post a status comment on the original issue: `gh issue comment <number> --body "..."`

## Resource Scaling

The sandbox starts with limited memory. If a build or test fails due to insufficient
memory, you can scale up.

**Detection**: Look for exit code 137, "Killed" in output, or LLVM allocation errors like:
- `LLVM ERROR: out of memory`
- `error: could not compile` with signal 9
- `fatal runtime error: out of memory`

**Scaling** (via instance metadata service):
- Check current limits: `curl -s http://169.254.169.254/v1/limits`
- Scale up: `curl -s -X POST http://169.254.169.254/v1/scale -H "Content-Type: application/json" -d '{"memoryMB": 8192}'`
- After the memory-intensive step completes, scale back down: `curl -s -X POST http://169.254.169.254/v1/scale -H "Content-Type: application/json" -d '{"memoryMB": 2048}'`

Scale down after compilation succeeds but before running tests — tests don't trigger
recompilation so they need much less memory.

## Rules

- Always use `CARGO_BUILD_JOBS=1` to keep memory usage predictable
- Don't install additional Rust toolchains or components — the sandbox has what you need
- Commit on a new branch named `fix/<issue-number>-<short-description>`
- PR body should reference the issue: `Fixes #<number>`
```

### Running Locally vs. In Sandbox

The same code runs in both environments. The only differences are:
- **Locally**: `cwd` would be wherever you want to clone into. The elasticity `curl` commands will 404 (no metadata service), but the build will just work if your machine has enough RAM.
- **In sandbox**: `cwd` is `/workspace`. Elasticity API is available. `GITHUB_TOKEN` is injected by the sandbox env.

No code changes, no conditional logic. The agent doesn't know or care where it's running.

---

## Component 3: Event Handler / API (`api/`)

Thin webhook handler + sandbox launcher. Receives a GitHub webhook, creates a sandbox, syncs agent source into it, runs the agent as a process, monitors exit.

### Dependencies

- `hono` — HTTP framework
- `@opencomputer/sdk` — sandbox creation + exec + filesystem
- `@hono/node-server` — run Hono on Node.js
- Node.js `crypto` — HMAC-SHA256 webhook signature verification
- `fetch` (built-in) — GitHub API calls for posting comments

No @octokit — we only make 2-3 GitHub API calls, raw `fetch` is simpler.

### Structure

```
api/
├── package.json
├── tsconfig.json
├── .env.example
├── src/
│   ├── index.ts            # Hono app, bind routes, start server
│   ├── webhook.ts          # POST /webhooks/github — verify, parse, dispatch
│   ├── sandbox.ts          # createSandbox(), syncAgent(), runAgent()
│   └── github.ts           # postComment(), verifySignature()
└── scripts/
    └── create-snapshot.ts  # One-time: build snapshot with Rust + Node.js + agent deps
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
ANTHROPIC_API_KEY=          # Passed through to sandbox for Claude agent
PORT=3000                   # Server port (default 3000)
```

### Webhook Handler (`webhook.ts`)

```typescript
import { Hono } from "hono";
import { verifySignature, postComment } from "./github";
import { runAgent } from "./sandbox";

const TRIGGER = "@myagent";

export const webhook = new Hono();

webhook.post("/webhooks/github", async (c) => {
  const body = await c.req.text();
  const sig = c.req.header("x-hub-signature-256") ?? "";
  if (!verifySignature(body, sig)) return c.text("bad signature", 401);

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
import { createHmac, timingSafeEqual } from "node:crypto";

export function verifySignature(payload: string, signature: string): boolean {
  const expected = "sha256=" + createHmac("sha256", process.env.GITHUB_WEBHOOK_SECRET!)
    .update(payload).digest("hex");
  if (expected.length !== signature.length) return false;
  return timingSafeEqual(Buffer.from(expected), Buffer.from(signature));
}

export async function postComment(repo: string, issue: number, body: string): Promise<void> {
  await fetch(`https://api.github.com/repos/${repo}/issues/${issue}/comments`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${process.env.GITHUB_TOKEN}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ body }),
  });
}
```

### Sandbox Orchestration (`sandbox.ts`)

Creates sandbox, syncs agent source, runs it as a process.

```typescript
import { Sandbox } from "@opencomputer/sdk";
import { readFileSync } from "node:fs";
import { postComment } from "./github";

// Load agent source files at startup (small — index.ts + prompt.md)
const agentSource = readFileSync("../agent/src/index.ts", "utf-8");
const agentPrompt = readFileSync("../agent/prompt.md", "utf-8");

interface RunContext {
  repo: string;
  issueNumber: number;
}

export async function runAgent(ctx: RunContext): Promise<void> {
  // 1. Create sandbox — deps are pre-installed in snapshot, source is not
  const sandbox = await Sandbox.create({
    snapshot: "rust-agent",
    timeout: 1800,
    memoryMB: 2048,
    envs: {
      GITHUB_TOKEN: process.env.GITHUB_TOKEN!,
      ANTHROPIC_API_KEY: process.env.ANTHROPIC_API_KEY!,
      CARGO_BUILD_JOBS: "1",
    },
  });

  console.log(`Sandbox ${sandbox.sandboxId} created for #${ctx.issueNumber}`);

  try {
    // 2. Sync agent source into sandbox (2 small files, instant)
    //    node_modules/ is already in the snapshot from npm install at build time
    await sandbox.files.makeDir("/workspace/agent/src");
    await sandbox.files.write("/workspace/agent/src/index.ts", agentSource);
    await sandbox.files.write("/workspace/agent/prompt.md", agentPrompt);

    // 3. Run agent — same command you'd use locally
    const session = await sandbox.exec.start(
      "npx",
      {
        args: [
          "tsx", "src/index.ts",
          "--repo", ctx.repo,
          "--issue", String(ctx.issueNumber),
        ],
        cwd: "/workspace/agent",
        timeout: 1500,
        onStdout: (data) => process.stdout.write(data),
        onStderr: (data) => process.stderr.write(data),
      },
    );

    // 4. Wait for exit
    const exitCode = await session.done;
    console.log(`Agent exited: ${exitCode}`);

    if (exitCode !== 0) {
      await postComment(ctx.repo, ctx.issueNumber,
        `❌ Agent exited with code ${exitCode}. Check logs for details.`
      );
    }
  } finally {
    await sandbox.kill();
    console.log(`Sandbox ${sandbox.sandboxId} killed`);
  }
}
```

**Key points**:
- `sandbox.files.write()` syncs 2 small text files — instant
- `npx tsx src/index.ts` uses tsx from the pre-installed node_modules
- Agent source changes don't require snapshot rebuild — just restart api/
- Snapshot rebuild only needed when agent deps change (rare)

### Concurrency

For the demo, one agent at a time is fine. Each webhook spawns an independent sandbox — no coordination needed.

### Deployment

- **Dev**: Run locally + ngrok/cloudflare tunnel for webhook delivery
- **Demo**: Fly.io (single machine, same pattern as agents-control)

---

## Snapshot: `rust-agent`

The snapshot has the runtime environment + agent dependencies. Agent source code is synced at launch time.

### What's In It

| Layer | What | Why |
|-------|------|-----|
| OC base image | Ubuntu, build-essential, git, curl, libssl-dev, pkg-config, python3 | Already there |
| Rust | `rustup` + stable toolchain | Compile ingest-rs |
| Node.js 22 | Via nodesource | Run the agent |
| gh CLI | Via apt | GitHub interaction from agent |
| Agent deps | `/workspace/agent/node_modules/` | Pre-installed, avoids npm install at launch |

**Not in snapshot**: agent source code (`src/index.ts`, `prompt.md`). These are synced at runtime so code changes don't require a snapshot rebuild.

### Build Script (`scripts/create-snapshot.ts`)

```typescript
import { readFileSync } from "node:fs";
import { Image } from "@opencomputer/sdk/dist/image.js";
import { Snapshots } from "@opencomputer/sdk/dist/snapshot.js";

const apiKey = process.env.OPENCOMPUTER_API_KEY!;
const apiUrl = process.env.OPENCOMPUTER_API_URL!;

// Only embed package.json — source code is synced at runtime
const agentPackageJson = readFileSync("../agent/package.json", "utf-8");

const image = Image.base()
  // Rust toolchain
  .runCommands(
    'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y',
  )
  // Node.js 22 (for the agent)
  .runCommands(
    "curl -fsSL https://deb.nodesource.com/setup_22.x | bash -",
    "apt-get install -y nodejs",
  )
  // gh CLI
  .aptInstall(["gh"])
  // Agent dependencies only (source synced at runtime)
  .addFile("/workspace/agent/package.json", agentPackageJson)
  .runCommands("cd /workspace/agent && npm install")
  // Environment
  .env({
    PATH: "/root/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
    RUST_BACKTRACE: "1",
  })
  .workdir("/workspace");

const snapshots = new Snapshots({ apiKey, apiUrl });
await snapshots.create({
  name: "rust-agent",
  image,
  onBuildLogs: (log) => console.log(log),
});

console.log("Snapshot 'rust-agent' created.");
```

### When to Rebuild

- Agent deps change in `package.json` (rare)
- Rust toolchain or Node.js version update
- New system packages needed

Agent source code changes (prompt, logic) do **not** require rebuild — they're synced at launch.

---

## Setup & Bootstrap

### 1. Build the Agent (local verification)

```bash
cd agent
npm install
ANTHROPIC_API_KEY=... GITHUB_TOKEN=... npx tsx src/index.ts --repo owner/repo --issue 42
```

### 2. Create the Snapshot

```bash
cd api
OPENCOMPUTER_API_KEY=... OPENCOMPUTER_API_URL=... npx tsx scripts/create-snapshot.ts
```

### 3. Push ingest-rs

Push `ingest-rs/` to a GitHub repo (e.g. `demo-org/ingest-rs`). Create the demo issue: "Batch endpoint response is missing `processed_at` timestamp". Leave it open.

### 4. Configure GitHub Webhook

On the repo, add a webhook:
- URL: `https://<api-host>/webhooks/github`
- Content type: `application/json`
- Secret: same as `GITHUB_WEBHOOK_SECRET`
- Events: "Issue comments" only

### 5. Run the API

```bash
cd api
cp .env.example .env   # fill in values
npm install
npm run dev             # starts on :3000
```

Then comment `@myagent resolve this` on the demo issue.

---

## Full Sequence

```
Time  What
─────────────────────────────────────────────────────
0:00  User comments "@myagent resolve this" on issue #42
0:01  GitHub POSTs webhook to api/
0:01  api/ verifies signature, posts "Working on it..." comment
0:02  api/ → Sandbox.create({ snapshot: "rust-agent", memoryMB: 2048 })
0:03  api/ → sandbox.files.write() — syncs index.ts + prompt.md
0:03  api/ → sandbox.exec.start("npx tsx src/index.ts --repo ... --issue 42")

      ── Agent process running inside sandbox ──
      ── (query() loop: Claude API ↔ tool calls) ──

0:10  Agent: gh issue view 42 → reads issue body
0:15  Agent: gh repo clone demo-org/ingest-rs
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
│   │   ├── pipeline/{parse,validate,normalize,enrich,batch,persist}.rs
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
│   └── prompt.md                  # System prompt (loaded by index.ts)
│
└── api/                           # Webhook handler + sandbox launcher
    ├── package.json               # hono, @opencomputer/sdk
    ├── tsconfig.json
    ├── .env.example
    ├── src/
    │   ├── index.ts               # Hono app, server
    │   ├── webhook.ts             # Webhook handler
    │   ├── sandbox.ts             # Sandbox.create() + files.write() + exec.start()
    │   └── github.ts              # Signature verification, comment posting
    └── scripts/
        └── create-snapshot.ts     # One-time: build snapshot (deps only, no source)
```

---

## Open Questions

- **Calibration**: Need to empirically verify the memory profile by building `ingest-rs` under constrained memory. Number of source event structs is the tuning lever.
- **OOM detection reliability**: Exit 137 is clear. `rustc` LLVM errors may look different. System prompt covers both patterns but needs testing.
- **Real repo vs. demo org**: Real public repo is more convincing but needs cleanup between demo runs. Dedicated demo org is safer.
- **Snapshot durability**: How long do OC snapshots persist? Need to confirm they survive across days/weeks or have a re-creation mechanism.
- **tsx in snapshot**: Need to verify that `npx tsx` works from pre-installed `node_modules` without a full `package.json` alongside the source. Fallback: sync `tsconfig.json` too, or use `node --import tsx/esm`.

## Resolved

- **Agent is real code**: Standalone Node.js program using `query()` from `@anthropic-ai/claude-agent-sdk`. Not prompt-only via `sandbox.agent.start()`. Runs locally or in sandbox. See AGENTS.md for reasoning.
- **No database in ingest-rs**: Pipeline writes JSON to stdout/response. No sqlx, no Postgres, no migrations. Monomorphization pressure comes from the generic pipeline across ~20 event types, not from DB code.
- **Agent deployment split**: Deps (`node_modules/`) baked into snapshot. Source (`src/index.ts`, `prompt.md`) synced at runtime via `sandbox.files.write()`. Code changes don't require snapshot rebuild.
- **Status reporting**: Agent posts its own GitHub comments via `gh` CLI. api/ only posts initial ack and failure fallback.
- **Framework**: Hono + raw fetch. No @octokit — only 2-3 GitHub API calls needed.
- **Elasticity API contract**: Per `elasticity.md`. Not yet implemented in OC — demo assumes it ships. See Prerequisites.
