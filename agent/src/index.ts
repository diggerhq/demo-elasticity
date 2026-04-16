import "dotenv/config";
import { query } from "@anthropic-ai/claude-agent-sdk";
import { readFileSync, mkdtempSync } from "node:fs";
import { parseArgs } from "node:util";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { tmpdir } from "node:os";

const __dirname = dirname(fileURLToPath(import.meta.url));

let repo: string | undefined;
let issue: string | undefined;

if (process.env.AGENT_INPUT_PATH) {
  const input = JSON.parse(readFileSync(process.env.AGENT_INPUT_PATH, "utf-8"));
  repo = input.repo;
  issue = String(input.issue_number);
} else {
  const { values } = parseArgs({
    options: {
      repo:  { type: "string" },
      issue: { type: "string" },
    },
    strict: true,
  });
  repo = values.repo;
  issue = values.issue;
}

if (!repo || !issue) {
  console.error("Usage: index.ts --repo owner/repo --issue 42");
  console.error("  or set AGENT_INPUT_PATH to a JSON file with { repo, issue_number }");
  process.exit(1);
}

const workdir = process.env.AGENT_WORKDIR ?? mkdtempSync(join(tmpdir(), "agent-"));
console.log(`Working directory: ${workdir}`);

const systemPrompt = readFileSync(join(__dirname, "../prompt.md"), "utf-8");

const stream = query({
  prompt: [
    `Resolve this GitHub issue.`,
    ``,
    `Repository: ${repo}`,
    `Issue number: ${issue}`,
    ``,
    `Start by running: gh issue view ${issue} --repo ${repo}`,
  ].join("\n"),
  options: {
    model: "claude-sonnet-4-6",
    systemPrompt,
    tools: ["Bash", "Read", "Write", "Edit", "Glob", "Grep"],
    allowedTools: ["Bash", "Read", "Write", "Edit", "Glob", "Grep"],
    permissionMode: "acceptEdits",
    cwd: workdir,
    maxTurns: 200,
  },
});

let exitCode = 0;

for await (const message of stream) {
  if (message.type === "assistant" && message.message?.content) {
    for (const block of message.message.content as any[]) {
      if (block.type === "text" && block.text) {
        // Show agent thinking — keep it short for demo
        const text = block.text.trim();
        if (text) console.log(`\n  ${text.slice(0, 300)}`);
      } else if (block.type === "tool_use") {
        // Show tool calls — highlight bash commands for demo visibility
        if (block.name === "Bash") {
          const cmd = block.input?.command ?? "";
          console.log(`\n  $ ${cmd}`);
        } else if (block.name === "Edit") {
          const file = block.input?.file_path ?? "";
          console.log(`\n  [edit] ${file.split("/").pop()}`);
        }
        // Skip Read/Glob/Grep — noise for demo
      }
    }
  }

  if (message.type === "result") {
    if (message.subtype === "success") {
      const dur = Math.round((message as any).duration_ms / 1000);
      const cost = (message as any).total_cost_usd?.toFixed(2);
      console.log(`\n====================================`);
      console.log(`  DONE — ${dur}s, $${cost}`);
      console.log(`====================================\n`);
    } else {
      const errors = "errors" in message ? (message as any).errors : [];
      console.error("\nAgent failed:", message.subtype, errors);
      exitCode = 1;
    }
  }
}

process.exit(exitCode);
