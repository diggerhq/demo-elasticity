```
/**
 * OpenSandbox Scaling Example
 *
 * Demonstrates scaling via:
 *   - External API: PUT /api/sandboxes/:id/limits (from your backend)
 *   - Internal API: curl http://169.254.169.254/v1/scale (from inside the sandbox)
 *
 * Usage:
 *   OPENCOMPUTER_API_URL=http://3.148.184.81:8080 OPENCOMPUTER_API_KEY=test-dev-key npx tsx examples/test-scaling.ts
 */

import { Sandbox } from "../src/index";

const API_URL = process.env.OPENCOMPUTER_API_URL ?? "http://localhost:8080";
const API_KEY = process.env.OPENCOMPUTER_API_KEY ?? "test-key";

function green(msg: string) { console.log(`\x1b[32m✓ ${msg}\x1b[0m`); }
function red(msg: string) { console.log(`\x1b[31m✗ ${msg}\x1b[0m`); }
function bold(msg: string) { console.log(`\x1b[1m${msg}\x1b[0m`); }
function dim(msg: string) { console.log(`\x1b[2m  ${msg}\x1b[0m`); }

let passed = 0;
let failed = 0;

function check(desc: string, ok: boolean, detail?: string) {
  if (ok) { green(desc); passed++; }
  else { red(`${desc}${detail ? ` (${detail})` : ""}`); failed++; }
}

/** Read guest memory in MB via /proc/meminfo */
async function getMemoryMB(sandbox: Sandbox): Promise<number> {
  const r = await sandbox.exec.run("awk '/MemTotal/{print $2}' /proc/meminfo");
  return Math.floor(parseInt(r.stdout.trim()) / 1024);
}

/** Read cgroup cpu.max */
async function getCpuMax(sandbox: Sandbox): Promise<string> {
  const r = await sandbox.exec.run("cat /sys/fs/cgroup/sandbox/cpu.max");
  return r.stdout.trim();
}

/** External API: PUT /api/sandboxes/:id/limits */
async function scaleExternal(sandboxId: string, memoryMB: number): Promise<void> {
  const base = API_URL.replace(/\/+$/, "");
  const url = `${base}/api/sandboxes/${sandboxId}/limits`;
  const resp = await fetch(url, {
    method: "PUT",
    headers: { "Content-Type": "application/json", "X-API-Key": API_KEY },
    body: JSON.stringify({ memoryMB }),
  });
  if (!resp.ok) throw new Error(`Scale failed: ${resp.status} ${await resp.text()}`);
}

async function main() {
  bold("\n========================================");
  bold(" OpenSandbox Scaling Demo");
  bold("========================================\n");
  dim(`API: ${API_URL}`);
  console.log();

  const sandbox = await Sandbox.create({
    timeout: 3600,
    apiKey: API_KEY,
    apiUrl: API_URL,
  });
  green(`Created sandbox: ${sandbox.sandboxId}`);

  const initialMB = await getMemoryMB(sandbox);
  dim(`Initial memory: ${initialMB}MB`);
  console.log();

  // ─── External API scaling (from your backend) ───────────────────────

  bold("── External API: PUT /api/sandboxes/:id/limits ──\n");

  // Scale up to 2GB
  bold("[1] Scale up to 2GB...");
  await scaleExternal(sandbox.sandboxId, 2048);
  await new Promise(r => setTimeout(r, 1000));
  let mem = await getMemoryMB(sandbox);
  dim(`Memory: ${mem}MB`);
  check("Scale up to 2GB", mem >= 1900);

  // Scale up to 4GB
  bold("[2] Scale up to 4GB...");
  await scaleExternal(sandbox.sandboxId, 4096);
  await new Promise(r => setTimeout(r, 1000));
  mem = await getMemoryMB(sandbox);
  dim(`Memory: ${mem}MB`);
  check("Scale up to 4GB", mem >= 3900);

  // Verify CPU auto-scaled (1 vCPU per 1GB = 4 vCPU at 4GB)
  const cpuMax = await getCpuMax(sandbox);
  dim(`cpu.max: ${cpuMax}`);
  const cpuUsec = parseInt(cpuMax.split(" ")[0]);
  check("CPU auto-scaled with memory", cpuUsec >= 100000, `cpu.max=${cpuMax}`);

  // Scale down to 1GB
  bold("[3] Scale down to 1GB...");
  await scaleExternal(sandbox.sandboxId, 1024);
  await new Promise(r => setTimeout(r, 1000));
  mem = await getMemoryMB(sandbox);
  dim(`Memory: ${mem}MB`);
  check("Scale down to 1GB", mem <= 1100);

  // Scale up to 8GB and allocate memory
  bold("[4] Scale up to 8GB + allocate 6GB...");
  await scaleExternal(sandbox.sandboxId, 8192);
  await new Promise(r => setTimeout(r, 1000));
  const alloc = await sandbox.exec.run(
    "python3 -c \"import ctypes; s=6*1024**3; b=ctypes.create_string_buffer(s); print(s//1024**3)\"",
    { timeout: 15 }
  );
  dim(`Allocated: ${alloc.stdout.trim()}GB`);
  check("Can allocate 6GB in 8GB VM", alloc.stdout.trim() === "6");

  // Scale back down
  await scaleExternal(sandbox.sandboxId, 1024);
  await new Promise(r => setTimeout(r, 1000));
  console.log();

  // ─── Internal API scaling (from inside the sandbox) ─────────────────

  bold("── Internal API: curl http://169.254.169.254/v1/scale ──\n");

  // Query status
  bold("[5] Query metadata status...");
  const status = await sandbox.exec.run("curl -s http://169.254.169.254/v1/status");
  dim(`/v1/status → ${status.stdout.trim()}`);
  check("Metadata status returns sandboxId", status.stdout.includes("sandboxId"));

  // Query limits
  bold("[6] Query metadata limits...");
  const limits = await sandbox.exec.run("curl -s http://169.254.169.254/v1/limits");
  dim(`/v1/limits → ${limits.stdout.trim()}`);
  check("Metadata limits returns memLimit", limits.stdout.includes("memLimit"));

  // Scale up via internal API
  bold("[7] Scale to 2GB via internal API...");
  const scaleResp = await sandbox.exec.run(
    'curl -s -X POST http://169.254.169.254/v1/scale -H "Content-Type: application/json" -d \'{"memoryMB":2048}\'',
    { timeout: 10 }
  );
  dim(`/v1/scale → ${scaleResp.stdout.trim()}`);
  check("Internal scale returns ok", scaleResp.stdout.includes('"ok":true'));

  await new Promise(r => setTimeout(r, 1000));
  mem = await getMemoryMB(sandbox);
  dim(`Memory after internal scale: ${mem}MB`);
  check("Memory is ~2GB after internal scale", mem >= 1900);

  // Scale to 4GB via internal API
  bold("[8] Scale to 4GB via internal API...");
  await sandbox.exec.run(
    'curl -s -X POST http://169.254.169.254/v1/scale -H "Content-Type: application/json" -d \'{"memoryMB":4096}\'',
    { timeout: 10 }
  );
  await new Promise(r => setTimeout(r, 1000));
  mem = await getMemoryMB(sandbox);
  dim(`Memory: ${mem}MB`);
  check("Memory is ~4GB after internal scale", mem >= 3900);

  // Scale down via internal API
  bold("[9] Scale down to 1GB via internal API...");
  await sandbox.exec.run(
    'curl -s -X POST http://169.254.169.254/v1/scale -H "Content-Type: application/json" -d \'{"memoryMB":1024}\'',
    { timeout: 10 }
  );
  await new Promise(r => setTimeout(r, 1000));
  mem = await getMemoryMB(sandbox);
  dim(`Memory: ${mem}MB`);
  check("Memory is ~1GB after internal scale down", mem <= 1100);

  // Python scaling from inside the VM
  bold("[10] Scale from Python inside the VM...");
  const pyResult = await sandbox.exec.run(`python3 -c "
import urllib.request, json

# Scale to 8GB from inside the sandbox
req = urllib.request.Request(
    'http://169.254.169.254/v1/scale',
    data=json.dumps({'memoryMB': 8192}).encode(),
    headers={'Content-Type': 'application/json'},
    method='POST'
)
resp = json.loads(urllib.request.urlopen(req).read())
print(f'ok={resp[\"ok\"]} memoryMB={resp[\"memoryMB\"]}')
"`, { timeout: 15 });
  dim(`Python result: ${pyResult.stdout.trim()}`);
  // Python scale worked if memory is now ~8GB (stdout may be empty due to buffering)
  await new Promise(r => setTimeout(r, 1000));
  mem = await getMemoryMB(sandbox);
  dim(`Memory after Python scale: ${mem}MB`);
  check("Python scale to 8GB works", mem >= 7900);

  // Query metadata
  bold("[11] Query metadata...");
  const meta = await sandbox.exec.run("curl -s http://169.254.169.254/v1/metadata");
  dim(`/v1/metadata → ${meta.stdout.trim()}`);
  check("Metadata returns region", meta.stdout.includes("region"));

  console.log();

  // Cleanup
  await sandbox.kill();
  green("Sandbox killed.");
  console.log();

  bold("========================================");
  bold(` Results: ${passed} passed, ${failed} failed`);
  bold("========================================\n");

  if (failed > 0) process.exit(1);
}

main().catch((err) => {
  console.error("Fatal error:", err);
  process.exit(1);
});
```
