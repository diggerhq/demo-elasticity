# Implementation Design — Elasticity Demo

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

## Component 2: Agent (`agent/`)

Claude Agent SDK, runs inside an OpenComputer sandbox. Workflow: read issue → clone `ingest-rs` → investigate → fix → build → test → PR.

### System Prompt

The prompt tells the agent:
- It resolves issues for the `ingest-rs` repo
- Standard tools available: bash, file read/write/edit
- Use `gh` CLI for GitHub interaction (read issues, create PRs)
- Elasticity instructions:

> If a build fails due to insufficient memory (exit 137, "killed", LLVM allocation failure), you can scale the sandbox. Check current limits: `curl -s http://169.254.169.254/v1/limits`. Scale up: `curl -s -X POST http://169.254.169.254/v1/scale -H "Content-Type: application/json" -d '{"memoryMB": 8192}'`. After the memory-intensive step completes, scale back down to the original value.

### Elasticity Loop

```
cargo build 2>&1 → OOM (exit 137 / killed)
→ GET /v1/limits → current memoryMB
→ POST /v1/scale {"memoryMB": 8192}
→ cargo build 2>&1 → success
→ POST /v1/scale {"memoryMB": 2048} → scale back
```

The agent isn't hardcoded to do this — the system prompt teaches it the pattern. It reacts to the failure naturally.

### Structure

```
agent/
├── prompt.md              # system prompt
└── .claude/
    └── settings.json      # tool permissions
```

### Sandbox Template: `rust-agent` Snapshot

Use OpenComputer's declarative image + snapshot system. The default base template already has `build-essential`, `git`, `curl`, `libssl-dev`, `pkg-config` — everything Rust needs as system deps. We just add the Rust toolchain and `gh` CLI on top.

```typescript
import { Image, Snapshots } from '@opencomputer/sdk';

const rustAgentImage = Image.base()
  .runCommands(
    'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y',
  )
  .aptInstall(['gh'])
  .env({
    PATH: '/root/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin',
    RUST_BACKTRACE: '1',
  })
  .workdir('/workspace');

// Create once — persists org-wide, cached by content hash
const snapshots = new Snapshots({ apiKey, apiUrl });
await snapshots.create({
  name: 'rust-agent',
  image: rustAgentImage,
  onBuildLogs: (log) => console.log(log),
});
```

Then sandboxes boot instantly from the snapshot — no image build, no Rust install:

```typescript
const sandbox = await Sandbox.create({ snapshot: 'rust-agent', memoryMB: 2048 });
```

If we need to update the snapshot later (e.g. add `cargo-audit`), we can apply a patch to the checkpoint without rebuilding from scratch.

## Component 3: Event Handler / API (`api/`)

TypeScript / Node.js (Hono). Receives GitHub webhooks, spins up the agent in a sandbox.

### Flow

```
GitHub webhook (issue_comment.created)
  → POST /webhooks/github
  → verify signature
  → if body contains "@myagent":
      → comment on issue: "On it..."
      → Sandbox.create({ snapshot: "rust-agent", memoryMB: 2048, ... })
      → start agent session with issue context in prompt
      → stream agent events, post status comments
      → on completion: comment with PR link or error
      → kill sandbox
```

Sandbox starts at 2 GB deliberately. The agent hits the wall and scales up — that's the demo.

### Sandbox Config

```typescript
const sandbox = await Sandbox.create({
  snapshot: "rust-agent",
  timeout: 1800,
  memoryMB: 2048,
  cpuCount: 2,
  envs: {
    GITHUB_TOKEN: process.env.GITHUB_TOKEN,
    ANTHROPIC_API_KEY: process.env.ANTHROPIC_API_KEY,
  },
});
```

### Status Reporting

Agent posts its own status comments via `gh issue comment` — more natural, shows agent autonomy. The API layer only posts the initial "On it..." and handles failures if the agent crashes without reporting.

## Demo Narrative

1. Show `ingest-rs` — normal Rust data ingestion service
2. Issue exists: "batch response missing `processed_at`"
3. Someone comments `@myagent resolve this`
4. Agent spins up (2 GB sandbox), reads issue, clones repo, makes the fix
5. `cargo build` → OOM killed
6. Agent checks `/v1/limits` → 2048 MB, scales to 8192 via `/v1/scale`
7. `cargo build` → succeeds
8. Agent scales back to 2048 MB
9. Tests pass, PR submitted
10. Total time at 8 GB: ~2 min out of ~15 min session

## Open Questions

- **Real repo or dedicated demo org**: Real public repo is more convincing but needs cleanup between runs.
- **Calibration**: Need to empirically verify the memory profile by building `ingest-rs` under constrained memory. Number of source event structs is the tuning lever.
- **OOM detection**: Exit 137 is clear. `rustc` may also fail with LLVM errors that don't look like OOM. System prompt should cover both patterns.
- **Scale-down timing**: After `cargo build` succeeds, before tests — tests don't recompile so they're cheap.

## Resolved

- **Sandbox template**: Declarative snapshot via `Image.base().runCommands(rustup).aptInstall([gh])`. Default base already has build-essential, git, curl, libssl-dev. Snapshot persists org-wide, boots instantly. Patches available for post-creation updates.
