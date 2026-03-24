# Implementation Design — Elasticity Demo

## Prerequisites & Assumptions

**Elasticity API**: The internal scaling API (metadata service at `169.254.169.254`) is described in `elasticity.md` but **not yet implemented** in OpenComputer. This demo assumes it will ship per that spec. Specifically:
- `POST /v1/scale` — live memory resize from inside the sandbox
- `GET /v1/limits` — query current resource limits
- CPU auto-scales with memory (1 vCPU per 1 GB)
- No reboot on resize

**External scaling API** (`PUT /api/sandboxes/:id/limits`) is also not yet implemented. The demo doesn't use it directly — the agent scales itself via the internal API — but it may be useful for monitoring/override.

**Memory cap**: OpenComputer currently enforces a 2048 MB ceiling on `Sandbox.create()`. This needs to be raised (or bypassed for this org) so the sandbox can scale to 8192 MB at runtime.

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

Can run locally:
```bash
cd agent
ANTHROPIC_API_KEY=... GITHUB_TOKEN=... npx tsx src/index.ts --repo owner/repo --issue 42
```

Or inside an OpenComputer sandbox (deployed as a snapshot via `scripts/deploy.ts`).

### Dependencies

```json
{
  "dependencies": {
    "@anthropic-ai/claude-agent-sdk": "^0.2.71"
  },
  "devDependencies": {
    "@opencomputer/sdk": "^0.4",
    "tsx": "^4",
    "typescript": "^5"
  }
}
```

One runtime dependency (the agent SDK). `@opencomputer/sdk` is a devDep — only used by the deploy script, not shipped into the snapshot.

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

**Local testing** (`.env` or shell):
```bash
ANTHROPIC_API_KEY=           # Claude API
GITHUB_TOKEN=                # gh CLI auth
AGENT_WORKDIR=/tmp/workspace # optional, defaults to cwd
```

### Structure

```
agent/
├── package.json
├── tsconfig.json
├── src/
│   └── index.ts           # Entry point — parses args, runs query(), handles result
├── prompt.md              # System prompt — loaded by index.ts at runtime
└── scripts/
    └── deploy.ts          # Packages agent + env into an OC snapshot
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
    model: "claude-sonnet-4-6",
    systemPrompt,
    tools: ["Bash", "Read", "Write", "Edit", "Glob", "Grep"],
    allowedTools: ["Bash", "Read", "Write", "Edit", "Glob", "Grep"],
    permissionMode: "bypassPermissions",
    allowDangerouslySkipPermissions: true,
    cwd: process.env.AGENT_WORKDIR ?? process.cwd(),
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

The same code runs in both environments. `cwd` defaults to `process.cwd()` locally, or can be set via `AGENT_WORKDIR` env var (the snapshot sets this to `/workspace`). The elasticity `curl` commands will 404 locally (no metadata service) but the build will just work if your machine has enough RAM.

No conditional logic. The agent doesn't know or care where it's running.

### Deployment (`scripts/deploy.ts`)

The agent owns its own packaging. `deploy.ts` builds a snapshot that includes the full runtime environment (Rust, Node.js, gh) and the agent code. api/ references this snapshot by name — it never touches agent files.

```typescript
import { Image } from "@opencomputer/sdk/dist/image.js";
import { Snapshots } from "@opencomputer/sdk/dist/snapshot.js";

const apiKey = process.env.OPENCOMPUTER_API_KEY!;
const apiUrl = process.env.OPENCOMPUTER_API_URL!;

const SNAPSHOT_NAME = "rust-agent";

const image = Image.base()
  // Rust toolchain
  .runCommands(
    'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y',
  )
  // Node.js 22
  .runCommands(
    "curl -fsSL https://deb.nodesource.com/setup_22.x | bash -",
    "apt-get install -y nodejs",
  )
  // gh CLI
  .aptInstall(["gh"])
  // Agent source (explicit files — avoids shipping local node_modules)
  .addLocalFile("package.json", "/workspace/agent/package.json")
  .addLocalFile("package-lock.json", "/workspace/agent/package-lock.json")
  .addLocalFile("tsconfig.json", "/workspace/agent/tsconfig.json")
  .addLocalFile("prompt.md", "/workspace/agent/prompt.md")
  .addLocalFile("src/index.ts", "/workspace/agent/src/index.ts")
  .runCommands("cd /workspace/agent && npm ci && npm run build")
  // Environment
  .env({
    PATH: "/root/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
    RUST_BACKTRACE: "1",
    AGENT_WORKDIR: "/workspace",
  })
  .workdir("/workspace");

const snapshots = new Snapshots({ apiKey, apiUrl });

// Delete existing snapshot if present, then create fresh
try { await snapshots.delete(SNAPSHOT_NAME); } catch { /* doesn't exist yet */ }

