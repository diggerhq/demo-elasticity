import { Sandbox } from "@opencomputer/sdk";
import { postComment } from "./github";

// Checkpoint ID from deploy-manual.ts output
const CHECKPOINT_ID = process.env.CHECKPOINT_ID ?? "03dc171a-c12d-455b-950f-bbd3f9db7a68";

// Rust is installed in /workspace, not /root — need explicit env for exec
const RUST_ENV = {
  RUSTUP_HOME: "/workspace/.rustup",
  CARGO_HOME: "/workspace/.cargo",
  PATH: "/workspace/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
  RUST_BACKTRACE: "1",
  AGENT_WORKDIR: "/workspace",
  CARGO_BUILD_JOBS: "1",
};

interface RunContext {
  repo: string;
  issueNumber: number;
}

export async function runAgent(ctx: RunContext): Promise<void> {
  const sandbox = await Sandbox.createFromCheckpoint(CHECKPOINT_ID, {
    apiKey: process.env.OPENCOMPUTER_API_KEY,
    apiUrl: process.env.OPENCOMPUTER_API_URL,
    timeout: 1800,
  });

  console.log(`Sandbox ${sandbox.sandboxId} created for #${ctx.issueNumber}`);

  try {
    const session = await sandbox.exec.start(
      "node",
      {
        args: [
          "/workspace/agent/dist/index.js",
          "--repo", ctx.repo,
          "--issue", String(ctx.issueNumber),
        ],
        cwd: "/workspace",
        env: {
          ...RUST_ENV,
          ANTHROPIC_API_KEY: process.env.ANTHROPIC_API_KEY ?? "",
          GITHUB_TOKEN: process.env.GITHUB_TOKEN ?? "",
        },
        timeout: 1500,
        onStdout: (data: Uint8Array) => process.stdout.write(data),
        onStderr: (data: Uint8Array) => process.stderr.write(data),
      },
    );

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
