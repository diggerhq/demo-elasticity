import "dotenv/config";
import { query } from "@anthropic-ai/claude-agent-sdk";
import { readFileSync, mkdtempSync } from "node:fs";
import { parseArgs } from "node:util";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { tmpdir } from "node:os";

const __dirname = dirname(fileURLToPath(import.meta.url));

const { values } = parseArgs({
  options: {
    repo:  { type: "string" },
    issue: { type: "string" },
  },
  strict: true,
});

if (!values.repo || !values.issue) {
  console.error("Usage: index.ts --repo owner/repo --issue 42");
  process.exit(1);
}

// Use a temp directory so we don't clone repos into the source tree
const workdir = process.env.AGENT_WORKDIR ?? mkdtempSync(join(tmpdir(), "agent-"));
console.log(`Working directory: ${workdir}`);

const systemPrompt = readFileSync(join(__dirname, "../prompt.md"), "utf-8");

const stream = query({
  prompt: [
    `Resolve this GitHub issue.`,
    ``,
    `Repository: ${values.repo}`,
    `Issue number: ${values.issue}`,
    ``,
    `Start by running: gh issue view ${values.issue} --repo ${values.repo}`,
  ].join("\n"),
  options: {
    model: "claude-sonnet-4-6",
    systemPrompt,
    tools: ["Bash", "Read", "Write", "Edit", "Glob", "Grep"],
    allowedTools: ["Bash", "Read", "Write", "Edit", "Glob", "Grep"],
    permissionMode: "acceptEdits",
    cwd: workdir,
    maxTurns: 50,
  },
});

let exitCode = 0;

for await (const message of stream) {
  if (message.type === "assistant" && message.message?.content) {
    for (const block of message.message.content as any[]) {
      if (block.type === "text" && block.text) {
        console.log(`\n[agent] ${block.text.slice(0, 300)}`);
      } else if (block.type === "tool_use") {
        const input = block.input ? JSON.stringify(block.input).slice(0, 200) : "";
        console.log(`\n[tool] ${block.name}(${input})`);
      }
    }
  }

  if (message.type === "result") {
    if (message.subtype === "success") {
      console.log("\nAgent completed successfully.");
      console.log(`Duration: ${(message as any).duration_ms}ms, Cost: $${(message as any).total_cost_usd}`);
    } else {
      const errors = "errors" in message ? (message as any).errors : [];
      console.error("\nAgent failed:", message.subtype, errors);
      exitCode = 1;
    }
  }
}

process.exit(exitCode);