await snapshots.create({
  name: SNAPSHOT_NAME,
  image,
  onBuildLogs: (log) => console.log(log),
});

console.log(`Snapshot '${SNAPSHOT_NAME}' deployed.`);
```

Run from the agent directory:
```bash
cd agent
OPENCOMPUTER_API_KEY=... OPENCOMPUTER_API_URL=... npx tsx scripts/deploy.ts
```

**Snapshot contents** (`rust-agent`):

| Layer | What |
|-------|------|
| OC base image | Ubuntu, build-essential, git, curl, libssl-dev, pkg-config, python3 |
| Rust | `rustup` + stable toolchain |
| Node.js 22 | Via nodesource |
| gh CLI | Via apt |
| Agent | `/workspace/agent/` — source, node_modules, built dist |

**Name resolution**: `Sandbox.create({ snapshot: "rust-agent" })` resolves by name. Deploy script deletes and recreates with the same name — api/ always gets the latest without config change.

**When to redeploy**: Any change to agent source, prompt, or deps. This is the agent's CI/CD step — equivalent to `docker build && docker push`.

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
GITHUB_TOKEN=               # PAT with repo scope — for posting comments from api/ itself
GITHUB_WEBHOOK_SECRET=      # Shared secret for webhook HMAC verification
PORT=3000                   # Server port (default 3000)
```

`ANTHROPIC_API_KEY` and the sandbox's `GITHUB_TOKEN` are **not** here — they live in the OC SecretStore and are injected into the sandbox automatically. api/ only needs its own `GITHUB_TOKEN` for posting comments on behalf of the webhook handler.

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

Creates sandbox from the agent's snapshot, runs it, waits for exit. No file syncing, no knowledge of agent internals.

