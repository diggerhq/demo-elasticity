# demo-elasticity

Demo scenario for OpenComputer's upcoming **elasticity** feature — the ability for an agent running inside a sandbox to dynamically request more (or fewer) compute resources without reprovisioning.

## What This Demonstrates

A ticket-resolving agent watches a GitHub repo. When someone comments `@myagent` on an issue, the agent picks it up, investigates, resolves it inside an OpenComputer sandbox, and submits a PR. The target repo is a Rust project — compilation is the bottleneck that triggers elastic scaling. The agent detects an OOM during `cargo build`, requests a burst of additional memory from within the sandbox, retries, and scales back down after compilation succeeds.

**The pitch**: "I could set my sandbox to 16 GB permanently, but that's expensive for an agent that only needs burst memory for 2 minutes of compilation. With elasticity, the agent requests what it needs, when it needs it."

## Components

### 1. `rust-app/` — Target Rust Project

A realistic Rust application that genuinely requires significant memory to compile but is otherwise simple. Lives in its own directory (or separate repo) — this is what the agent checks out and works on when resolving issues.

Requirements:
- Compilation must fail at 2 GB RAM and succeed at ≥8 GB
- The app itself is straightforward (not artificially bloated)
- Should look like a real project someone would maintain

### 2. `agent/` — Issue-Resolver Agent (Claude Agent SDK)

Runs inside an OpenComputer sandbox. Uses Claude Agent SDK with tools for:
- Reading GitHub issues and comments
- Cloning repos, editing code, running builds
- **Requesting resource changes** via the elasticity API (the new thing)
- Creating branches and submitting PRs

The agent's system prompt encodes the workflow: triage → investigate → fix → build → test → PR.

### 3. `api/` — Event Handler / Orchestration

Reacts to GitHub webhook events (issue comment created), filters for `@myagent` mentions, and spins up the agent in an OpenComputer sandbox. Manages sandbox lifecycle, passes context to the agent, and reports status back to the issue.

This is the "agent app" layer — later, patterns here may be extracted as experimental OpenComputer APIs.

## Elasticity API

The elasticity API is **not yet public** in OpenComputer. This demo exercises it. Two surfaces — external (control plane) and internal (from inside the VM via instance metadata).

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
