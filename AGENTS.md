# demo-elasticity

Demo for OpenComputer's upcoming **elasticity** feature — live resource scaling for sandboxes without reprovisioning.

An agent resolves GitHub issues in an OpenComputer sandbox. The target project is a Rust data ingestion service (`ingest-rs`) whose compilation requires more memory than the sandbox starts with. Instead of over-provisioning the sandbox for the entire session, the agent requests a burst of memory from inside the VM, compiles, and scales back down. You pay for 8 GB for 2 minutes of compilation instead of the full 15-minute session.

## Components

### 1. `ingest-rs/` — Data Ingestion Service (Rust)

An HTTP service that receives events from webhooks, CSV uploads, and streaming sources, normalizes them through a typed generic transform pipeline, and emits JSON. Standard Rust stack: axum, serde, tokio. No database — the pipeline validates, normalizes, and serializes. The monomorphization pressure comes from ~20 event types × 6 generic pipeline stages, not from storage code.

The compilation is memory-intensive because the generic transform pipeline is monomorphized across many concrete event types — each source has its own struct, and each goes through the same generic layers, which forces `rustc`/LLVM to generate and optimize a large amount of IR. This is normal for Rust projects with broad type surface; nothing is artificially inflated.

Memory profile (clean build, `CARGO_BUILD_JOBS=1`):
- 2 GB → OOM killed
- 4 GB → gray zone
- 8 GB → succeeds comfortably

### 2. `agent/` — Issue-Resolver Agent (Claude Agent SDK)

A standalone Node.js application using `@anthropic-ai/claude-agent-sdk`. Clones a repo, resolves a GitHub issue, builds, tests, and opens a PR. Can run locally or inside an OpenComputer sandbox — the sandbox is just compute, the agent is a real program.

The agent uses the elasticity API from inside the sandbox: when a build OOMs, it detects the failure (exit 137, LLVM allocation errors), queries current limits, scales up via the metadata service, retries, and scales back down.

**Why real code, not prompt-only**: OpenComputer's `sandbox.agent.start()` accepts a system prompt and runs a managed agent loop inside the VM. We deliberately don't use that here. The demo should show the pattern real users follow: build an agent as code using the SDK, then deploy it to infrastructure. Baking the agent framework into the deployment platform and having users ship prompts instead of applications conflates two concerns — it's like if a PaaS required you to use its built-in web framework instead of bringing your own. The agent should be a portable program that happens to run in a sandbox.

### 3. `api/` — Event Handler / Orchestration

Receives GitHub webhooks (`issue_comment.created`), filters for `@myagent` mentions, creates an OpenComputer sandbox from the agent's pre-built snapshot (starting at 2 GB — deliberately undersized for compilation), and runs the agent. Posts status updates back to the issue thread. Has zero knowledge of agent internals — just a checkpoint ID, entry point path, and CLI args.

## Elasticity API

The elasticity API is **implemented and deployed** on the `feat/qemu-backend-azure` branch of OpenComputer (the branch running at `app.opencomputer.dev`). Both the internal metadata service (`169.254.169.254/v1/scale`) and the external endpoint (`POST /api/sandboxes/:id/scale`) are working. `elasticity.md` in this repo documents the full API surface.

Two surfaces — external (control plane) and internal (from inside the VM via instance metadata).

### External API (from your backend)

```
POST /api/sandboxes/:id/scale
{"memoryMB": 8192}
```

Header: `X-API-Key`. Memory change is live — no reboot. CPU auto-scales with memory.

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

## Conventions

- **Happy paths only**: This is a demo project. Don't add fallbacks, backwards compatibility shims, or defensive error handling. If something fails, it fails. Keep the code simple and direct.

## Process

- **`.agents-wip/`** — Design docs and implementation plans in progress. Iterate here before building.
- **`.agents-done/`** — Completed design docs moved here after implementation ships.
- **`AGENTS.md`** (this file) — Stable high-level reference. Update when the project shape changes, not for transient planning.