```typescript
import { Sandbox } from "@opencomputer/sdk";
import { postComment } from "./github";

interface RunContext {
  repo: string;
  issueNumber: number;
}

export async function runAgent(ctx: RunContext): Promise<void> {
  // 1. Create sandbox — code is in snapshot, secrets come from SecretStore
  const sandbox = await Sandbox.create({
    snapshot: "rust-agent",
    secretStore: "rust-agent",
    timeout: 1800,
    memoryMB: 2048,
    envs: {
      CARGO_BUILD_JOBS: "1",  // non-secret config only
    },
  });

  console.log(`Sandbox ${sandbox.sandboxId} created for #${ctx.issueNumber}`);

  try {
    // 2. Run agent
    const session = await sandbox.exec.start(
      "node",
      {
        args: [
          "/workspace/agent/dist/index.js",
          "--repo", ctx.repo,
          "--issue", String(ctx.issueNumber),
        ],
        cwd: "/workspace",
        timeout: 1500,
        onStdout: (data) => process.stdout.write(data),
        onStderr: (data) => process.stderr.write(data),
      },
    );

    // 3. Wait for exit
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

api/ knows four things about the agent: the snapshot name, the secret store name, the entry point path, and the CLI args. That's it. No secret values, no agent source files.

### Concurrency

For the demo, one agent at a time is fine. Each webhook spawns an independent sandbox — no coordination needed.

### Deployment

- **Dev**: Run locally + ngrok/cloudflare tunnel for webhook delivery
- **Demo**: Fly.io (single machine, same pattern as agents-control)

---

## Setup & Operations

### Bootstrap (once)

Infrastructure that exists before anything runs. Do these once, update when keys rotate or infra changes.

**OC SecretStore** — holds secrets for the agent sandbox:

```typescript
import { SecretStore } from "@opencomputer/sdk";

const opts = { apiKey: "...", apiUrl: "..." };

const store = await SecretStore.create({ name: "rust-agent", ...opts });

await SecretStore.setSecret(store.id, "ANTHROPIC_API_KEY", "sk-ant-...", {
  allowedHosts: ["api.anthropic.com"],
  ...opts,
});

await SecretStore.setSecret(store.id, "GITHUB_TOKEN", "ghp_...", {
  allowedHosts: ["github.com", "api.github.com"],
  ...opts,
});
```

Can be a small script (`scripts/setup-secrets.ts`) or done via OC dashboard if one exists. Secrets are injected as env vars into any sandbox created with `secretStore: "rust-agent"`. The `allowedHosts` field provides egress control.

### Deploy (on agent code change)

```bash
cd agent
npm install                  # only needed once locally (for tsx + @opencomputer/sdk)
npx tsx scripts/deploy.ts    # rebuilds snapshot "rust-agent"
```

The deploy script uploads source files and runs `npm ci && npm run build` inside the snapshot — the build happens there, not locally. Agent code change → redeploy snapshot → next sandbox picks it up.

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

Run the agent on your machine against a real GitHub repo + issue:

```bash
cd agent
ANTHROPIC_API_KEY=... GITHUB_TOKEN=... npx tsx src/index.ts --repo demo-org/ingest-rs --issue 1
```

The agent will clone, investigate, fix, build, test, and PR — same as in a sandbox. Elasticity `curl` commands will 404 (no metadata service) but that's fine — your machine has enough RAM so the build won't OOM. This tests agent logic, prompt quality, and the full workflow without any OC dependency.

#### 2. Agent in sandbox (no webhook, no elasticity)

Test the sandbox deployment path without waiting for the elasticity API. Use a small script (`api/scripts/test-sandbox.ts`) that calls `runAgent()` directly:

```typescript
import { runAgent } from "../src/sandbox";

await runAgent({
  repo: "demo-org/ingest-rs",
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
0:02  api/ → Sandbox.create({ snapshot: "rust-agent", secretStore: "rust-agent", memoryMB: 2048 })
0:03  api/ → sandbox.exec.start("node /workspace/agent/dist/index.js --repo ... --issue 42")

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
│       └── deploy.ts              # Builds snapshot: Rust + Node + gh + agent code
│
└── api/                           # Webhook handler + sandbox launcher
    ├── package.json               # hono, @opencomputer/sdk
    ├── tsconfig.json
    ├── .env.example
    └── src/
        ├── index.ts               # Hono app, server
        ├── webhook.ts             # Webhook handler
        ├── sandbox.ts             # Sandbox.create() + exec.start()
        └── github.ts              # Signature verification, comment posting
```

---

## Implementation Notes

Things that aren't obvious from the code sketches alone:

**OC SDK reference**: The OpenComputer TypeScript SDK source is at `../opencomputer/sdks/typescript/src/`. Read this for exact method signatures — the code sketches in this doc are accurate but not exhaustive. Key files: `sandbox.ts`, `exec.ts`, `filesystem.ts`, `image.ts` (Node.js-only import from `@opencomputer/sdk/dist/image.js`), `snapshot.ts` (Node.js-only import from `@opencomputer/sdk/dist/snapshot.js`).

**Do NOT use `addLocalDir` in deploy.ts**: The deploy script uses explicit `addLocalFile()` calls for a reason. `addLocalDir(".")` recursively base64-encodes every file into the snapshot manifest — including `node_modules/` — which would be enormous and broken. The snapshot's `npm ci` installs deps cleanly from the uploaded `package.json` + `package-lock.json`.

**ingest-rs lives here, deployed separately**: `ingest-rs/` source is developed in this repo. For the demo, it gets pushed to a separate GitHub repo (e.g. `demo-org/ingest-rs`) where the demo issue lives. The agent inside the sandbox clones it from GitHub via `gh repo clone`. Changes to `ingest-rs/` here need to be pushed to the demo repo separately.

**Agent SDK `query()` returns an async generator**: The stream yields `SDKMessage` objects. The code sketch handles `"assistant"` and `"result"` types. For the full type union, check the SDK types at `../base360-checkin-agent/agent/node_modules/@anthropic-ai/claude-agent-sdk/sdk.d.ts` or the SDK source.

---

## Open Questions

- **Calibration**: Need to empirically verify the memory profile by building `ingest-rs` under constrained memory. Number of source event structs is the tuning lever.
- **OOM detection reliability**: Exit 137 is clear. `rustc` LLVM errors may look different. System prompt covers both patterns but needs testing.
- **Real repo vs. demo org**: Real public repo is more convincing but needs cleanup between demo runs. Dedicated demo org is safer.
- **Snapshot lifecycle**: How long do OC snapshots persist? Does `snapshots.create()` with an existing name overwrite, or do we need delete+create? Current deploy script does delete+create to be safe. If OC adds tag/version support, we can use that instead.

## Resolved

- **Agent is real code**: Standalone Node.js program using `query()` from `@anthropic-ai/claude-agent-sdk`. Not prompt-only via `sandbox.agent.start()`. Runs locally or in sandbox. See AGENTS.md for reasoning.
- **No database in ingest-rs**: Pipeline writes JSON to stdout/response. No sqlx, no Postgres, no migrations. Monomorphization pressure comes from the generic pipeline across ~20 event types, not from DB code.
- **Agent owns deployment**: `agent/scripts/deploy.ts` builds the snapshot. api/ just references `snapshot: "rust-agent"`. Clean separation — api/ has zero knowledge of agent internals.
- **Status reporting**: Agent posts its own GitHub comments via `gh` CLI. api/ only posts initial ack and failure fallback.
- **Framework**: Hono + @octokit/rest + @octokit/webhooks for GitHub interaction.
- **Elasticity API contract**: Per `elasticity.md`. Not yet implemented in OC — demo assumes it ships. See Prerequisites.
