import "dotenv/config";
import { Sandbox } from "@opencomputer/sdk";
import { readFileSync } from "node:fs";

const SNAPSHOT_NAME = "rust-agent";

console.log("Creating base sandbox...");
const sandbox = await Sandbox.create({
  timeout: 3600,
  apiKey: process.env.OPENCOMPUTER_API_KEY,
  apiUrl: process.env.OPENCOMPUTER_API_URL,
});
console.log(`Sandbox: ${sandbox.sandboxId}`);

async function run(cmd: string, label: string, allowFail = false) {
  console.log(`\n=== ${label} ===`);
  console.log(`> ${cmd}`);
  const result = await sandbox.exec.run(cmd, { timeout: 600 });
  if (result.stdout) console.log(result.stdout.slice(-1000));
  if (result.stderr) console.error(result.stderr.slice(-1000));
  if (result.exitCode !== 0 && !allowFail) {
    throw new Error(`${label} failed with exit code ${result.exitCode}`);
  }
  console.log(`✓ ${label} done (exit ${result.exitCode})`);
}

// Step 1: Rust toolchain — install into /workspace (rootfs is only 1.7GB, /workspace has 20GB)
await run(
  'export RUSTUP_HOME=/workspace/.rustup CARGO_HOME=/workspace/.cargo && curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path',
  "Install Rust"
);
await run(
  'export RUSTUP_HOME=/workspace/.rustup CARGO_HOME=/workspace/.cargo PATH=/workspace/.cargo/bin:$PATH && rustc --version && cargo --version',
  "Verify Rust"
);

// Step 2: Node.js already present in base (v20), skip

// Step 3: gh CLI
await run(
  'type gh >/dev/null 2>&1 || (curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | sudo dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg && echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | sudo tee /etc/apt/sources.list.d/github-cli.list && sudo apt-get update && sudo apt-get install -y gh)',
  "Install gh CLI"
);

// Step 4: Claude Code CLI (required by @anthropic-ai/claude-agent-sdk)
await run(
  "sudo npm install -g @anthropic-ai/claude-code",
  "Install Claude Code CLI"
);

// Step 5: Upload agent source files
console.log("\n=== Upload agent files ===");
await sandbox.files.makeDir("/workspace/agent/src");
await sandbox.files.write("/workspace/agent/package.json", readFileSync("package.json", "utf-8"));
await sandbox.files.write("/workspace/agent/package-lock.json", readFileSync("package-lock.json", "utf-8"));
await sandbox.files.write("/workspace/agent/tsconfig.json", readFileSync("tsconfig.json", "utf-8"));
await sandbox.files.write("/workspace/agent/prompt.md", readFileSync("prompt.md", "utf-8"));
await sandbox.files.write("/workspace/agent/src/index.ts", readFileSync("src/index.ts", "utf-8"));
console.log("✓ Files uploaded");

// Verify upload
await run(
  "ls -la /workspace/agent/ && cat /workspace/agent/package.json | head -5",
  "Verify upload"
);

// Step 6: Install agent deps and build (chown because files.write creates as root)
await run(
  "sudo chown -R sandbox:sandbox /workspace/agent && export RUSTUP_HOME=/workspace/.rustup CARGO_HOME=/workspace/.cargo PATH=/workspace/.cargo/bin:$PATH && cd /workspace/agent && npm install && npm run build",
  "Install agent deps + build"
);

// Step 7: Set environment (non-fatal — env vars are passed via exec.start anyway)
await run(
  'echo \'export RUSTUP_HOME=/workspace/.rustup CARGO_HOME=/workspace/.cargo PATH=/workspace/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin\' | sudo tee -a /root/.bashrc > /dev/null',
  "Set PATH", true
);
await run(
  'echo \'export RUST_BACKTRACE=1\' | sudo tee -a /root/.bashrc > /dev/null && echo \'export AGENT_WORKDIR=/workspace\' | sudo tee -a /root/.bashrc > /dev/null',
  "Set env vars", true
);

// Step 8: Checkpoint — wait for it to be ready before killing sandbox
console.log("\n=== Creating checkpoint ===");
const checkpoint = await sandbox.createCheckpoint(SNAPSHOT_NAME);
console.log(`Checkpoint created: ${checkpoint.id} (status: ${checkpoint.status})`);

// Poll until checkpoint is ready
if (checkpoint.status !== "ready") {
  console.log("Waiting for checkpoint to be ready...");
  const checkpoints = await sandbox.listCheckpoints();
  const check = checkpoints.find((c: any) => c.id === checkpoint.id);
  console.log(`Current status: ${check?.status ?? "unknown"}`);

  // Wait and poll
  for (let i = 0; i < 60; i++) {
    await new Promise(r => setTimeout(r, 5000));
    const list = await sandbox.listCheckpoints();
    const cp = list.find((c: any) => c.id === checkpoint.id);
    const status = cp?.status ?? "unknown";
    process.stdout.write(`  ${status}...`);
    if (status === "ready") {
      console.log(" ✓");
      break;
    }
    if (status === "failed") {
      console.log(" ✗");
      throw new Error("Checkpoint failed");
    }
  }
}

// Cleanup
await sandbox.kill();
console.log(`\nDone. Snapshot '${SNAPSHOT_NAME}' is ready (checkpoint: ${checkpoint.id}).`);
