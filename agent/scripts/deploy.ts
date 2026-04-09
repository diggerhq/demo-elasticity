import "dotenv/config";
// @ts-ignore — files exist but aren't in package exports
import { Image } from "../node_modules/@opencomputer/sdk/dist/image.js";
// @ts-ignore
import { Snapshots } from "../node_modules/@opencomputer/sdk/dist/snapshot.js";

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
