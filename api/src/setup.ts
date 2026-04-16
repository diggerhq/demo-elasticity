/**
 * One-time setup: registers the issue-resolver agent and its secrets
 * with the platform API.
 *
 * Usage: npx tsx src/setup.ts
 */
import "dotenv/config";

const PLATFORM_API = process.env.PLATFORM_API_URL ?? "https://api.opencomputer.dev";
const API_KEY = process.env.OPENCOMPUTER_API_KEY ?? "";

const headers = {
  "Content-Type": "application/json",
  "X-API-Key": API_KEY,
};

async function main() {
  // 1. Register the agent
  console.log("Creating issue-resolver agent...");
  const agentRes = await fetch(`${PLATFORM_API}/v1/agents`, {
    method: "POST",
    headers,
    body: JSON.stringify({
      id: "issue-resolver",
      display_name: "Issue Resolver",
      config: {
        snapshot: process.env.SNAPSHOT_ID ?? "rust-agent-v1",
        entrypoint: "node /workspace/agent/dist/index.js",
        env: {
          CARGO_BUILD_JOBS: "1",
          RUSTUP_HOME: "/workspace/.rustup",
          CARGO_HOME: "/workspace/.cargo",
          PATH: "/workspace/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
          RUST_BACKTRACE: "1",
          AGENT_WORKDIR: "/workspace",
        },
        elasticity: {
          baseline_mb: 2048,
          max_mb: 8192,
        },
        idle_timeout_s: 1800,
      },
    }),
  });

  if (agentRes.ok) {
    console.log("Agent created:", await agentRes.json());
  } else if (agentRes.status === 409) {
    console.log("Agent already exists, skipping.");
  } else {
    console.error("Failed to create agent:", agentRes.status, await agentRes.text());
    process.exit(1);
  }

  // 2. Set secrets
  const secrets: Array<{ key: string; value: string; allowed_hosts: string[] }> = [
    {
      key: "ANTHROPIC_API_KEY",
      value: process.env.ANTHROPIC_API_KEY ?? "",
      allowed_hosts: ["api.anthropic.com"],
    },
    {
      key: "GITHUB_TOKEN",
      value: process.env.GITHUB_TOKEN ?? "",
      allowed_hosts: ["api.github.com"],
    },
  ];

  for (const secret of secrets) {
    if (!secret.value) {
      console.warn(`Skipping ${secret.key} — not set in env`);
      continue;
    }
    console.log(`Setting secret ${secret.key}...`);
    const res = await fetch(
      `${PLATFORM_API}/v1/agents/issue-resolver/secrets/${secret.key}`,
      {
        method: "PUT",
        headers,
        body: JSON.stringify({ value: secret.value, allowed_hosts: secret.allowed_hosts }),
      },
    );
    if (res.ok || res.status === 204) {
      console.log(`  ${secret.key} set.`);
    } else {
      console.error(`  Failed: ${res.status} ${await res.text()}`);
    }
  }

  console.log("Done.");
}

main();
