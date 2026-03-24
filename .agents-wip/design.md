# Implementation Design — Elasticity Demo

## Overview

Three components that together tell the story: "my agent hit a memory wall compiling Rust — instead of over-provisioning the sandbox permanently, it scaled up for the burst and scaled back down."

## Component 1: Rust App (`rust-app/`)

### Goal

A legitimate Rust project that **genuinely** needs ≥8 GB RAM to compile from clean but is otherwise a normal, small project. Not artificially bloated — it should look like something a team actually ships.

### Approach: Heavy Generics + Derive + Moderate Dependency Tree

The memory pressure in `rustc` comes from monomorphization and LLVM codegen. A realistic way to trigger it:

- **A web service** using `axum` + `serde` + `sqlx` (typed queries) + `tokio` — standard stack, nothing exotic
- **Heavily generic data pipeline module** — a few layers of generic transforms over generic types, each with `Serialize`/`Deserialize` derives. 4-5 nested generics with trait bounds that force monomorphization across many concrete types
- **A batch of concrete instantiations** in a single translation unit — e.g., a `types.rs` that defines 15-20 domain structs, each threaded through the generic pipeline. This is where `rustc` blows up: each concrete type × each generic layer × LLVM IR
- **Proc macro usage** via `sqlx::FromRow`, `serde`, `clap` — real-world derives that expand code

### Why This Works

- `rustc` allocates per-codegen-unit; heavy monomorphization in a single crate keeps everything in one process
- LLVM's optimization passes on large IR are the actual memory hog
- 15-20 structs through 4-5 generic layers is enough to push past 4 GB without being absurd
- The resulting binary is a normal web service

### Structure

```
rust-app/
├── Cargo.toml
├── src/
│   ├── main.rs          # axum server, routes
│   ├── types.rs         # 15-20 domain structs (the "realistic breadth")
│   ├── pipeline.rs      # generic transform pipeline (the memory multiplier)
│   ├── handlers.rs      # HTTP handlers that use pipeline with concrete types
│   └── db.rs            # sqlx queries (typed)
├── migrations/          # sqlx migrations (even if trivial)
└── README.md
```

### Calibration

Need to verify the memory profile empirically:
- Build in a 2 GB sandbox → should OOM or get killed
- Build in a 4 GB sandbox → may or may not succeed (gray zone is fine)
- Build in an 8 GB sandbox → should succeed comfortably
- Build in a 16 GB sandbox → fast, no pressure

Use `CARGO_BUILD_JOBS=1` to keep it single-threaded (predictable memory, not CPU-bound). The demo is about memory, not parallelism.

### The Issue

The GitHub issue for the demo should be something like: "API response for batch endpoint is missing the `updated_at` field" — a simple fix (add a field to one struct, update the handler) but the agent still has to compile the project to verify, which is where the memory wall hits.

## Component 2: Agent (`agent/`)

### Goal

Claude Agent SDK agent running inside an OpenComputer sandbox. Resolves GitHub issues by:
1. Understanding the issue
2. Cloning the repo, reading code
3. Making the fix
4. Building to verify → hits OOM → scales up → retries → succeeds → scales down
5. Running tests
6. Creating a branch and PR

### System Prompt Design

The agent needs to know:
- It's a ticket-resolver for a specific GitHub repo
- It has access to standard tools (bash, file read/write/edit, etc.)
- It can query sandbox resources via `curl http://169.254.169.254/v1/limits`
- **When a build OOMs**, it should scale up via `curl -X POST http://169.254.169.254/v1/scale -d '{"memoryMB": N}'` and retry
- After the memory-intensive step completes, it should scale back down
- It should create PRs via `gh` CLI

### Key Behavior: The Elasticity Loop

```
1. cargo build 2>&1
2. detect OOM (exit code 137 / "out of memory" / "killed" in output)
3. curl http://169.254.169.254/v1/limits → see current memoryMB
4. curl -X POST http://169.254.169.254/v1/scale -d '{"memoryMB": <current * 2 or 8192>}'
5. verify via /v1/limits
6. cargo build 2>&1 → succeeds
7. curl -X POST http://169.254.169.254/v1/scale -d '{"memoryMB": 2048}' → scale back
```

The agent discovers this workflow through its system prompt guidance, not hardcoded logic. The prompt should say something like:

> If a build fails due to insufficient memory (OOM, killed, exit 137), you can request more memory from the sandbox runtime. Check current limits at `http://169.254.169.254/v1/limits` and scale via `POST http://169.254.169.254/v1/scale` with `{"memoryMB": N}`. Scale back down after the memory-intensive step completes.

