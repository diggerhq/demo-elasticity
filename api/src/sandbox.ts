import { Sandbox } from "@opencomputer/sdk";
import { postComment } from "./github";

interface RunContext {
  repo: string;
  issueNumber: number;
}

export async function runAgent(ctx: RunContext): Promise<void> {
  // 1. Create sandbox — code is in snapshot, secrets come from SecretStore
  const sandbox = await Sandbox.create({
    snapshot: "rust-agent",
    secretStore: "rust-agent",
    timeout: 1800,
    memoryMB: 2048,
    envs: {
      CARGO_BUILD_JOBS: "1",  // non-secret config only
    },
  });

  console.log(`Sandbox ${sandbox.sandboxId} created for #${ctx.issueNumber}`);

  try {
    // 2. Run agent
    const session = await sandbox.exec.start(
      "node",
      {
        args: [
          "/workspace/agent/dist/index.js",
          "--repo", ctx.repo,
          "--issue", String(ctx.issueNumber),
        ],
        cwd: "/workspace",
        timeout: 1500,
        onStdout: (data) => process.stdout.write(data),
        onStderr: (data) => process.stderr.write(data),
      },
    );

    // 3. Wait for exit
    const exitCode = await session.done;
    console.log(`Agent exited: ${exitCode}`);

    if (exitCode !== 0) {
      await postComment(ctx.repo, ctx.issueNumber,
        `❌ Agent exited with code ${exitCode}. Check logs for details.`
      );
    }
  } finally {
    await sandbox.kill();
    console.log(`Sandbox ${sandbox.sandboxId} killed`);
  }
}
