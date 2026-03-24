# demo-elasticity

Demo for OpenComputer's upcoming **elasticity** feature — live resource scaling for sandboxes without reprovisioning.

An agent resolves GitHub issues in an OpenComputer sandbox. The target project is a Rust data ingestion service (`ingest-rs`) whose compilation requires more memory than the sandbox starts with. Instead of over-provisioning the sandbox for the entire session, the agent requests a burst of memory from inside the VM, compiles, and scales back down. You pay for 8 GB for 2 minutes of compilation instead of the full 15-minute session.

## Components

### 1. `ingest-rs/` — Data Ingestion Service (Rust)

An HTTP service that receives events from webhooks, CSV uploads, and streaming sources, normalizes them through a typed transform pipeline, and writes to a database. Standard Rust stack: axum, serde, sqlx, tokio.

The compilation is memory-intensive because the generic transform pipeline is monomorphized across many concrete event types — each source has its own struct, and each goes through the same generic layers, which forces `rustc`/LLVM to generate and optimize a large amount of IR. This is normal for Rust projects with broad type surface; nothing is artificially inflated.

Memory profile (clean build, `CARGO_BUILD_JOBS=1`):
- 2 GB → OOM killed
- 4 GB → gray zone
- 8 GB → succeeds comfortably

### 2. `agent/` — Issue-Resolver Agent (Claude Agent SDK)

Runs inside an OpenComputer sandbox. Picks up GitHub issues, clones the repo, investigates, makes a fix, builds, tests, and submits a PR.

The agent knows how to use the elasticity API via its system prompt: when a build fails with OOM (exit 137, "killed", LLVM allocation errors), it checks current limits at `http://169.254.169.254/v1/limits`, scales up via `POST /v1/scale`, retries the build, then scales back down.

### 3. `api/` — Event Handler / Orchestration

Receives GitHub webhooks (`issue_comment.created`), filters for `@myagent` mentions, creates an OpenComputer sandbox (starting at 2 GB — deliberately undersized for compilation), and starts the agent with the issue context. Posts status updates back to the issue thread.

This is the "agent app" layer. Patterns that emerge here may later be extracted as experimental OpenComputer APIs.

## Elasticity API

The elasticity API is **not yet implemented** in OpenComputer. `elasticity.md` in this repo describes the assumed contract — both the internal metadata service (`169.254.169.254/v1/scale`) and the external control plane endpoint (`PUT /api/sandboxes/:id/limits`). The demo is built against that spec. If the API ships differently, only the agent prompt and `elasticity.md` need updating.

Two surfaces — external (control plane) and internal (from inside the VM via instance metadata).

### External API (from your backend)

```
PUT /api/sandboxes/:id/limits
{"memoryMB": 8192}
```

Header: `X-API-Key`. Memory change is live — no reboot. CPU auto-scales with memory (1 vCPU per 1 GB).

### Internal API (from inside the sandbox)

Instance metadata service at `169.254.169.254`:

```bash
# Scale memory
curl -s -X POST http://169.254.169.254/v1/scale \
  -H "Content-Type: application/json" \
  -d '{"memoryMB": 8192}'
# → {"ok": true, "memoryMB": 8192}

# Query current limits
curl -s http://169.254.169.254/v1/limits
# → {"memLimit": ..., ...}

# Query sandbox status
curl -s http://169.254.169.254/v1/status
# → {"sandboxId": "...", ...}

# Query metadata (region, etc.)
curl -s http://169.254.169.254/v1/metadata
# → {"region": "...", ...}
```

### Semantics

- Resize is **live** — no reboot, no checkpoint-restore cycle
- CPU auto-scales with memory (1 vCPU per 1 GB)
- Memory can grow and shrink
- Upper bounds enforced by org quotas / plan limits
- The sandbox remains at the new size until explicitly changed or killed
- Billing is per-second at the current resource level

### Reference

See `elasticity.md` for the full test script exercising both APIs.

## Process

- **`.agents-wip/`** — Design docs and implementation plans in progress. Iterate here before building.
- **`.agents-done/`** — Completed design docs moved here after implementation ships.
- **`AGENTS.md`** (this file) — Stable high-level reference. Update when the project shape changes, not for transient planning.
