import "dotenv/config";
import { Sandbox } from "@opencomputer/sdk";

const CHECKPOINT_ID = process.env.CHECKPOINT_ID!;

const sb = await Sandbox.createFromCheckpoint(CHECKPOINT_ID, {
  apiKey: process.env.OPENCOMPUTER_API_KEY,
  apiUrl: process.env.OPENCOMPUTER_API_URL,
  timeout: 300,
});
console.log("Sandbox:", sb.sandboxId);

// Run agent directly with all output captured
const r = await sb.exec.run(
  [
    "export RUSTUP_HOME=/workspace/.rustup",
    "export CARGO_HOME=/workspace/.cargo",
    "export PATH=/workspace/.cargo/bin:$PATH",
    `export ANTHROPIC_API_KEY="${process.env.ANTHROPIC_API_KEY}"`,
    `export GITHUB_TOKEN="${process.env.GITHUB_TOKEN}"`,
    "export AGENT_WORKDIR=/workspace",
    "export CARGO_BUILD_JOBS=1",
    "cd /workspace/agent",
    "node dist/index.js --repo diggerhq/demo-elasticity --issue 1 2>&1",
  ].join(" && "),
  { timeout: 240 }
);

console.log("Exit:", r.exitCode);
console.log("=== Output (last 3000 chars) ===");
console.log(r.stdout.slice(-3000));
if (r.stderr) {
  console.error("=== Stderr ===");
  console.error(r.stderr.slice(-1000));
}

await sb.kill();