### Agent Configuration

```
agent/
├── prompt.md              # system prompt
└── .claude/
    └── settings.json      # tool permissions, MCP config if needed
```

### Tools Needed Inside Sandbox

- `bash` (compile, git, gh, curl)
- `file read/write/edit` (code changes)
- `gh` CLI (create PRs, read issues)
- `git` (clone, branch, commit, push)
- `curl` (elasticity API)

The sandbox template needs: Rust toolchain, `gh` CLI, git, curl. Either baked into a custom template or installed at agent boot.

## Component 3: Event Handler / API (`api/`)

### Goal

Lightweight service that:
1. Receives GitHub webhook (`issue_comment` event)
2. Filters for `@myagent` mentions
3. Creates an OpenComputer sandbox
4. Loads and starts the agent with the issue context
5. Posts status updates back to the GitHub issue

### Stack

TypeScript / Node.js (Hono or Express). Runs on Fly.io or anywhere.

### Flow

```
GitHub webhook (issue_comment.created)
  → POST /webhooks/github
  → verify signature
  → extract issue number, comment body, repo
  → if body contains "@myagent":
      → POST comment: "🤖 On it — spinning up a sandbox..."
      → create OpenComputer sandbox (template with Rust + gh)
      → start agent session with prompt including issue context
      → poll / stream agent events
      → on completion: POST comment with PR link or error summary
      → kill sandbox
```

### Sandbox Configuration

```typescript
const sandbox = await Sandbox.create({
  template: "rust-agent",       // custom template with rust + gh + git
  timeout: 1800,                // 30 min max
  memoryMB: 2048,               // start small — the agent scales up if needed
  cpuCount: 2,
  envs: {
    GITHUB_TOKEN: process.env.GITHUB_TOKEN,
    ANTHROPIC_API_KEY: process.env.ANTHROPIC_API_KEY,
  },
});
```

Starting at 2 GB is the point — it's deliberately undersized for compilation. The agent discovers the constraint and uses elasticity to overcome it.

### Status Reporting

Post GitHub issue comments at key milestones:
- "Investigating..." (agent started)
- "Found the issue, working on a fix..." (agent identified the problem)
- "Build failed — requesting more memory..." (elasticity moment!)
- "Build succeeded, creating PR..." (post-scale)
- "PR created: #123" or "Failed: <reason>"

How much of this to automate vs. let the agent do directly (via `gh issue comment`) is a design choice. The agent could do all the commenting itself if it has `gh` access.

## Demo Script / Narrative

The demo walks through this scenario live or recorded:

1. **Show the repo** — normal Rust web service, nothing unusual
2. **Show the issue** — someone filed a bug, another dev comments `@myagent resolve this`
3. **Agent starts** — sandbox created at 2 GB (show the OpenComputer dashboard or logs)
4. **Agent investigates** — reads the issue, clones the repo, understands the codebase
5. **Agent makes the fix** — straightforward code change
6. **Agent builds** — `cargo build` → OOM killed (show the failure)
7. **Agent reacts** — checks `/v1/limits`, sees 2048 MB, scales to 8192 MB via `/v1/scale`
8. **Agent retries** — `cargo build` → succeeds
9. **Agent scales down** — back to 2048 MB (show the cost-awareness)
10. **Agent runs tests** — pass
11. **Agent submits PR** — done
12. **Contrast** — "without elasticity, you'd pay for 8 GB for the entire 15-minute session; with elasticity, you paid for 8 GB for only the 2 minutes of compilation"

## Open Questions

- **Sandbox template**: Build a custom `rust-agent` template with Rust toolchain + gh + git pre-installed? Or install at boot? Pre-installed is faster and more realistic for the demo.
- **Agent commenting**: Should the API layer post status comments, or should the agent itself use `gh issue comment`? Agent doing it is more natural and shows agent autonomy. API layer doing it is more reliable.
- **Real GitHub repo vs. mock**: For the demo, use a real public repo? Or a local git server? Real repo is more convincing but requires cleanup. Could use a dedicated demo org.
- **Rust app calibration**: Need to actually build the app in constrained memory to find the right number of types/generics. May need iteration.
- **Error detection**: How reliably can the agent detect OOM? Exit code 137 is clear, but `rustc` might also just print "error: could not compile" with an obscure LLVM error. The system prompt should cover both patterns.
- **Scale-down timing**: Should the agent scale down immediately after `cargo build`, or after tests too? Tests are cheap (no recompilation) so scaling down after build is correct.
